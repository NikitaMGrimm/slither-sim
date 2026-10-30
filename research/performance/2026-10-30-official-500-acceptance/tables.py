"""Print only report tables from the bounded published aggregates."""
from pathlib import Path
import json,sys
data=json.loads(Path(sys.argv[1]).read_text())
print('| Run | Pass | Deferred | World CPU p50 / p99 / p99.9 / max ms | Maintenance / other mean ms | End mean / p90 / p99 length |')
print('| --- | --- | ---: | ---: | ---: | ---: |')
for run in data['runs']:
 r=run['result'];q=r['worldCpuMsPerTick'];c=r['endpointCensus']['fullWindowAfter']
 print(f"| {run['run']} | {r['passes']} | {r['deferredTicks']} | {' / '.join(f'{q[k]:.3f}' for k in ['p50','p99','p999','max'])} | {r['maintenance']['worldCpuMsPerTick']['mean']:.3f} / {r['ordinary']['worldCpuMsPerTick']['mean']:.3f} | {c['meanSegments']:.3f} / {c['p90Segments']} / {c['p99Segments']} |")
print('\n| Run | Gap p99 ms | Header-gap residual p99 ms | Frame excess p99 ms | Steal % |')
print('| --- | ---: | ---: | ---: | ---: |')
for run in data['runs']:
 r=run['result'];c=r['client'];print(f"| {run['run']} | {c['frameGapMs']['p99']:.3f} | {c['gapResidualMs']['p99']:.3f} | {c['frameExcessMs']['p99']:.3f} | {r['stealPercent']:.4f} |")
