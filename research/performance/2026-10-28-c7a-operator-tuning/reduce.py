"""Small diagnostic reductions; #201 remains the delivery gate and frame reducer."""
import json
from pathlib import Path


def resources(path):
    rows = json.loads(Path(path).read_text())['rows']
    a = next(r for r in rows if r.get('boundary') == 'start')
    b = next(r for r in rows if r.get('boundary') == 'end')
    elapsed = b['unix'] - a['unix']
    process = {k: sum(b['processes'][pid][k+'Seconds']-v[k+'Seconds']
                      for pid, v in a['processes'].items())/elapsed*100 for k in ['user', 'system']}
    threads = {}
    for pid, ts in a['threads'].items():
        for tid, v in ts.items():
            last = b['threads'][pid][tid]
            threads[v['name']] = {k: (last[k+'Seconds']-v[k+'Seconds'])/elapsed*100 for k in ['user', 'system']}
            threads[v['name']].update(niceStart=v['nice'], niceEnd=last['nice'])
    return {'windowSeconds': elapsed, 'processCpuPercent': process, 'threadCpuPercent': threads,
            'networkIrqsPerSecond': (b['networkIrqs']-a['networkIrqs'])/elapsed,
            'totalIrqsPerSecond': (b['totalIrqs']-a['totalIrqs'])/elapsed}


def grade_arm(rows):
    if len(rows) != 2 or {r['rep'] for r in rows} != {1, 2}:
        return 'incomplete'
    return 'pass' if all(r['valid'] and r['gatePass'] for r in rows) else 'miss'
