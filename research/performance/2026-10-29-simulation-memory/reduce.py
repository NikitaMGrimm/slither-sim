import json,struct,subprocess,collections,re,hashlib,statistics,bisect
from pathlib import Path
r=Path(__file__).resolve().parent
assert (r/'DONE').exists(),'all measured windows must finish before reduction'
def write(name,value):(r/name).write_text(json.dumps(value,indent=2)+'\n')
names={1:'cycles',2:'branch-misses',3:'L1d-load-misses',5:'dTLB-load-misses',6:'generic-cache-misses',7:'frontend-stalled-cycles'}
out=[];profiles=[]
for protocol in ['stock','legacy']:
 summaries=[]
 for event,name in names.items():
  arms=[json.loads((r/f'{protocol}-e{event}-r{i}.json').read_text()) for i in range(3)]
  summaries.extend(a['summary'] for a in arms)
  for maintenance in [False,True]:
   runs=[]
   for arm in arms:
    rows=[x for x in arm['counts']['rows'] if x['maintenance']==maintenance]
    n=len(rows);ins=sum(x['counters'][0][0] for x in rows);value=sum(x['counters'][1][0] for x in rows)
    runs.append({'ticks':n,'instructionsPerTick':ins/n,'eventPerTick':value/n,'eventPerKInstructions':value/ins*1000,'cpuMsPerTick':sum(x['cpuNs'] for x in rows)/n/1e6,'minimumRunningFraction':min(v[2]/v[1] for x in rows for v in x['counters'])})
   out.append({'protocol':protocol,'event':name,'maintenance':maintenance,'runs':runs})
 assert all(s==summaries[0] for s in summaries),'fork outcomes differ'
 jobs=[(event,'initial',f'{protocol}-e{event}-r0-sample',r/'study_host') for event in [1,3,5,6]]
 if (r/'PROFILE-DONE').exists():
  jobs += [(event,'lower-rate',f'profile-{protocol}-e{event}-r0-sample',r/'study_host-profile') for event in [3,6]]
 for event,series,stem,exe in jobs:
  path=r/(stem+'.json.samples')
  data=json.loads((r/(stem+'.json')).read_text())
  assert data['summary']==summaries[0], 'profile and counter fork outcomes differ'
  meta=data['counts']['samplerCount'];assert meta[1] and meta[2]/meta[1]>=.99
  mappings=(r/(stem+'.maps')).read_text().splitlines()
  mapping=next(x.split() for x in mappings if x.split()[-1]==str(exe) and int(x.split()[2],16)==0)
  base=int(mapping[0].split('-')[0],16)
  segments=[];biases={}
  for line in mappings:
   parts=line.split()
   if len(parts)<6 or not parts[-1].startswith('/'):continue
   lo,hi=[int(x,16) for x in parts[0].split('-')];off=int(parts[2],16);file=parts[-1]
   segments.append((lo,hi,file,off));biases[file]=min(biases.get(file,lo-off),lo-off)
  hits=collections.Counter();types=collections.Counter();lost=0
  for maint,t,value in struct.iter_unpack('<QQQ',path.read_bytes()):
   types[t]+=1
   if t==9:hits[(bool(maint),value-base)]+=1
   if t==2:lost+=value
  addresses=sorted({ip for m,ip in hits if ip>=0})
  text=subprocess.run(['addr2line','-a','-f','-C','-i','-e',str(exe)],input=''.join(hex(ip)+'\n' for ip in addresses),text=True,stdout=subprocess.PIPE,check=True).stdout
  frames={};current=None;lines=[]
  def save():
   if current is not None:frames[current]=[(lines[i],lines[i+1]) for i in range(0,len(lines)-1,2)]
  for line in text.splitlines():
   if re.fullmatch(r'0x[0-9a-f]+',line):save();current=int(line,16);lines=[]
   else:lines.append(line)
  save()
  libraries={}
  for m,ip in hits:
   if frames.get(ip,[]) and frames[ip][0][0]!='??':continue
   absolute=ip+base
   segment=next((s for s in segments if s[0]<=absolute<s[1]),None)
   if not segment:continue
   file=segment[2]
   if file==str(exe):continue
   if file not in libraries:
    symbols=[]
    result=subprocess.run(['nm','-D','-S','--numeric-sort','--defined-only','--demangle',file],stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True)
    for line in result.stdout.splitlines():
     parts=line.split(maxsplit=3)
     if len(parts)==4 and parts[2].lower() in ['t','i','w']:
      symbols.append((int(parts[0],16),int(parts[1],16),parts[3].split('@')[0]))
    libraries[file]=symbols
   rel=absolute-biases[file];syms=libraries[file];ix=bisect.bisect_right([s[0] for s in syms],rel)-1
   if ix>=0 and rel<syms[ix][0]+syms[ix][1]:frames[ip]=[(Path(file).name+'::'+syms[ix][2],'external/shared-library')]
  for maint in [False,True]:
   leaves=collections.Counter();sites=collections.Counter();groups=collections.Counter();unresolved=0;total=0
   for (m,ip),count in hits.items():
    if m!=maint:continue
    total+=count;stack=frames.get(ip,[])
    if not stack or stack[0][0]=='??':unresolved+=count;leaves['unresolved/shared-library']+=count;groups['unresolved/shared-library']+=count;continue
    leaves[stack[0][0]]+=count
    source=next(((fn,loc) for fn,loc in stack if str(r/'source')+'/' in loc),None)
    if source:
     fn,loc=source;loc=loc.replace(str(r/'source')+'/','');sites[loc]+=count
     group=loc.split(':')[0];groups[group]+=count
    else:groups['allocator/standard-library/external']+=count
   profiles.append({'protocol':protocol,'event':names[event],'maintenance':maint,'series':series,'primary':not(types.get(5) or types.get(6)),'period':1000003 if event==1 else 10007 if series=='lower-rate' else 1009,'samples':total,'lost':lost,'recordTypes':dict(types),'samplerCounter':meta,'unresolved':unresolved,'leafFunctions':[{'function':fn,'samples':n} for fn,n in leaves.most_common(16)],'sourceSites':[{'site':fn,'samples':n} for fn,n in sites.most_common(16)],'sourceGroups':[{'source':fn,'samples':n} for fn,n in groups.most_common()]})
write('counter-results.json',out);write('profile-results.json',profiles)
footprints=[]
for protocol in ['stock','legacy']:
 f=json.loads((r/(protocol+'-footprint.json')).read_text());known=f.pop('knownSnakes');interest=f.pop('interests');sizes=f['sizes']
 f['protocol']=protocol
 f['knownSnakes']={'denseLengths':sum(x[0] for x in known),'denseCapacities':sum(x[1] for x in known),'occupied':sum(x[2] for x in known),'slotCapacities':sum(x[3] for x in known),'denseCapacityBytes':sum(x[1] for x in known)*sizes['optionKnownSnake'],'occupiedPayloadBytes':sum(x[2] for x in known)*sizes['knownSnake'],'slotCapacityBytes':sum(x[3] for x in known)*8}
 f['interests']={'subscribed':sum(x[0] for x in interest),'lastSeen':sum(x[1] for x in interest),'lastSeenPayloadBytes':sum(x[1] for x in interest)*10,'subscribedPayloadBytes':sum(x[0] for x in interest)*2}
 footprints.append(f)
write('footprint-results.json',footprints)
for row in out:
 if row['event'] not in ['cycles','L1d-load-misses','dTLB-load-misses','generic-cache-misses','branch-misses']:continue
 a=row['runs'];print(row['protocol'],row['event'],'maintenance' if row['maintenance'] else 'ordinary', 'perTick',round(statistics.median(x['eventPerTick'] for x in a),2),'MPKI',round(statistics.median(x['eventPerKInstructions'] for x in a),3))
for p in profiles:
 print(p['protocol'],p['event'],p['maintenance'],'samples',p['samples'],'lost',p['lost'],'top',p['sourceGroups'][:5])
