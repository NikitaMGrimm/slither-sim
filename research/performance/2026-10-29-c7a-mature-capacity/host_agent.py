"""Private-interface host collection/control. All raw data stays task-local."""
from pathlib import Path
import json,os,subprocess,time,threading,signal,pwd,tarfile,hashlib
from http.server import ThreadingHTTPServer,BaseHTTPRequestHandler
from urllib.parse import urlsplit,parse_qs
from metrics import processes,prior
r=Path(__file__).resolve().parent;job=json.loads((r/'job.json').read_text());root=r/'host-run';root.mkdir()
lock=threading.Lock();state={'serverAlive':False};stop=threading.Event();stopped=threading.Event();offset=0
config={'bind':f"{job['serverPrivate']}:{job['gamePort']}",'seed':7,'players':{'max':751},'bots':{'count':0,'threads':1},'simulation':{'world':{'preset':'official'},'delivery':'official'}}
(root/'config.json').write_text(json.dumps(config)+'\n')
cpu=json.loads(subprocess.check_output(['lscpu','--json'],text=True));(root/'cpu-private.json').write_text(json.dumps(cpu)+'\n')
env=dict(os.environ,RAYON_NUM_THREADS='1',CAPACITY_MEASURE=str(root/'MEASURE'),CAPACITY_DIAGNOSTIC=str(root/'DIAGNOSTIC'))
log=(root/'host.log').open('xb');host=subprocess.Popen([str(r/'bin/host'),'host','--config',str(root/'config.json'),'--stats','1'],stdout=log,stderr=subprocess.STDOUT,env=env,start_new_session=True)
(root/'process.json').write_text(json.dumps({'pid':host.pid,'command':['host','--stats','1'],'binarySha256':hashlib.file_digest((r/'bin/host').open('rb'),'sha256').hexdigest()})+'\n')
def inventory(label):
 raw=subprocess.check_output(['ps','-eo','pid,ppid,stat,pcpu,args','-ww'],text=True).replace(pwd.getpwuid(os.getuid()).pw_name,'<redacted-account>');(root/(label+'-processes.txt')).write_text(raw)
 return hashlib.sha256(raw.encode()).hexdigest()
def collect():
 global offset
 last_sample=0
 with (root/'telemetry.jsonl').open('x') as out:
  try:
   while not stop.is_set():
    assert host.poll() is None,'host exited'
    with (root/'host.log').open() as stream:
     stream.seek(offset)
     while True:
      at=stream.tell();line=stream.readline()
      if not line.endswith('\n'):offset=at;break
      value=None;key=None
      if line.startswith('CENSUS '):value=json.loads(line[7:]);value.pop('tickNanos',None);key='census'
      elif line.startswith('SOCKET '):value=json.loads(line[7:]);key='socket'
      elif 'stats: ticks/s=' in line:
       value=prior.parse_stats(line)
       if value is not None:value['unix']=time.time();key='stats'
      if key:
       with lock:state[key]=value
    if time.monotonic()-last_sample>=1:
     snap=processes.snapshot(host.pid);assert all(t['cpus']==[0] for t in snap['threads'])
     own=processes.snapshot(os.getpid());core=next(x for x in Path('/proc/stat').read_text().splitlines() if x.startswith('cpu0 '))
     out.write(json.dumps({'server':snap,'collector':own,'core':core})+'\n');out.flush();last_sample=time.monotonic()
    with lock:state['serverAlive']=True;state['serverUnix']=time.time()
    time.sleep(.05)
  except BaseException as e:
   with lock:state['failure']=type(e).__name__+': '+str(e);state['serverAlive']=False
  finally:
   if host.poll() is None:
    # Bracket the final counted edge before terminating either host thread.
    time.sleep(1.1);out.write(json.dumps({'server':processes.snapshot(host.pid),'collector':processes.snapshot(os.getpid())})+'\n');out.flush()
    os.killpg(host.pid,signal.SIGTERM)
    try:host.wait(timeout=10)
    except subprocess.TimeoutExpired:os.killpg(host.pid,signal.SIGKILL);host.wait()
   log.close()
   with lock:state['serverAlive']=False;state['stopped']=True
   (root/'final.json').write_text(json.dumps(state,indent=2)+'\n');stopped.set()
threading.Thread(target=collect,daemon=True).start()
class Handler(BaseHTTPRequestHandler):
 def log_message(self,*args):pass
 def answer(self,value):
  raw=json.dumps(value).encode();self.send_response(200);self.send_header('Content-Type','application/json');self.send_header('Content-Length',str(len(raw)));self.end_headers();self.wfile.write(raw)
 def do_GET(self):
  if self.path.startswith('/log?'):
   offset=int(parse_qs(urlsplit(self.path).query)['offset'][0]);assert offset>=0
   with (root/'host.log').open('rb') as stream:stream.seek(offset);raw=stream.read(1048576)
   self.send_response(200);self.send_header('Content-Length',str(len(raw)));self.send_header('X-Host-Alive',str(int(host.poll() is None)));self.end_headers();self.wfile.write(raw);return
  if self.path=='/status':
   with lock:value=dict(state)
   return self.answer(value)
  if self.path=='/bundle' and stopped.is_set():
   archive=r/'host-results.tar.gz'
   if not archive.exists():
    with tarfile.open(archive,'w:gz') as out:out.add(root,arcname='host-run')
   raw=archive.read_bytes();self.send_response(200);self.send_header('Content-Length',str(len(raw)));self.end_headers();self.wfile.write(raw);return
  self.send_error(404)
 def do_POST(self):
  n=int(self.headers.get('Content-Length','0'));assert n<=1024;value=json.loads(self.rfile.read(n));command=value['command'];now=time.time()
  if command=='measure':
   label=value['label'];assert label.isalnum() or all(c.isalnum() or c in '-_' for c in label)
   digest=inventory(label+('-start' if value['active'] else '-end'))
   if value['active']:(root/'MEASURE').touch()
   else:(root/'MEASURE').unlink(missing_ok=True)
   event={'unix':time.time(),'label':label,'active':value['active'],'processInventorySha256':digest}
   with (root/'marks.jsonl').open('a') as out:out.write(json.dumps(event)+'\n')
   return self.answer(event)
  if command=='stop':stop.set();return self.answer({'stopRequested':now})
  self.send_error(400)
server=ThreadingHTTPServer((job['serverPrivate'],job['controlPort']),Handler);server.serve_forever(poll_interval=.25)
