"""Select anonymous inline-source IP samples; raw maps remain private."""
import csv,json,re,subprocess,sys
from pathlib import Path
from collections import Counter
r=Path(sys.argv[1]);session=r/'sessions'/sys.argv[2]
exe=(r/'base-profile/bin/study_host').resolve()
result={}
for path in sorted(session.glob('sample-*.samples')):
 maps=path.with_suffix('.maps').read_text();regions=[]
 for line in maps.splitlines():
  fields=line.split(maxsplit=5)
  if len(fields)==6:
   start,end=(int(x,16) for x in fields[0].split('-'))
   regions.append((start,end,int(fields[2],16),fields[5]))
 with path.open() as f:
  header=f.readline().strip();rows=list(csv.DictReader(f))
 elf_type=int.from_bytes(exe.read_bytes()[16:18],'little');assert elf_type==3, 'sampler recipe expects a PIE executable'
 image_base=min(start for start,end,offset,name in regions if name==str(exe) and offset==0)
 counts=Counter();external=Counter();total=0
 for row in rows:
  ip=int(row['ip'],16);w=int(row['period']);total+=w
  region=next((v for v in regions if v[0]<=ip<v[1]),None)
  if region is None:
   external['unmapped IP']+=w
   continue
  start,end,offset,name=region
  if name==str(exe):counts[ip-image_base]+=w
  else:external[Path(name).name]+=w
 assert sum(counts.values())+sum(external.values())==total
 addresses=sorted(counts)
 text=subprocess.check_output(['addr2line','-a','-f','-C','-i','-e',str(exe)],input=''.join(f'{p:x}\n' for p in addresses),text=True)
 (session/(path.stem+'-symbols.txt')).write_text(text)
 stacks={};current=None;it=iter(text.splitlines())
 for line in it:
  if line.startswith('0x'):current=int(line,16);stacks[current]=[]
  else:
   source=next(it)
   if '/src/' in source:source='src/'+source.rsplit('/src/',1)[1]
   elif '/examples/' in source:source='examples/'+source.rsplit('/examples/',1)[1]
   else:source=Path(source).name
   stacks[current].append({'function':line,'source':source})
 assert sorted(stacks)==addresses
 groups=Counter();locations=Counter()
 for ip,weight in counts.items():
  stack=stacks[ip];names='\n'.join(v['function'] for v in stack)
  if 'BodyBounds' in names:group='body bounds construction'
  elif 'snakes::covered' in names:
   if 'closest_on_segment' in names or 'segment_distance_le' in names:group='coverage projection/exact edge tests'
   elif 'distance_' in names:group='coverage exact distance comparison'
   else:group='coverage bounds/query bookkeeping'
  elif 'strikes_scaled' in names or 'segments_intersect' in names:group='collision exact tests'
  elif 'BoundsIndex' in names:group='bounds index queries/builds (shared)'
  elif 'compact_food' in names:group='food compaction'
  else:group='other executable'
  groups[group]+=weight
  if group not in ['other executable','bounds index queries/builds (shared)']:
   first=next((v for v in stack if v['source'].startswith('src/')),None)
   if first:locations[(group,first['function'],first['source'])]+=weight
 result[path.stem]={'header':header,'samples':len(rows),'weightedInstructions':total,
   'groups':{k:{'weight':v,'share':v/total} for k,v in groups.most_common()},
   'external':dict(external),'selectedLocations':[{'group':g,'function':fn,'source':src,'weight':v,'share':v/total} for (g,fn,src),v in locations.most_common(35)]}
(session/'samples-selected.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:{'samples':v['samples'],'groups':v['groups'],'external':v['external']} for k,v in result.items()},indent=2))
