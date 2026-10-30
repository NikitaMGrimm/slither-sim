from pathlib import Path
import os,time,json,pwd,subprocess
r=Path(__file__).resolve().parent
os.sched_setaffinity(0,{3})
hz=os.sysconf('SC_CLK_TCK')
account=pwd.getpwuid(os.getuid()).pw_name
def inventory(label):
 text=subprocess.check_output(['ps','-eo','pid,ppid,stat,pcpu,args','-ww'],text=True)
 (r/(label+'-processes.txt')).write_text(text.replace(account,'<redacted-account>'))
def sample():
 processes=[]
 for task in Path('/proc').iterdir():
  if not task.name.isdigit():continue
  try:
   f=(task/'stat').read_text().rsplit(')',1)[1].split()
   command=(task/'cmdline').read_bytes()
   processes.append({'pid':int(task.name),'start':f[19],'cpuTicks':int(f[11])+int(f[12]),'kernel':bool(int(f[6])&0x200000),'study':str(r).encode() in command})
  except (FileNotFoundError,ProcessLookupError,PermissionError):pass
 values=next(x.split()[1:] for x in Path('/proc/stat').read_text().splitlines() if x.startswith('cpu0 '))
 return {'unix':time.time(),'monotonic':time.monotonic(),'processes':processes,'hostVcpuSeconds':{k:int(v)/hz for k,v in zip(['user','nice','system','idle','iowait','irq','softirq','steal','guest','guestNice'],values)}}
def delta(before,after):
 old={v['pid']:v for v in before['processes']};new={v['pid']:v for v in after['processes']}
 amounts=[]
 for v in after['processes']:
  prior=old.get(v['pid'])
  if prior is not None and prior['start']==v['start'] and not v['study'] and not v['kernel']:
   value=v['cpuTicks']-prior['cpuTicks'];assert value>=0;amounts.append(value/hz)
 wall=after['unix']-before['unix']
 return {'wallSeconds':wall,'largestOtherUserProcessCpuSeconds':max(amounts,default=0),'totalOtherUserProcessCpuSeconds':sum(amounts),'largestOtherCpuRate':max(amounts,default=0)/wall,'totalOtherCpuRate':sum(amounts)/wall,'newNonStudyProcesses':sum(not v['study'] and not v['kernel'] and (p not in old or old[p]['start']!=v['start']) for p,v in new.items()),'exitedNonStudyProcesses':sum(not v['study'] and not v['kernel'] and (p not in new or new[p]['start']!=v['start']) for p,v in old.items()),'hostVcpuContextSeconds':{k:after['hostVcpuSeconds'][k]-before['hostVcpuSeconds'][k] for k in before['hostVcpuSeconds']}}
inventory('calibration-initial')
print('CALIBRATION SETTLING: no other guest work',flush=True)
time.sleep(20)
rows=[]
with (r/'calibration-raw.jsonl').open('x') as raw:
 before=sample();raw.write(json.dumps(before)+'\n');raw.flush()
 for i in range(12):
  inventory('calibration-window-'+str(i)+'-before')
  deadline=time.monotonic()+30
  while True:
   remaining=deadline-time.monotonic()
   if remaining<=0:break
   time.sleep(min(1,remaining))
   after=sample();raw.write(json.dumps(after)+'\n');raw.flush()
  after=sample();raw.write(json.dumps(after)+'\n');raw.flush()
  row=delta(before,after);row['window']=i;rows.append(row)
  inventory('calibration-window-'+str(i)+'-after')
  (r/'calibration-progress.json').write_text(json.dumps({'completed':len(rows),'planned':12})+'\n')
  print('CALIBRATION WINDOW',i+1,'OF 12 COMPLETE',flush=True)
  before=after
rate=max(v['largestOtherCpuRate'] for v in rows)
qualifies=rate<=.02 and all(v['totalOtherCpuRate']<=.05 for v in rows)
out={'windows':rows,'plannedWindows':12,'windowSeconds':30,'counterQuantumSeconds':1/hz,'idleEnvelopeRate':rate,'excessRateAllowance':.01,'endpointQuantumCount':2,'isolationFormula':'W * (idleEnvelopeRate + 0.01) + 2 * counterQuantumSeconds','exampleThirtySecondAllowanceSeconds':30*(rate+.01)+2/hz,'idleQualificationPasses':qualifies}
(r/'calibration.json').write_text(json.dumps(out,indent=2)+'\n')
assert qualifies,'calibration guest was not idle; retain and stop'
print('CALIBRATION COMPLETE / NO HOST TIMING',flush=True)
