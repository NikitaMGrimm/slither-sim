"""Bounded release-pair and picture controls; stdlib only, no private input."""
import csv,json,re,statistics as st,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parent
ARMS=('base','candidate')
PARITY={f'{a}/{s}/n{n}/{q}'for a,s,ns in [('rtx','3840x2160',[-1,-4,-1000,0]),('igpu','1920x1080',[-4]),('igpu','3840x2160',[-4])]for n in ns for q in ['original','native']}
def rows(name):
 with(ROOT/name).open(newline='',encoding='utf-8')as f:return list(csv.DictReader(f))
def med(rs,k):return st.median(float(r[k])for r in rs)
def shape(v,n):return isinstance(v,str)and re.fullmatch('[0-9a-f]{'+str(n)+'}',v)is not None
def validate(runs,parity,canonical,ids):
 assert len({ids[a]['binary_sha256']for a in ARMS})==2
 assert len({ids[a]['driver_sha256']for a in ARMS})==1
 groups={}
 for r in runs:
  assert r['cohort']=='final'and r['arm']in ARMS
  assert r['sample_id'].startswith('zoom-current-')
  arm=r['arm']
  assert r['binary_sha256']==ids[arm]['binary_sha256']
  assert(int(r['callbacks']),int(r['messages']),int(r['skips']))==(600,29044,0)
  expected=300 if int(r['zoom_notches'])==0 or(r['arm']=='candidate'and int(r['zoom_notches'])in[-1,-4])else 0
  assert int(r['retained_art_callbacks'])==expected
  assert r['status']in ['accepted','compile_overlap']
  assert(r['status']=='accepted')==(int(r['compile_samples'])==0)
  assert r['reason']==('none'if r['status']=='accepted'else'observed rustc/link overlap')
  assert shape(r['sent_digest'],16)and all(shape(r[k],64)for k in ['frames_sha256','load_sha256','binary_sha256'])
  key=tuple(r[k]for k in ['cohort','round','adapter','physical','quality','zoom_notches'])
  pair=groups.setdefault(key,{})
  assert r['arm']not in pair;pair[r['arm']]=r
 for pair in groups.values():
  selected=set(pair)==set(ARMS)and all(r['status']=='accepted'for r in pair.values())
  assert all((r['paired']=='True')==selected for r in pair.values())
  if len(pair)==2:assert len({r['sent_digest']for r in pair.values()})==1
 assert set(parity)==PARITY
 for record in parity.values():
  assert set(record['arms'])==set(ARMS)
  for a,r in record['arms'].items():
   assert(r['callbacks'],r['skips'])==(600,0)and r['binary_sha256']==ids[a]['binary_sha256']
   assert shape(r['image_sequence_sha256'],64)
  equal=record['arms']['base']['image_sequence_sha256']==record['arms']['candidate']['image_sequence_sha256']
  assert record['exact']==equal
  if not equal:assert 0<record['canonical_max_rgb']<=2
 assert set(canonical)=={'1920x1080','3840x2160'}
 for arms in canonical.values():
  assert set(arms)==set(ARMS)
  for a,r in arms.items():
   assert(r['callbacks'],r['skips'])==(600,0)and r['binary_sha256']==ids[a]['binary_sha256']
   assert r['canonical_quads']==48556857
   assert shape(r['quad_digest'],16)and r['quad_digest']!='d1fba762150c532c'
   assert shape(r['canonical_and_send_digest'],16)
  for k in ['canonical_quads','quad_digest','canonical_and_send_digest']:assert arms['base'][k]==arms['candidate'][k]
 return groups
def tables(runs):
 rs=[r for r in runs if r['cohort']=='final'and r['paired']=='True']
 groups=[(a,s,n,q)for a,s,ns in [('rtx','3840x2160',[-1,-4,-1000,0]),('igpu','1920x1080',[-4]),('igpu','3840x2160',[-4])]for n in ns for q in ['original','native']]
 text=['| Adapter / size / notches / quality | Pairs | Base art median/p95 ms | Retained art median/p95 ms | Paired p95 reduction median [range] ms |','| --- | ---: | ---: | ---: | ---: |']
 for a,s,n,q in groups:
  part=[[r for r in rs if(r['adapter'],r['physical'],int(r['zoom_notches']),r['quality'],r['arm'])==(a,s,n,q,arm)]for arm in ARMS]
  assert len(part[0])==len(part[1])>=(5 if a=='rtx'else 3)
  base={r['round']:r for r in part[0]};d=[float(base[r['round']]['wall_ns_p95_ms'])-float(r['wall_ns_p95_ms'])for r in part[1]]
  text.append(f'| {a} / {s} / {n} / {q} | {len(part[0])} | '+' | '.join(f"{med(p,'wall_ns_p50_ms'):.3f}/{med(p,'wall_ns_p95_ms'):.3f}"for p in part)+f' | {st.median(d):+.3f} [{min(d):+.3f}, {max(d):+.3f}] |')
 text+=['','| RTX 4K / notches / quality / arm | Food plan ms | Pellet prepare ms | All submit ms | CPU scopes ms | GPU / glow ms | Whole median/p95 ms | Process Rust GB | Load median [range] % |','| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |']
 for n in [-1,-4,-1000,0]:
  for q in ['original','native']:
   for arm in ARMS:
    p=[r for r in rs if(r['adapter'],int(r['zoom_notches']),r['quality'],r['arm'])==('rtx',n,q,arm)]
    text.append(f'| {n} / {q} / {arm} | '+' | '.join(f'{med(p,k):.3f}'for k in ['food_plan_p50_ms','pellet_prepare_p50_ms','submit_ns_p50_ms','cpu_ns_p50_ms'])+f" | {med(p,'gpu_ns_p50_ms'):.3f}/{med(p,'glow_ns_p50_ms'):.3f} | {med(p,'whole_p50_ms'):.3f}/{med(p,'whole_p95_ms'):.3f} | {med(p,'whole_process_bytes')/1e9:.3f} | {med(p,'load_p50'):.1f} [{min(float(r['load_min'])for r in p):.1f}, {max(float(r['load_max'])for r in p):.1f}] |")
 text+=['','| AMD / size / quality (-4) | Base CPU / GPU ms | Retained CPU / GPU ms | Base / retained process Rust GB |','| --- | ---: | ---: | ---: |']
 for size in ['1920x1080','3840x2160']:
  for q in ['original','native']:
   p=[[r for r in rs if r['adapter']=='igpu'and r['physical']==size and r['quality']==q and r['arm']==arm]for arm in ARMS]
   text.append(f'| {size} / {q} | '+' | '.join(f'{med(a,"cpu_ns_p50_ms"):.3f}/{med(a,"gpu_ns_p50_ms"):.3f}'for a in p)+f' | {med(p[0],"whole_process_bytes")/1e9:.3f}/{med(p[1],"whole_process_bytes")/1e9:.3f} |')
 text+=['','| Candidate RTX 4K / notches / quality, slowest 16 art callbacks | Wall ms | Ingress ms | Food advance / clone ms | Source rebuild ms | Pellet prepare ms | Submit ms | GPU ms |','| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |']
 for n in [-1,-4,-1000,0]:
  for q in ['original','native']:
   rounds={r['round']for r in rs if r['adapter']=='rtx'and int(r['zoom_notches'])==n and r['quality']==q and r['arm']=='candidate'}
   p=[r for r in rows('tail-stages.csv')if int(r['zoom_notches'])==n and r['quality']==q and r['status']=='accepted'and r['round']in rounds]
   text.append(f'| {n} / {q} | {med(p,"wall_ms"):.3f} | {med(p,"ingress_ms"):.3f} | {med(p,"food_advance_ms"):.3f}/{med(p,"food_clone_ms"):.3f} | {med(p,"source_ms"):.3f} | {med(p,"prepare_ms"):.3f} | {med(p,"submit_ms"):.3f} | {med(p,"gpu_ms"):.3f} |')
 return '\n'.join(text)
def main():
 runs=rows('runs.csv');validate(runs,json.loads((ROOT/'parity.json').read_text(encoding='utf-8')),json.loads((ROOT/'canonical-controls.json').read_text(encoding='utf-8')),json.loads((ROOT/'identities.json').read_text(encoding='utf-8')))
 text=tables(runs)
 if '--check'in sys.argv:
  assert text in(ROOT/'REPORT.md').read_text(encoding='utf-8'),'tables differ'
  print('Run selection, binary/driver identities, 7200 image controls and active canonical controls verified')
 else:print(text)
if __name__=='__main__':main()
