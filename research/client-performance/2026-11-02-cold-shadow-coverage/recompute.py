"""Standard-library reductions of the bounded validation study artifacts."""
from pathlib import Path
import csv,json,statistics,sys
ROOT=Path(__file__).resolve().parent
def rows(name):
 with (ROOT/name).open(newline='',encoding='utf-8')as stream:return list(csv.DictReader(stream))
def tables():
 data=rows('runs.csv');identity=json.loads((ROOT/'identities.json').read_text(encoding='utf-8'))
 for case,arms in identity['parity'].items():
  for arm in ['control','candidate']:
   assert arms[arm]['binary_sha256']==identity['arms'][arm]['binary_sha256']
   assert (int(arms[arm]['callbacks']),int(arms[arm]['messages']),int(arms[arm]['skips']))==(600,29044 if case.startswith('join_')else 8,0)
  for key in ['command_digest','quad_digest','numeric_sha256']:assert arms['control'][key]==arms['candidate'][key]
 for r in data:
  assert int(r['callbacks'])==600 and int(r['skips'])==0
  assert int(r['messages'])==(29044 if r['case'].startswith('join_')else 8)
  assert r['binary_sha256']==identity['arms'][r['arm']]['binary_sha256']
  assert r['materials_source_sha256']==identity['arms'][r['arm']]['materials_source_sha256']
  assert r['base']==identity['base']
  assert r['numeric_sha256']==identity['parity'][r['case']][r['arm']]['numeric_sha256']
  assert (r['status'],r['reason'])==(('excluded','observed_compile')if int(r['compile_samples'])else('accepted','none'))
 assert len({(r['pair'],r['case'],r['arm'])for r in data})==len(data)
 lines=['| Case / median of complete accepted pairs | Before | After | Paired reduction range |','| --- | ---: | ---: | ---: |'];notes=[]
 for case in ['join_default','join_whole_map','rectangle_default']:
  pairs={}
  for r in data:
   if r['case']==case and r['status']=='accepted':
    pair=pairs.setdefault(r['pair'],{});assert r['arm']not in pair;pair[r['arm']]=r
  pairs=[p for p in pairs.values()if set(p)=={'control','candidate'}];assert len(pairs)>=5
  for key,label,scale,unit in [('art_wall_ns_p50','art callback p50',1e6,'ms'),('art_wall_ns_p95','art callback p95',1e6,'ms'),('art_redraw_ns_p50','art preparation p50',1e6,'ms'),('art_submit_ns_p50','art CPU submit p50',1e6,'ms'),('art_gpu_ns_p50','art GPU p50',1e6,'ms'),('cold_redraw_ns_max','cold preparation maximum',1e6,'ms'),('cold_wall_ns_max','cold callback maximum',1e6,'ms'),('wall_ns_max','10-second worst callback',1e6,'ms'),('processing_s','processing elapsed',1,'s'),('alloc_bytes','requested allocations',1e9,'GB')]:
   vals={a:[float(p[a][key])for p in pairs]for a in ['control','candidate']};d=[100*(1-float(p['candidate'][key])/float(p['control'][key]))for p in pairs]
   lines.append(f'| {case} / {label} | {statistics.median(vals["control"])/scale:.3f} {unit} | {statistics.median(vals["candidate"])/scale:.3f} {unit} | {min(d):.1f} to {max(d):.1f}% |')
  for arm in ['control','candidate']:
   selected=[p[arm]for p in pairs]
   notes.append(f'{case} {arm}: {len(selected)} runs; art p50 range {min(float(r["art_wall_ns_p50"])for r in selected)/1e6:.3f} to {max(float(r["art_wall_ns_p50"])for r in selected)/1e6:.3f} ms; art callbacks above 16.7 ms {min(int(r["art_over_budget"])for r in selected)} to {max(int(r["art_over_budget"])for r in selected)} / 300; observed load {min(float(r["load_min"])for r in selected):.1f} to {max(float(r["load_max"])for r in selected):.1f}%.')
 return '\n'.join(lines)+'\n\n'+'\n\n'.join(notes)+f'\n\nAll runs: {len(data)}; accepted {sum(r["status"]=="accepted"for r in data)}; excluded {sum(r["status"]!="accepted"for r in data)}.'
def diagnostics():
 data=rows('diagnostics.csv');lines=['| Cold-window maximum per stage / one untimed diagnostic | Join before / after ms | Rectangle before / after ms |','| --- | ---: | ---: |']
 for stage in ['redraw_ns','raster','font_load','text_raster','hud_text','section_minimap','final_validate','gpu_pack','gpu_upload','gpu_encode','gpu_ns']:
  cells=[]
  for case in ['join_default','rectangle_default']:
   values={r['arm']:float(r['max_ns'])/1e6 for r in data if r['case']==case and r['window']=='cold'and r['stage']==stage}
   cells.append(f'{values["control"]:.3f} / {values["candidate"]:.3f}')
  lines.append(f'| {stage} | '+ ' | '.join(cells)+' |')
 return '\n'.join(lines)
def rejected():
 data=rows('rejected-circle-runs.csv');identity=json.loads((ROOT/'rejected-circle-identities.json').read_text(encoding='utf-8'))
 pairs={}
 for r in data:
  arm=identity['arms'][r['arm']]
  assert r['base']==identity['base'] and r['binary_sha256']==arm['binary_sha256']
  assert r['materials_source_sha256']==arm['materials_source_sha256']and r['bank_source_sha256']==arm['bank_source_sha256']
  if r['case']=='join_default'and r['status']=='accepted':pairs.setdefault(r['pair'],{})[r['arm']]=r
 complete=[p for p in pairs.values()if len(p)==2];assert len(complete)==5
 med={a:statistics.median(float(p[a]['cold_redraw_ns_max'])/1e6 for p in complete)for a in ['control','candidate']}
 return f'Rejected circle-only trial: {len(data)} runs retained, {sum(r["status"]=="excluded"for r in data)} excluded. Five complete default-view pairs: cold preparation maximum median {med["control"]:.3f} to {med["candidate"]:.3f} ms. This is the earlier 643e4b3e composition, not the current shadow fix.'
if __name__=='__main__':
 output=tables();print(output);print(diagnostics());print(rejected())
 if '--check'in sys.argv:
  report=(ROOT/'REPORT.md').read_text(encoding='utf-8')
  assert report.split('<!-- BEGIN RESULTS -->\n',1)[1].split('\n<!-- END RESULTS -->',1)[0]==output
  assert report.split('<!-- BEGIN DIAGNOSTICS -->\n',1)[1].split('\n<!-- END DIAGNOSTICS -->',1)[0]==diagnostics()
  assert report.split('<!-- BEGIN REJECTED -->\n',1)[1].split('\n<!-- END REJECTED -->',1)[0]==rejected()
