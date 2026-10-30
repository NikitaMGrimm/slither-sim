"""Three fresh live pairs under PLAN.md; preserve all failed attempts.

Only execute after frozen plan, preflight, controls, reconstruction and cleanup.
No quality or outcome thresholds are chosen here. Full raw logs stay external.
"""
from pathlib import Path
import os,subprocess,json,time,resource,signal
r=Path(__file__).resolve().parent;os.sched_setaffinity(0,{3})
assert json.loads((r/'target-cleanup.json').read_text())['removed']
assert not Path('/tmp/slither-live-checkpoint-target').exists()
assert json.loads((r/'preflight.json').read_text())['cases']['pilot-candidate-3']['holds'][0]['client']['keepsUp']
assert json.loads((r/'controls.json').read_text())['originalFilesUnchanged']
assert json.loads((r/'reconstruction.json').read_text())['commonClientProductionSourcesUnchanged']
accepted=[];attempts=[];isolation_retries=0;begin=time.time();reserve=600.;limit=6*3600
order=['base','candidate','candidate','base','base','candidate']
def spent():
 a=resource.getrusage(resource.RUSAGE_CHILDREN);b=resource.getrusage(resource.RUSAGE_SELF)
 return reserve+a.ru_utime+a.ru_stime+b.ru_utime+b.ru_stime

def active_cpu(name):
 path=r/'runs'/name/'telemetry.jsonl'
 if not path.exists():return 0.
 with path.open('rb') as f:
  f.seek(0,2);f.seek(max(0,f.tell()-65536));lines=f.read().splitlines()
 for line in reversed(lines):
  try:
   v=json.loads(line);return sum(v[k]['process']['user']+v[k]['process']['system'] for k in ['server','client'])
  except (json.JSONDecodeError,KeyError,TypeError):pass
 return 0.

def save(status):
 (r/'series-progress.json').write_text(json.dumps({'status':status,'startUnix':begin,'unix':time.time(),'accepted':accepted,'attempts':attempts,'isolationRetries':isolation_retries,'cpuWithAllowanceSeconds':spent()},indent=2)+'\n')
try:
 for index,arm in enumerate(order):
  class_retries=0;attempt=0
  while True:
   # Require room for an observed normal cohort, with a 20% margin.
   expected=max([v['hostClientCpuSeconds']*1.2 for v in attempts],default=3600)
   assert limit-spent()>=expected,'insufficient remaining CPU allowance for a fresh cohort'
   attempt+=1;name=f'{arm}-pair-{index//2+1}-attempt-{attempt}'
   save('collecting '+name);print('START',name,'cpu_with_allowance',round(spent(),2),flush=True)
   with (r/(name+'-controller.log')).open('xb') as log:
    proc=subprocess.Popen(['python3',str(r/'run.py'),arm,name,'500','--short-mature'],stdin=subprocess.DEVNULL,stdout=log,stderr=subprocess.STDOUT)
    while proc.poll() is None:
     if spent()+active_cpu(name)+30>=limit:
      proc.send_signal(signal.SIGINT);proc.wait();raise RuntimeError('CPU allowance reached; partial attempt retained')
     time.sleep(5)
   charge=active_cpu(name)+8
   if proc.returncode:
    attempts.append({'name':name,'accepted':False,'hostClientCpuSeconds':charge,'failure':'collector failure'});save('failed')
    raise RuntimeError(name+' collection failed; retain and stop')
   with (r/(name+'-selection.log')).open('xb') as log:
    check=subprocess.run(['python3',str(r/'select.py'),str(r),str(r/(name+'-accepted.json')),name],stdout=log,stderr=subprocess.STDOUT)
   failure=(r/(name+'-selection.log')).read_text()
   row={'name':name,'arm':arm,'pair':index//2+1,'orderIndex':index,'accepted':check.returncode==0,'hostClientCpuSeconds':charge}
   attempts.append(row);save('validating')
   if check.returncode==0:
    accepted.append(name)
    with (r/(name+'-coarse-selection.log')).open('xb') as log:
     coarse=subprocess.run(['python3',str(r/'select.py'),str(r),str(r/(name+'-coarse.json')),name,'--coarse-window'],stdout=log,stderr=subprocess.STDOUT)
    row['coarseValid']=coarse.returncode==0
    print('ACCEPTED',name,'coarse_valid',row['coarseValid'],'cpu_with_allowance',round(spent(),2),flush=True)
    save('accepted');break
   if 'mature length class drift' in failure and class_retries==0:
    class_retries+=1;row['retryReason']='class-only';print('CLASS RETRY',name,flush=True);save('class retry');continue
   if 'competing process CPU' in failure and isolation_retries==0:
    isolation_retries+=1;row['retryReason']='isolation-only';print('ISOLATION RETRY',name,flush=True);save('isolation retry');continue
   row['failure']=failure[-2000:];raise RuntimeError(name+' failed a non-retryable quality gate')
 subprocess.run(['python3',str(r/'select.py'),str(r),str(r/'selected.json'),*accepted],check=True)
 (r/'cpu-receipt.json').write_text(json.dumps({'buildCalibrationPilotAllowanceSeconds':reserve,'allPrimaryAndControllerCpuSeconds':spent()-reserve,'totalWithAllowanceSeconds':spent(),'limitSeconds':limit},indent=2)+'\n')
 save('complete');print('ALL SIX COHORTS COMPLETE',flush=True)
except BaseException:
 save('failed');raise
finally:
 (r/'CORE_STOP').touch()
