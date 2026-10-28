"""Resolve every counted libm return address to its repository callers."""
import csv,json,subprocess,sys
from collections import defaultdict
from pathlib import Path
root=Path(sys.argv[1]).resolve();session=root/'sessions'/sys.argv[2]
def owner(stack,scope):
 text='\n'.join(r['function'] for r in stack)
 if 'snakes::covered' in text:return 'covered segments' if 'segment_distance' in text else 'covered head'
 if 'snakes::changes' in text:return 'incremental anchors'
 if 'encode::appearance' in text:return 'appearances'
 if 'SectorInterest' in text:return 'food-interest heading'
 if 'physics::advance_directed' in text:return 'physics advance_directed'
 if 'physics::head_tip' in text:return 'physics head-tip'
 if 'advance_prey' in text:return 'physics prey'
 if 'physics::strikes_scaled' in text:return 'physics collision distance'
 if 'KioskState' in text:return 'physics kiosk distance'
 if 'find_eater' in text:return 'physics pickup distance'
 if 'boundary::constrain' in text:return 'physics boundary'
 if 'SimulatorBackend>::updates' in text and ('Point>::distance' in text or 'Point::distance' in text):return 'prey visibility'
 return stack[0]['function'] if stack else 'unresolved executable caller'
result={}
for arm in (['base','candidate'] if '--plain' in sys.argv[3:] else ['base-profile','candidate-profile']):
 file=session/(arm+'-parent.csv')
 with file.open() as f:
  calibration=f.readline().strip();assert calibration.startswith('# controls=')
  rows=list(csv.DictReader(f))
 addresses=[hex(int(r['offset'],16)-1) for r in rows]
 text=subprocess.check_output(['addr2line','-a','-f','-C','-i','-e',str(root/arm/'bin/study_host'),*addresses],text=True)
 stacks=[];current=None;it=iter(text.splitlines())
 for line in it:
  if line.startswith('0x'):current=[];stacks.append(current)
  else:
   source=next(it)
   if 'slither_sim' in line and str(root/arm/'source/src')+'/' in source:current.append({'function':line,'source':'src/'+source.rsplit('/src/',1)[-1]})
 assert len(stacks)==len(rows)
 selected=[];totals=defaultdict(lambda:{'calls':0,'instructions':0})
 for r,stack in zip(rows,stacks):
  group=owner(stack,int(r['scope']));calls=int(r['calls']);instructions=int(r['callee_instructions']);totals[group]['calls']+=calls;totals[group]['instructions']+=instructions
  selected.append({'offset':r['offset'],'scope':int(r['scope']),'function':r['function'],'calls':calls,'rawInstructions':int(r['instructions']),'calleeInstructions':instructions,'owner':group,'stack':stack})
 result[arm]={'calibration':calibration,'calls':selected,'totals':dict(sorted(totals.items(),key=lambda x:-x[1]['instructions']))}
 print(arm,json.dumps(result[arm]['totals']),flush=True)
(session/'callers-selected.json').write_text(json.dumps(result,indent=2)+'\n')
