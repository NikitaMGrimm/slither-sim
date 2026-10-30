"""One separately budgeted diagnostic, only after all six primary holds."""
from pathlib import Path
import subprocess,resource,os,time,json,signal
r=Path(__file__).resolve().parent;os.sched_setaffinity(0,{3})
assert json.loads((r/'series-progress.json').read_text())['status']=='complete'
assert len(json.loads((r/'selected.json').read_text())['cases'])==6
start=time.time();reserve=100.;limit=7200.
def spent():
 a=resource.getrusage(resource.RUSAGE_CHILDREN);b=resource.getrusage(resource.RUSAGE_SELF)
 return reserve+a.ru_utime+a.ru_stime+b.ru_utime+b.ru_stime
subprocess.run(['python3',str(r/'build_fine.py')],check=True)
subprocess.run(['python3',str(r/'verify_sources.py'),str(r)],check=True)
# The earlier sampler has ended; resume the same stream outside any window.
time.sleep(2);(r/'CORE_STOP').unlink()
with (r/'fine-core-controller.log').open('xb') as out:
 core=subprocess.Popen(['python3',str(r/'core_cpu.py')],stdin=subprocess.DEVNULL,stdout=out,stderr=subprocess.STDOUT)
name='current-fine-1'
try:
 with (r/'current-fine-1-controller.log').open('xb') as out:
  p=subprocess.Popen(['python3',str(r/'run_fine.py'),name],stdin=subprocess.DEVNULL,stdout=out,stderr=subprocess.STDOUT)
  while p.poll() is None:
   path=r/'runs'/name/'telemetry.jsonl';active=0.
   if path.exists():
    with path.open('rb') as f:f.seek(0,2);f.seek(max(0,f.tell()-65536));lines=f.read().splitlines()
    for line in reversed(lines):
     try:
      row=json.loads(line);active=sum(row[k]['process']['user']+row[k]['process']['system'] for k in ['server','client']);break
     except (json.JSONDecodeError,KeyError,TypeError):pass
   if spent()+active+30>=limit:
    p.send_signal(signal.SIGINT);p.wait();raise RuntimeError('fine CPU allowance reached; partial diagnostic retained')
   time.sleep(5)
  assert p.returncode==0,'fine collector failed; retain, no retry'
 subprocess.run(['python3',str(r/'select.py'),str(r),str(r/'fine-coarse-validation.json'),name,'--coarse-window'],check=True)
 text=(r/'select.py').read_text()
 for old,new in [("meta['coarseStartUnix']","meta['fineStartUnix']"),("meta['coarseEndUnix']","meta['diagnosticEndUnix']")]:
  assert old in text,old;text=text.replace(old,new)
 (r/'select-fine-window.py').write_text(text,newline='\n')
 subprocess.run(['python3',str(r/'select-fine-window.py'),str(r),str(r/'fine-validation.json'),name,'--coarse-window'],check=True)
 print('FINE DIAGNOSTIC AND BOTH QUALITY WINDOWS COMPLETE',flush=True)
finally:
 (r/'CORE_STOP').touch();core.wait(timeout=10)
 (r/'fine-cpu-receipt.json').write_text(json.dumps({'startUnix':start,'endUnix':time.time(),'setupAndAnalysisReserveSeconds':reserve,'allChildAndControllerCpuSeconds':spent()-reserve,'totalWithAllowanceSeconds':spent(),'limitSeconds':limit},indent=2)+'\n')
