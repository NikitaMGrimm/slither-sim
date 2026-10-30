"""Synthetic controls: amended CPU reporting, exact class edges and real gates."""
from pathlib import Path
import json,math,tempfile
from acceptance import mature,verdict,quantiles,hist_quantiles,reduce_window
import run_acceptance
compile(run_acceptance.s,'linked-acceptance-runner.py','exec')
good={'living':490,'meanSegments':45.,'p90Segments':156,'p99Segments':200}
assert mature(good)
assert mature(good|{'meanSegments':65.,'p90Segments':196})
for key,value in [('living',489),('meanSegments',44.999),('meanSegments',65.001),('p90Segments',155),('p90Segments',197),('p99Segments',199)]:assert not mature(good|{key:value})
args=dict(client_valid=True,zero_deferrals=True,start_mature=True,end_mature=True)
assert verdict(**args)['passes']
for key in args:assert not verdict(**(args|{key:False}))['passes']
assert not verdict(**args,accounting=False)['passes']
assert quantiles([1,2,3,4])['p999']==4
assert hist_quantiles([{'x':[[1000,2],[-1000,1]]}],'x')['p99']==1
with tempfile.TemporaryDirectory() as name:
 root=Path(name);lines=[]
 for tick in range(0,258101,100):
  lines.append('CENSUS '+json.dumps(dict(tick=tick,unix=100+tick*.007,deferred=0,living=500,occupied=500,meanSegments=55.,p90Segments=176,p99Segments=247,food=28000,prey=20,radius=14800)))
  if tick:
   rows=[]
   for t in range(tick-99,tick+1):
    food=math.floor(t*.007/.504)>math.floor((t-1)*.007/.504)
    rows.append([2_000_000,8_000_000,1] if food else ([4_000_000,1_000_000,1] if t%2 else [1_000_000,4_000_000,1]))
   lines.append('CAPACITY '+json.dumps(dict(tick=tick,rows=rows,stages=[0]*16)))
 for second in range(99,1910):lines.append('SOCKET '+json.dumps(dict(unix=second,queueBytes=0,kernelSendBytes=0,players=500,externalPlayers=0,writeBlocks=0,bufferFull=0,rejects=0)))
 (root/'host-stream.log').write_text('\n'.join(lines)+'\n')
 (root/'stats.jsonl').write_text('\n'.join(json.dumps(dict(unix=s,players=500)) for s in range(99,1910))+'\n')
 clients=[]
 for second in range(99,1910):
  for w in range(1,7):clients.append('CLIENT '+json.dumps(dict(unix=second,worker=w,cpuFraction=.1,lateP99Nanos=1_000_000,frames=second*100,bytes=second*1000,decisions=second*20,joins=84,deaths=0,forced=0,admissionRefusals=0,maxReceiptAge=.01,thin=False)))
 for w in range(1,7):clients.append('CLIENT_WINDOW '+json.dumps(dict(worker=w,startUnix=100,endUnix=1900,gapsMicros=[[7000,100]],gapResidualMicros=[[0,100]],frameExcessMicros=[[1000,100]])))
 original='\n'.join(clients)+'\n';(root/'client.log').write_text(original)
 window=dict(label='synthetic',bots=500,start={'unix':100},end={'unix':1900})
 base=reduce_window(root,window);assert base['passes'] and not base['p99AtMost7ms'] and base['worldCpuMsPerTick']['p99']==10
 assert base['simulationCpuMsPerTick']['p99']+base['socketCpuMsPerTick']['p99']!=base['worldCpuMsPerTick']['p99']
 # Last exterior census, not a favorable earlier endpoint, owns end maturity.
 changed=[]
 for line in lines:
  if line.startswith('CENSUS '):
   v=json.loads(line[7:])
   if v['unix']>=1900:v['meanSegments']=66
   line='CENSUS '+json.dumps(v)
  changed.append(line)
 (root/'host-stream.log').write_text('\n'.join(changed)+'\n');assert not reduce_window(root,window)['passes']
 (root/'host-stream.log').write_text('\n'.join(lines)+'\n')
 (root/'client.log').write_text(original.replace('"lateP99Nanos": 1000000','"lateP99Nanos": 42000000'));assert not reduce_window(root,window)['passes']
 (root/'client.log').write_text(original)
print('PASS: report-only CPU p99, aligned sums, full-window coverage, exact maturity bounds, end drift, generator rejection, all amended gates',flush=True)
