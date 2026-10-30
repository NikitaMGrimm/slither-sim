"""Reuse the ladder collector, replacing only its fixed sequence and publication."""
from pathlib import Path
r=Path(__file__).resolve().parent
s=(r/'shared258/run_ladder.py').read_text()
def swap(old,new):
 global s
 assert s.count(old)==1,(old,s.count(old));s=s.replace(old,new)
swap('from window import reduce_window','from acceptance import reduce_window')
swap('import os,json,time,subprocess,signal,urllib.request,ipaddress,tarfile,hashlib','import os,json,time,subprocess,signal,urllib.request,ipaddress,tarfile,hashlib,re')
swap('client_rows=[];', 'client_rows=[];metric_workers=set();')
swap("   if line.startswith('CLIENT '):client_rows.append(json.loads(line[7:]))", "   if line.startswith('CLIENT '):client_rows.append(json.loads(line[7:]))\n   elif line.startswith('CLIENT_WINDOW '):metric_workers.add(int(re.search(r'\"worker\":(\\d+)',line)[1]))")
swap(';target=350',';target=500')
swap("'runtime':'692cd561714b8af3ea95adb3adcbd807b3a48734'", "'runtime':json.loads((r/'pins.json').read_text())['runtime']")
swap("'initialBots':350,'maxBotSlots':750,'humanSlotReserved':True", "'initialBots':500,'maxBotSlots':500,'humanSlotReserved':False")
swap("v['living']>=343", "v['living']>=490")
first=s.index(" save('ladder');previous=target-50;")
last=s.index('\nexcept BaseException as e:',first)
s=s[:first]+''' save('settling',bots=target);wait(10)
 recent=[v for v in client_rows if v['unix']>=time.time()-3]
 assert {v['worker'] for v in recent}==set(range(1,7)) and all(v['target']==500 for v in recent),'500 clients not admitted'
 label='acceptance-500';write('RESET','count')
 window={'label':label,'bots':500,'start':api('/',{'command':'measure','label':label,'active':True})};save('counting',bots=500)
 wait(1800);window['end']=api('/',{'command':'measure','label':label,'active':False});write('RESET','end');wait(2)
 metric_deadline=time.monotonic()+30
 while metric_workers!=set(range(1,7)):
  pump();assert time.monotonic()<metric_deadline,'Client metric publication deadline';time.sleep(.05)
 result=reduce_window(root,window);summary['windows'].append(window|{'result':result})
 summary.update(status='complete',stopReason='acceptance-passed' if result['passes'] else 'acceptance-failed');save('complete')
 print('ACCEPTANCE',json.dumps({'passes':result['passes'],'gates':result['gates'],'p99Ms':result['worldCpuMsPerTick']['p99']}),flush=True)
'''+s[last:]
first=s.index(" (r/'run-summary.json').write_text(json.dumps(summary,indent=2)")
s=s[:first]+''' from delivery import publish
 summary['outcome']=summary['status'];summary['completeHostEvidence']=(r/'host-run').exists() and not summary.get('retrievalFailure')
 paths=[root,r/'run-summary.json',r/'pins.json']
 if (r/'host-run').exists():paths.append(r/'host-run')
 publish(r,summary,paths)
'''
if __name__=='__main__':
 exec(compile(s,str(r/'linked-acceptance-runner.py'),'exec'),{'__file__':str(r/'linked-acceptance-runner.py'),'__name__':'__main__'})
