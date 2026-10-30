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
print('\n| Run | Ticks | Count | World mean / p99 ms | Simulation mean ms | Socket mean ms |')
print('| --- | --- | ---: | ---: | ---: | ---: |')
for run in data['runs']:
 for label in ['all','maintenance','ordinary']:
  r=run['result'] if label=='all' else run['result'][label]
  print(f"| {run['run']} | {label} | {r['ticks']} | {r['worldCpuMsPerTick']['mean']:.3f} / {r['worldCpuMsPerTick']['p99']:.3f} | {r['simulationCpuMsPerTick']['mean']:.3f} | {r['socketCpuMsPerTick']['mean']:.3f} |")
print('\n| Run | Component | CPU p50 / p99 / p99.9 / max ms |')
print('| --- | --- | ---: |')
for run in data['runs']:
 for label,key in [('Simulation/broadcast','simulationCpuMsPerTick'),('Socket I/O','socketCpuMsPerTick')]:
  q=run['result'][key];print(f"| {run['run']} | {label} | {' / '.join(f'{q[k]:.3f}' for k in ['p50','p99','p999','max'])} |")
