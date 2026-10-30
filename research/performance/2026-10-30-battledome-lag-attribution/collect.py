"""Remote-only driver, extending #286's retained capture implementation by reference.

python3 collect.py ROOT PRIVATE_JOB. prior_run.py/checkpoint.py are the exact
#286 helpers, staged beside this file; they are not copied into this study.
"""
from pathlib import Path
import importlib.util
import json
import os
import signal
import subprocess
import sys
import time
import urllib.request

from bundle import package,sha

root=Path(sys.argv[1]).resolve()
job=json.loads(Path(sys.argv[2]).read_text())
server=job["serverPrivate"]
endpoint=f"http://{server}:18769"
os.sched_setaffinity(0,{7})
spec=importlib.util.spec_from_file_location("prior_run",Path(__file__).with_name("prior_run.py"))
prior=importlib.util.module_from_spec(spec);spec.loader.exec_module(prior)
rows=root/"rows";rows.mkdir(exist_ok=True)
load_dir=root/"load";load_dir.mkdir(exist_ok=True)
load=probe=ping=None


def api(action,**values):
    request=urllib.request.Request(endpoint,json.dumps(dict(action=action,**values)).encode())
    with urllib.request.urlopen(request,timeout=120) as response:return json.load(response)


def progress(stage,**values):
    prior.atomic_text(root/"progress.json",json.dumps(dict(stage=stage,unix=time.time(),**values))+"\n")


def wait(seconds,stage):
    until=time.monotonic()+seconds
    while time.monotonic()<until:
        assert api("status")["alive"]
        assert load is None or load.poll() is None,"Generator exited"
        progress(stage,remaining=round(until-time.monotonic()))
        time.sleep(min(3,max(0,until-time.monotonic())))


def start_probe(label):
    directory=rows/label;directory.mkdir()
    capture=prior.Capture(directory)
    log=(directory/"probe.log").open("w")
    process=subprocess.Popen(["taskset","-c","6",str(root/"bin/probe"),server,"8766",
                              str(directory),"1800",str(900+len(list(rows.iterdir())))],
                             stdout=log,stderr=subprocess.STDOUT,start_new_session=True)
    return directory,capture,process,log


def checkpoint(label,directory,window,start):
    meta=api("checkpoint",label=label,tickOffset=start["tickBytes"],window=window)
    assert meta["complete"] and meta["allMembersVerified"] and meta["requiredSourcesVerified"]
    urllib.request.urlretrieve(endpoint+f"/{label}.tar.gz",directory/"server.tar.gz")
    assert (directory/"server.tar.gz").stat().st_size==meta["archiveBytes"]
    assert sha(directory/"server.tar.gz")==meta["archiveSha256"]
    (directory/"server-retrieval.json").write_text(json.dumps(meta)+"\n")
    (directory/"window.json").write_text(json.dumps(window)+"\n")
    (directory/"pins.json").write_bytes((root/"pins.json").read_bytes())
    for source in [root/"clients.jsonl",root/"icmp.log"]:
        data=source.read_bytes();(directory/source.name).write_bytes(data[:data.rfind(b"\n")+1])
    ready=package(directory,root/f"{label}.tar.gz")
    progress(label+":complete",archive=ready)


def measure(label,mode,seconds,kind,existing=None):
    api("mode",label=label,mode=mode)
    probe=existing or start_probe(label)
    directory,capture,process,log=probe
    wait(5 if kind.endswith("calibration") else 10,label+":settle")
    assert process.poll() is None,"Probe exited before window"
    start=api("mark",label=label+":start")
    begin=time.time();(directory/"START").touch()
    prior.atomic_text(load_dir/"RESET",label)
    wait(seconds,label+":count")
    end=time.time();finish=api("mark",label=label+":end")
    (directory/"STOP").touch();process.wait(timeout=30);log.close();capture.finish()
    assert process.returncode==0 and (directory/"COMPLETE").exists(),"Probe failed"
    window={"label":label,"mode":mode,"kind":kind,"seats":320,
            "startUnix":begin,"endUnix":end,"startTargetMono":start["mono"],
            "endTargetMono":finish["mono"],"targetStartUnix":start["unix"],
            "targetEndUnix":finish["unix"]}
    checkpoint(label,directory,window,start)


def process_cpu_per_tick(label):
    # Existing target stats supply tick rate in OFF; schedstat supplies process
    # CPU in all arms. The denominator is descriptive, not a fixed-work claim.
    import re,tarfile
    directory=rows/label;window=json.loads((directory/"window.json").read_text())
    with tarfile.open(directory/"server.tar.gz") as archive:
        telemetry=[json.loads(x) for x in archive.extractfile("telemetry.jsonl")]
        logs=[json.loads(x) for x in archive.extractfile("host.jsonl")]
    selected=[x for x in telemetry if window["startTargetMono"]<=x["mono"]<=window["endTargetMono"]]
    first,last=selected[0],selected[-1]
    cpu=lambda x:sum(t["schedstat"][0] for t in x["threads"])
    rates=[]
    for x in logs:
        if first["mono"]<=x["mono"]<=last["mono"]:
            found=re.search(r"ticks/s=([\d.]+)",x["line"])
            if found:rates.append(float(found[1]))
    assert rates
    return (cpu(last)-cpu(first))/1e6 / ((last["mono"]-first["mono"])*sum(rates)/len(rates))


def main():
    global load,ping
    ping_log=(root/"icmp.log").open("w")
    ping=subprocess.Popen(["stdbuf","-oL","ping","-n","-D","-i","0.2",server],
                          stdout=ping_log,stderr=subprocess.STDOUT,start_new_session=True)
    api("start",bind=f"{server}:8766")
    prior.atomic_text(load_dir/"target","79\n")
    load_log=(root/"clients.jsonl").open("w")
    load=subprocess.Popen([str(root/"bin/clients"),str(load_dir),"8766","79",server],
                          stdout=subprocess.DEVNULL,stderr=load_log,start_new_session=True)
    first=start_probe("01-off")
    try:
        wait(300,"warm-80")
        prior.atomic_text(load_dir/"target","159\n");wait(60,"warm-160")
        prior.atomic_text(load_dir/"target","319\n");wait(60,"warm-320")
        for label,mode in [("01-off","off"),("02-light","light"),("03-trace","trace"),
                           ("04-trace","trace"),("05-light","light"),("06-off","off")]:
            measure(label,mode,60,"calibration",first if label=="01-off" else None)
        costs={label:process_cpu_per_tick(label) for label in ["01-off","02-light","03-trace","04-trace","05-light","06-off"]}
        light=(costs["02-light"]+costs["05-light"])/2
        trace=(costs["03-trace"]+costs["04-trace"])/2
        mode="trace" if trace/light<=1.05 and trace-light<=.15 else "light"
        (root/"calibration.json").write_text(json.dumps({"hostCPUPerTickMs":costs,
            "lightMedianMs":light,"traceMedianMs":trace,"primaryMode":mode,
            "descriptiveEcologyDriftNotFixedWork":True},indent=2)+"\n")
        for label in ["07-primary","08-primary","09-primary"]:
            measure(label,mode,300,"primary")
    finally:
        prior.atomic_text(load_dir/"STOP","stop\n")
        if load is not None:load.wait(timeout=30);load_log.close()
        if ping is not None:os.killpg(ping.pid,signal.SIGINT);ping.wait(timeout=5);ping_log.close()
        api("stop")
    assert load.returncode==0
    progress("collection-complete")


if __name__=="__main__":
    try:main()
    except Exception as error:progress("failed",error=repr(error));raise
