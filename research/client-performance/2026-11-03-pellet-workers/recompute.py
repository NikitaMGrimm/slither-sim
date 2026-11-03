"""Recompute bounded local-only study summaries with Python stdlib; no Git or recording reads."""
import csv
import json
import re
import statistics as stats
import sys
from pathlib import Path
ROOT=Path(__file__).resolve().parent
ARMS=['serial','pool']
EXPECTED_PARITY={f'normal/{scene}-{adapter}-{size}-{quality}'for scene in ['replay','rectangle']for adapter,size in [('rtx','3840x2160'),('igpu','3840x2160'),('igpu','1920x1080')]for quality in ['original','native']if scene!='rectangle'or size!='1920x1080'}|{f'{variant}/replay-rtx-3840x2160-{quality}'for variant in ['serial-fallback','late-20ms']for quality in ['original','native']}|{f'reconnect/{quality}'for quality in ['original','native']}
def rows(name):
 with(ROOT/name).open(newline='',encoding='utf-8')as f:return list(csv.DictReader(f))
def median(rs,k):return stats.median(float(r[k])for r in rs)
def shape(v,n):return isinstance(v,str)and re.fullmatch('[0-9a-f]{'+str(n)+'}',v)is not None
def validate(runs,parity,ids):
 pairs={}
 assert ids['serial']['binary_sha256']!=ids['pool']['binary_sha256']
 for r in runs:
  assert(int(r['callbacks']),int(r['messages']),int(r['skips']))==(600,29044,0)
  assert r['binary_sha256']in ids['timing_binary_arms'][r['cohort']][r['arm']]
  if r['cohort']=='final':assert r['binary_sha256']==ids[r['arm']]['binary_sha256']
  assert r['canonical_active']=='False' # Inactive output is never a parity gate.
  assert shape(r['command_digest'],16)
  assert shape(r['frames_sha256'],64)and shape(r['load_sha256'],64)
  assert(r['status']=='accepted')==(int(r['compile_samples'])==0)
  key=tuple(r[k]for k in ['cohort','condition','round','adapter','physical','quality'])
  pair=pairs.setdefault(key,{})
  assert r['arm']not in pair;pair[r['arm']]=r
 for key,pair in pairs.items():
  assert set(pair)==({'base','serial','pool'}if key[:2]==('initial','adoption')else set(ARMS))
  accepted=all(r['status']=='accepted'for r in pair.values())
  assert all((r['paired']=='True')==accepted for r in pair.values())
  assert len({r['command_digest']for r in pair.values()})==1
 final=parity['final'];assert set(final)==EXPECTED_PARITY
 for key,r in final.items():
  assert r['callbacks']==r['baseline_callbacks']==600
  assert r['skips']==r['baseline_skips']==0
  expected=ids['serial']['binary_sha256']if key.startswith('reconnect/')else ids['image_baseline_binary_sha256']
  assert r['baseline_binary_sha256']==expected
  assert r['binary_sha256']==ids['pool']['binary_sha256']!=expected
  assert shape(r['image_sequence_sha256'],64)
  assert r['image_sequence_sha256']==r['baseline_image_sequence_sha256']
 return pairs

def tables(runs,idle,density,callbacks):
 selected=[r for r in runs if r['cohort']=='final'and r['paired']=='True']
 def group(condition,quality,adapter='rtx',size='3840x2160'):
  return[r for r in selected if(r['condition'],r['quality'],r['adapter'],r['physical'])==(condition,quality,adapter,size)]
 def cell(rs):return f"{median(rs,'wall_ns_p50_ms'):.3f}/{median(rs,'wall_ns_p95_ms'):.3f}"
 text=['| Adapter / physical size / quality | Pairs | Serial art median/p95 ms | Pool art median/p95 ms | Paired p95 delta median [range] ms |','| --- | ---: | ---: | ---: | ---: |']
 for adapter,size in [('rtx','3840x2160'),('igpu','1920x1080'),('igpu','3840x2160')]:
  for quality in ['original','native']:
   rs=group('default'if adapter=='rtx'else'guard',quality,adapter,size);assert len(rs)>=10
   parts=[[r for r in rs if r['arm']==arm]for arm in ARMS]
   a={r['round']:r for r in parts[0]};d=[float(r['wall_ns_p95_ms'])-float(a[r['round']]['wall_ns_p95_ms'])for r in parts[1]]
   text.append(f'| {adapter} / {size} / {quality} | {len(rs)//2} | '+ ' | '.join(map(cell,parts))+f' | {stats.median(d):+.3f} [{min(d):+.3f}, {max(d):+.3f}] |')
 text+=['','| RTX 4K / quality / arm | Art median range ms | Whole median/p95 ms | CPU stages ms | Pellet prepare ms | Submit ms | GPU ms | Rust requests / GB | Host load median [range] % |','| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |']
 for quality in ['original','native']:
  for arm in ARMS:
   rs=[r for r in group('default',quality)if r['arm']==arm];values=[float(r['wall_ns_p50_ms'])for r in rs]
   cells=[f'{min(values):.3f}-{max(values):.3f}',f"{median(rs,'whole_p50_ms'):.3f}/{median(rs,'whole_p95_ms'):.3f}"]+[f'{median(rs,k):.3f}'for k in ['cpu_ns_p50_ms','pellet_prepare_p50_ms','submit_ns_p50_ms','gpu_ns_p50_ms']]+[f"{median(rs,'whole_process_requests'):.0f} / {median(rs,'whole_process_bytes')/1e9:.3f}",f"{median(rs,'load_p50'):.1f} [{min(float(r['load_min'])for r in rs):.1f}, {max(float(r['load_max'])for r in rs):.1f}]"]
   text.append(f'| {quality} / {arm} | '+' | '.join(cells)+' |')
 text+=['','| Stress / quality | Pairs | Serial art median/p95 ms | Pool art median/p95 ms | Serial/pool whole p95 ms |','| --- | ---: | ---: | ---: | ---: |']
 for condition in ['late20','busy8','disabled']:
  for quality in ['original','native']:
   rs=group(condition,quality)
   if not rs:continue
   assert len(rs)>=4;parts=[[r for r in rs if r['arm']==arm]for arm in ARMS]
   text.append(f'| {condition} / {quality} | {len(rs)//2} | '+' | '.join(map(cell,parts))+' | '+'/'.join(f"{median(p,'whole_p95_ms'):.3f}"for p in parts)+' |')
 text+=['','| Final paced idle mode | Serial/pool process CPU % of one core | Serial/pool CPU ms per callback | Pool threads / observed idle CPU seconds |','| --- | ---: | ---: | ---: |']
 for mode in ['menu','sparse','after-dense']:
  rs=[r for r in idle if r['cohort']=='final'and r['mode']==mode and r['status']=='accepted'];parts=[[r for r in rs if r['arm']==arm]for arm in ARMS];assert all(len(p)>=2 for p in parts)
  text.append(f'| {mode} | '+' / '.join(f"{median(p,'one_core_percent'):.3f}"for p in parts)+' | '+' / '.join(f"{median(p,'cpu_ms_per_callback'):.3f}"for p in parts)+f" | {median(parts[1],'pellet_threads'):.0f} / {max(float(r['pellet_idle_cpu_s'])for r in parts[1]):.6f} |")
 text+=['','| Visible / settled | Serial preparation ms | Two participants ms | Four participants ms |','| --- | ---: | ---: | ---: |']
 for count in ['1024','2048','4096','8192']:
  for settled in ['false','true']:
   rs=[r for r in density if(r['density'],r['settled'])==(count,settled)]
   text.append(f'| {count} / {settled} | '+' | '.join(f"{median([r for r in rs if r['participants']==n],'p50_ns')/1e6:.4f}"for n in ['1','2','4'])+' |')
 text+=['','| First final pair / quality | Source rebuilds / 300 art callbacks | Slowest-16 source / ingress / redraw median ms | Slowest-30 whole callbacks: cold / dispatched |','| --- | ---: | ---: | ---: |']
 for quality in ['original','native']:
  rs=[r for r in callbacks if r['cohort']=='final'and r['arm']=='pool'and r['quality']==quality];assert len(rs)==600
  art=[r for r in rs if 120<=int(r['callback'])<420];slow=sorted(art,key=lambda r:int(r['wall_ns']),reverse=True)[:16];whole=sorted(rs,key=lambda r:int(r['wall_ns']),reverse=True)[:30]
  cells='/'.join(f'{median(slow,k)/1e6:.3f}'for k in ['pellet_sources_ns','ingress_ns','redraw_ns'])
  text.append(f"| {quality} | {sum(int(r['pellet_sources_ns'])>0 for r in art)} | {cells} | {sum(int(r['callback'])<120 for r in whole)} / {sum(int(r['attempt'])>0 for r in whole)} |")
 return '\n'.join(text)
def main():
 runs=rows('runs.csv');parity=json.loads((ROOT/'parity.json').read_text(encoding='utf-8'));ids=json.loads((ROOT/'identities.json').read_text(encoding='utf-8'));validate(runs,parity,ids)
 text=tables(runs,rows('idle.csv'),rows('density.csv'),rows('callback-stages.csv'))
 if '--check'in sys.argv:
  assert text in(ROOT/'REPORT.md').read_text(encoding='utf-8'), 'report tables differ from retained summaries'
  print('Bounded runs, binary identities, 9600 final image pairs and report tables verified')
 else:print(text)
if __name__=='__main__':main()