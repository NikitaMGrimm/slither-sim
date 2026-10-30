"""Disposable one-core target controller; all endpoints and raw logs stay private."""
from pathlib import Path
from http.server import BaseHTTPRequestHandler, HTTPServer
import hashlib
import json
import os
import re
import signal
import subprocess
import sys
import tarfile
import threading
import time

root = Path(sys.argv[1]).resolve()
session = root / "session"
session.mkdir(exist_ok=True)
host = trace = None
reader = sampler = None
sampling = threading.Event()
trace_stream = None
trace_label = None


def lines(process):
    with (session / "host.jsonl").open("w") as stream:
        for line in process.stderr:
            stream.write(json.dumps({"unix":time.time(), "mono":time.monotonic(),
                                     "line":line.rstrip()})+"\n")
            stream.flush()


def sample():
    with (session / "telemetry.jsonl").open("w") as stream:
        while not sampling.wait(.1):
            threads = []
            for path in Path(f"/proc/{host.pid}/task").glob("*"):
                try:
                    threads.append({"tid":int(path.name), "comm":(path/"comm").read_text().strip(),
                                    "schedstat":list(map(int,(path/"schedstat").read_text().split()))})
                except FileNotFoundError:
                    pass
            stat = Path("/proc/stat").read_text().splitlines()
            value = {"mono":time.monotonic(), "unix":time.time(), "threads":threads,
                     "cpu":list(map(int,stat[0].split()[1:])),
                     "ctxt":int(next(x for x in stat if x.startswith("ctxt ")).split()[1])}
            stream.write(json.dumps(value)+"\n")
            stream.flush()


def stop_trace():
    global trace, trace_stream
    if trace is not None:
        if trace.poll() is None:
            subprocess.run(["sudo","-n","kill","-INT","--",f"-{trace.pid}"],check=True)
        trace.wait(timeout=15)
        trace_stream.close()
        trace = None


def stop():
    global host
    stop_trace()
    if host is not None:
        (session / "RECORD").unlink(missing_ok=True)
        time.sleep(.3)  # let the observer close its current buffered block
        sampling.set()
        sampler.join(timeout=3)
        if host.poll() is None:
            os.killpg(host.pid,signal.SIGTERM)
        host.wait(timeout=10)
        reader.join(timeout=3)
        host = None


def package(label, directory):
    files = sorted(p for p in directory.iterdir() if p.is_file())
    manifest = {p.name:hashlib.sha256(p.read_bytes()).hexdigest() for p in files}
    (directory / "manifest.json").write_text(json.dumps(manifest,indent=2)+"\n")
    temporary = root / f"{label}.tar.gz.tmp"
    with tarfile.open(temporary,"w:gz",compresslevel=1) as archive:
        for path in files + [directory/"manifest.json"]:
            archive.add(path,arcname=path.name)
    with tarfile.open(temporary,"r|gz") as archive:
        for member in archive:
            if member.name != "manifest.json":
                assert hashlib.sha256(archive.extractfile(member).read()).hexdigest()==manifest[member.name]
    output = root / f"{label}.tar.gz"
    temporary.replace(output)
    receipt = {"complete":True,"allMembersVerified":True,"requiredSourcesVerified":True,
               "archiveBytes":output.stat().st_size,
               "archiveSha256":hashlib.sha256(output.read_bytes()).hexdigest()}
    (root / f"{label}-ready.json").write_text(json.dumps(receipt)+"\n")
    return receipt


class Handler(BaseHTTPRequestHandler):
    def log_message(self,*_): pass

    def do_POST(self):
        global host,reader,sampler,trace,trace_stream,trace_label
        try:
            request=json.loads(self.rfile.read(int(self.headers["Content-Length"])))
            action=request["action"]
            if action=="start":
                assert host is None
                config={"bind":request["bind"],"seed":7,"players":{"max":641},
                        "bots":{"count":0,"threads":1},
                        "simulation":{"world":{"preset":"battledome"}}}
                path=session/"config.json";path.write_text(json.dumps(config)+"\n")
                (session/"host.jsonl").write_text("")
                host=subprocess.Popen(["taskset","-c","0",str(root/"bin/host"),"host",
                    "--config",str(path),"--stats","0.1"],stderr=subprocess.PIPE,
                    stdout=subprocess.DEVNULL,text=True,start_new_session=True,
                    env=dict(os.environ,STUDY_LOG=str(session/"ticks.bin")))
                reader=threading.Thread(target=lines,args=(host,),daemon=True);reader.start()
                sampling.clear();sampler=threading.Thread(target=sample,daemon=True);sampler.start()
                deadline=time.monotonic()+10
                while "Listening on " not in (session/"host.jsonl").read_text():
                    assert host.poll() is None and time.monotonic()<deadline
                    time.sleep(.02)
                result={"started":True}
            elif action=="status":
                result={"alive":host is not None and host.poll() is None,"unix":time.time()}
            elif action=="mode":
                stop_trace()
                label=request["label"];assert re.fullmatch(r"[a-z0-9-]+",label)
                mode=request["mode"];assert mode in ["off","light","trace","sched"]
                if mode=="off":(session/"RECORD").unlink(missing_ok=True)
                else:(session/"RECORD").touch()
                if mode in ["trace","sched"]:
                    trace_label=label
                    trace_stream=(session/f"{label}-perf.log").open("w")
                    events=["sched:sched_switch","sched:sched_wakeup","sched:sched_wakeup_new",
                            "irq:irq_handler_entry","irq:irq_handler_exit",
                            "irq:softirq_entry","irq:softirq_exit"]
                    if mode=="sched":events=events[:3]
                    command=["sudo","-n","perf","record","-a","--clockid","mono","-m","512"]
                    for event in events:command += ["-e",event]
                    command += ["-o",str(session/f"{label}-perf.data"),"--","sleep","3600"]
                    trace=subprocess.Popen(command,stdout=subprocess.DEVNULL,stderr=trace_stream,
                                             start_new_session=True)
                result={"mode":mode,"unix":time.time(),"mono":time.monotonic()}
            elif action=="mark":
                result={"unix":time.time(),"mono":time.monotonic(),
                        "tickBytes":(session/"ticks.bin").stat().st_size,
                        "label":request["label"]}
            elif action=="checkpoint":
                label=request["label"];assert re.fullmatch(r"[a-z0-9-]+",label)
                stop_trace();(session/"RECORD").unlink(missing_ok=True);time.sleep(.3)
                directory=root/"checkpoints"/label;directory.mkdir(parents=True)
                for name in ["host.jsonl","telemetry.jsonl","ticks.meta.json","config.json"]:
                    data=(session/name).read_bytes()
                    if name.endswith("jsonl"):data=data[:data.rfind(b"\n")+1]
                    (directory/name).write_bytes(data)
                with (session/"ticks.bin").open("rb") as stream:
                    offset=request["tickOffset"]//128*128;stream.seek(offset)
                    data=stream.read();data=data[:len(data)//128*128]
                    (directory/"ticks.bin").write_bytes(data)
                for name in [f"{label}-perf.data",f"{label}-perf.log"]:
                    if (session/name).exists():
                        subprocess.run(["sudo","-n","chmod","a+r",str(session/name)],check=True)
                        import shutil
                        shutil.copyfile(session/name,directory/name)
                (directory/"window.json").write_text(json.dumps(request["window"])+"\n")
                result=package(label,directory)
            elif action=="stop":
                stop();result={"stopped":True}
            else:raise ValueError("Unknown target action")
            output=json.dumps(result).encode();self.send_response(200)
        except Exception as error:
            output=json.dumps({"error":repr(error)}).encode();self.send_response(500)
        self.end_headers();self.wfile.write(output)

    def do_GET(self):
        name=self.path.removeprefix("/")
        if not re.fullmatch(r"[a-z0-9-]+\.tar\.gz",name) or not (root/name.replace(".tar.gz","-ready.json")).exists():
            self.send_error(404);return
        self.send_response(200);self.end_headers()
        with (root/name).open("rb") as stream:
            while data:=stream.read(1<<20):self.wfile.write(data)


HTTPServer(("0.0.0.0",int(sys.argv[2])),Handler).serve_forever()
