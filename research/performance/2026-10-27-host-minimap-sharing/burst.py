"""Linux x86-64: instrument a fresh source archive, or --reduce a capture."""
import json
from pathlib import Path
import shutil
import statistics
import subprocess
import sys

if sys.argv[1] == '--reduce':
    out = Path(sys.argv[2])
    host = json.loads((out / 'host.json').read_text())
    rows = [r for r in json.loads((out / 'ticks.json').read_text())
            if host['holdStartUnix'] <= r['unix'] < host['holdEndUnix']]
    assert rows and all(r['enabled'] > 0 and r['running'] / r['enabled'] >= .99 for r in rows)
    for due in [True, False]:
        group = [r for r in rows if r['due'] == due]
        print('due', due, 'ticks', len(group))
        for key in ['instructions', 'cpuNs', 'wallNs', 'mapInstructions', 'mapCpuNs']:
            values = [r[key] for r in group]
            print(key, 'mean/median/max', statistics.mean(values), statistics.median(values), max(values))
        print('map instruction share', sum(r['mapInstructions'] for r in group) / sum(r['instructions'] for r in group))
    sys.exit(0)

source = Path(sys.argv[1]).resolve()
shutil.copyfile(Path(__file__).with_name('burst_instrument.rs'), source / 'src/client/burst_instrument.rs')
path = source / 'src/client/mod.rs'
path.write_text(path.read_text() + '\nmod burst_instrument;\n')
for name, before, addition in [
    ('server.rs', '            for _ in 0..due {\n',
     '                let study_tick = super::burst_instrument::Tick::start();\n'),
    ('server.rs', '                self.hand_over()?;\n',
     '                study_tick.finish(self.ticks, self.waiting.len());\n'),
    ('simulation/broadcast.rs', '        let leaderboard_due = clock_due(\n',
     '        crate::client::burst_instrument::due(minimap_due);\n'),
    ('simulation/broadcast.rs', '    ) -> Result<Option<Vec<u8>>, String> {\n',
     '        let _study_map = crate::client::burst_instrument::Map::start();\n'),
]:
    path = source / 'src/client' / name
    text = path.read_text()
    assert text.count(before) == 1
    after = addition + before if 'leaderboard_due' in before else before + addition
    path.write_text(text.replace(before, after))
prior = source / 'research/performance/2026-10-26-host-capacity'
shutil.copyfile(prior / 'host_probe.rs', source / 'examples/host_probe.rs')
subprocess.run(['patch', '-o', str(source / 'examples/host_capacity_load.rs'),
    str(prior / 'host_capacity_load.rs'), str(source /
    'research/performance/2026-10-26-host-known-slots/host-players.patch')], check=True)
text = (source / 'research/performance/2026-10-26-host-known-slots/loopback.py').read_text()
text = text.replace('18766', '18769').replace('time.time() + 1', 'time.time() + 2')
text = text.replace("RAYON_NUM_THREADS='2'", "RAYON_NUM_THREADS='1'")
(source / 'study_loopback.py').write_text(text)
