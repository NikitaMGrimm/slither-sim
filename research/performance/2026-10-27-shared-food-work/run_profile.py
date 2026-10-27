"""Run the already-built main whole-tick diagnostic and verify full traces."""
import hashlib
import json
import os
from pathlib import Path
import statistics
import subprocess
import sys
if not __debug__:
    raise SystemExit('Run without optimization')
root=Path(sys.argv[1]).resolve();control=Path(sys.argv[2]).resolve()
os.sched_setaffinity(0,{int(os.environ.get('TICK_CPU','3'))})
binary=root/'bin/study_protocols'
pins=json.loads((root/'pins.json').read_text())
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
assert sha(binary)==pins['binarySha256'] and sha(control)==pins['controlSha256']
env=dict(os.environ,RAYON_NUM_THREADS='1',RUST_TEST_THREADS='1')
selected={};ids=None
for case in ['official500','official500-half','official500-legacy']:
    path=root/(case+'.phases.json')
    with (root/(case+'.log')).open('w') as log:
        subprocess.run([str(binary),case],env=dict(env,STUDY_PHASES=str(path)),stdout=log,stderr=subprocess.STDOUT,check=True)
    data=json.loads(path.read_text());assert len(data['phases']) in (32,37)
    rows=[r for r in data['rows'] if r['tick']>=120 and r['tick']!=720 and r['playing']==500]
    current=[r['tick'] for r in rows];ids=current if ids is None else ids
    assert len(rows)==853 and current==ids
    assert all(r['enabled']>0 and r['running']/r['enabled']>=.99 for r in rows)
    summary=json.loads((root/(case+'.log')).read_text().splitlines()[-1])
    for label,executable in [('profile',binary),('control',control)]:
        with (root/f'{case}-{label}.trace').open('xb') as out,(root/f'{case}-{label}.stderr').open('x') as err:
            subprocess.run([str(executable),case,'--trace'],env=dict(env,STUDY_PHASES=str(root/(case+'-control-phases.json'))),stdout=out,stderr=err,check=True)
        assert json.loads((root/f'{case}-{label}.stderr').read_text().splitlines()[-1])==summary
    with (root/f'{case}-profile.trace').open('rb') as left,(root/f'{case}-control.trace').open('rb') as right:
        while True:
            a,b=left.read(1<<20),right.read(1<<20);assert a==b
            if not a:break
    phases={name:statistics.fmean(r['phases'][i] for r in rows) for i,name in enumerate(data['phases'])}
    calls={name:statistics.fmean(r['calls'][i] for r in rows) for i,name in enumerate(data['phases'])}
    total=statistics.fmean(r['instructions'] for r in rows)
    top=['ingest','actions','world_step','food_events','settle','advance_interest','processing_overrides','broadcast','client_completion','background_decisions']
    prep=['reveal_rebuild','reports','broadcast_clock_setup','broadcast_interest','shared_food','prey_ids','prey_clocks','bounds','grid','prey_bounds','recipient_envelope']
    update=['updates_prefix','candidates','snake_updates','recipient_food','recipient_prey']
    recipient=['board','updates','map','hud','framing']
    residuals={'advance_glue_and_probes':total-sum(phases[n] for n in top),
               'broadcast_shared_setup_drops_and_probes':phases['broadcast']-sum(phases[n] for n in prep),
               'updates_glue_drops_and_nested_probes':phases['updates']-sum(phases[n] for n in update),
               'recipient_assembly_drops_and_nested_probes':phases['recipient_envelope']-sum(phases[n] for n in recipient)}
    selected[case]={'traceEqual':True,'ticks':len(rows),'instructions':total,'phases':phases,'calls':calls,'residuals':residuals,
                    'minimumScheduledPct':min(100*r['running']/r['enabled'] for r in rows),'pins':pins}
    (root/'selected.json').write_text(json.dumps(selected,indent=2)+'\n')
    print('WHOLE TICK',case,json.dumps(selected[case]),flush=True)
assert sha(binary)==pins['binarySha256'] and sha(control)==pins['controlSha256']
