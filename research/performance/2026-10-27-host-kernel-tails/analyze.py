"""Reduce buffered stages and kernel/scheduler evidence from one diagnostic host.

CPU percentages use /proc user/system ticks; stack groups are shares of all
recorded system-wide cpu-clock periods, including idle. No profiler timing grade.
"""
from pathlib import Path
import json,math,re,sys

def stats(v):
 v=sorted(v);n=len(v)
 return {'count':n,'p50':v[math.ceil(n*.5)-1],'p99':v[math.ceil(n*.99)-1],'max':v[-1]} if n else {'count':0}

def reduce(path):
 out={};sched=json.loads((path/'scheduler.json').read_text(encoding='utf-8'));rows=sched['rows'];a=min(rows,key=lambda r:abs(r['unix']-sched['start']));b=min(rows,key=lambda r:abs(r['unix']-sched['end']));span=b['unix']-a['unix'];out['windowSeconds']=span
 threads={}
 def vals(t):
  result={}
  for line in t['sched'].splitlines():
   if ':' not in line:continue
   k,v=line.split(':',1)
   try:result[k.strip()]=float(v.strip())
   except ValueError:pass
  return result
 for tid,t in a['threads'].items():
  if tid not in b['threads']:continue
  other=b['threads'][tid];v,w=vals(t),vals(other);user=(other['userTicks']-t['userTicks'])/sched['hz'];kernel=(other['systemTicks']-t['systemTicks'])/sched['hz']
  threads[t['name']]={'userCpuPercent':user/span*100,'kernelCpuPercent':kernel/span*100,'nice':other['nice'],'policy':w.get('policy'),'schedulerDelta':{k:w[k]-v[k] for k in ['se.sum_exec_runtime','nr_switches','nr_voluntary_switches','nr_involuntary_switches','wait_sum','wait_count'] if k in v and k in w},'schedstatDelta':[int(y)-int(x) for x,y in zip(t['schedstat'].split(),other['schedstat'].split())]}
 out['threads']=threads
 def irq(text):return {line.split(':')[0].strip():sum(map(int,line.split(':')[1].split())) for line in text.splitlines()[1:] if ':' in line}
 x,y=irq(a['softirqs']),irq(b['softirqs']);out['softirqCounts']={k:y[k]-x[k] for k in x}
 if (path/'perf-self.txt').exists():
  entries=[]
  for line in (path/'perf-self.txt').read_text(encoding='utf-8').splitlines():
   m=re.match(r'\s*([\d.]+)%\s+(\S+)\s+(\S+)\s+\[([ku.])\]\s+(.*)',line)
   if m:entries.append({'percent':float(m[1]),'thread':m[2],'dso':m[3],'mode':m[4],'symbol':re.sub(r'\s+-\s+-\s*$','',m[5]).strip()})
  out['sampleSharesByThreadMode']={}
  for e in entries:
   key=e['thread']+'/'+e['mode'];out['sampleSharesByThreadMode'][key]=out['sampleSharesByThreadMode'].get(key,0)+e['percent']
  out['kernelTop']=[e for e in entries if e['mode']=='k'][:30]
  out['socketKernelTop']=[e for e in entries if e['mode']=='k' and e['thread'].startswith('slither-socket')][:25]
  out['socketUserTop']=[e for e in entries if e['mode'] in ['u','.'] and e['thread'].startswith('slither-socket')][:15]
 if (path/'stages.json').exists():
  records=json.loads((path/'stages.json').read_text(encoding='utf-8'));sim=json.loads((path/'stages.json.simulation').read_text(encoding='utf-8'));clock=next(r for r in records if r[0]==0);epoch=clock[1]-clock[2]
  queues=[r for r in records if r[0]==3 and sched['start']<=r[1]<sched['end']];sends=[r for r in records if r[0]==4 and sched['start']<=r[1]<sched['end']];enc=[r for r in sim if r[0]==8 and sched['start']<=r[1]<sched['end']]
  out['stageSemantics']='representative slot 0; timestamps at encode handoff, socket queue, send_due pop; send pop precedes buffered flush, not a wire timestamp; profiles only'
  out['slot0EncodeLatenessMs']=stats([(r[1]-epoch-r[2])*1000 for r in enc]);out['slot0QueueLatenessMs']=stats([(r[1]-epoch-r[2])*1000 for r in queues]);out['slot0SendPopLatenessMs']=stats([(r[1]-epoch-r[2])*1000 for r in sends])
  pairs=[];q={}
  for r in records:
   if r[0]==3:q.setdefault((round(r[2],9),r[3],r[4]),[]).append(r)
   elif r[0]==4:
    key=(round(r[2],9),r[3],r[4])
    if q.get(key):
     prev=q[key].pop(0)
     if sched['start']<=r[1]<sched['end']:pairs.append((r[1]-prev[1])*1000)
  out['slot0QueueToSendPopMs']=stats(pairs)
  enc_by={}
  for r in sim:
   if r[0]==8:enc_by.setdefault((round(r[2],9),r[3],r[4]),[]).append(r)
  handoffs=[]
  for r in records:
   if r[0]!=3:continue
   key=(round(r[2],9),r[3],r[4])
   if enc_by.get(key):
    prev=enc_by[key].pop(0)
    if sched['start']<=r[1]<sched['end']:handoffs.append((r[1]-prev[1])*1000)
  out['slot0EncodeToQueueMs']=stats(handoffs)

  out['packetFamilyCounts']={k:sum(bool(r[4]&mask) for r in enc) for k,mask in [('map',1),('leaderboard',2)]}
 return out


def kernel_groups(path):
 groups={};total=0
 for sample in path.read_text(encoding='utf-8').split('\n\n'):
  lines=sample.splitlines()
  if not lines:continue
  m=re.match(r'(.+?)\s+\d+/\d+\s+\[\d+\]\s+[\d.]+:\s+(\d+) cpu-clock:',lines[0])
  if not m:continue
  comm=m[1].strip();weight=int(m[2]);total+=weight
  stack='\n'.join(lines[1:]);first=lines[1] if len(lines)>1 else ''
  if comm!='slither-sockets':continue
  if '[kernel.kallsyms]' not in first:group='user_or_unresolved'
  elif any(s in stack for s in ['handle_softirqs','__do_softirq','net_rx_action']):group='softirq'
  elif any(s in stack for s in ['do_sys_poll','do_poll ','__x64_sys_poll']):group='readiness_poll'
  elif any(s in stack for s in ['tcp_sendmsg','sock_sendmsg','__sys_sendto']):group='send_path'
  elif any(s in stack for s in ['tcp_recvmsg','sock_recvmsg','__sys_recvfrom']):group='receive_path'
  elif any(s in stack for s in ['futex','schedule_hrtimeout','hrtimer_nanosleep','__schedule']):group='scheduler_or_wait'
  elif any(s in first for s in ['srso','retbleed','clear_cpu_buffers','verw']):group='mitigation_leaf'
  elif any(s in stack for s in ['do_syscall_64','entry_SYSCALL_64']):group='other_syscall'
  else:group='other_kernel'
  groups[group]=groups.get(group,0)+weight
 return {k:v/total*100 for k,v in groups.items()}


def tick_lag(host):
 counter=json.loads((host/'host.json').read_text(encoding='utf-8'))
 origin,records=json.loads((host/'ticks.json').read_text(encoding='utf-8'))
 rows=[(i,r) for i,r in enumerate(records) if counter['holdStartUnix']<=origin+r[0]*1e-9<counter['holdEndUnix']]
 start=[r[0]*1e-6-(i+1)*7 for i,r in rows]
 end=[(r[0]+r[1])*1e-6-(i+1)*7 for i,r in rows]
 least_start,least_end=min(start),min(end)
 return {'tickStartLagNormalizedMs':stats(x-least_start for x in start),
         'tickCompletionLagNormalizedMs':stats(x-least_end for x in end)}


if __name__=='__main__':
 import argparse
 p=argparse.ArgumentParser(description=__doc__)
 p.add_argument('host',type=Path);p.add_argument('output',type=Path)
 args=p.parse_args();result=reduce(args.host)
 if (args.host/'ticks.json').exists():result.update(tick_lag(args.host))
 if (args.host/'perf-stacks.txt').exists():result['socketKernelStackGroupsPercentOfCore']=kernel_groups(args.host/'perf-stacks.txt')
 args.output.write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
