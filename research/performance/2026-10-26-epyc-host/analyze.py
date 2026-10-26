"""Reduce retained EPYC socket trials using the original capacity timing/grade code."""
import argparse
import importlib.util
import json
import math
from pathlib import Path

_spec = importlib.util.spec_from_file_location(
    'capacity_analysis', Path(__file__).parent.parent/'2026-10-26-host-capacity/analyze.py')
_base = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_base)
reduce_trial, stats = _base.reduce_trial, _base.stats


def reduce(host, load, count):
    r = reduce_trial(host, load, count)
    r['cpuMsPerExecutedTick'] = (r['hostResources']['cpuPercentOneCore']/100
                                * r['hostResources']['windowSeconds']*1000/r['executedTicks'])
    telemetry = json.loads((host/'telemetry.json').read_text())
    r['peakRssBytes'] = max(v['hwmBytes'] for row in telemetry['rows']
                            for v in row['processes'].values() if v)
    waits, all_waits, pending = [], [], []
    final = {}
    closes = {}
    for p in sorted(load.glob('load-*.json')):
        shard = json.loads(p.read_text())
        waits += [w for w in shard['spawnWaits'] if w['inHold']]
        all_waits += shard['spawnWaits']
        pending += shard['pendingRequestsAtEnd']
        for k, v in shard['clients']['final'].items(): final[k] = final.get(k, 0)+v
        for k, v in shard['closeReasons'].items(): closes[k] = closes.get(k, 0)+v
    r['clientsFinal'] = final
    r['closeReasons'] = closes
    def wait_stats(values):
        v = sorted(values)
        s = stats(v)
        if v: s['p95'] = v[math.ceil(len(v)*.95)-1]
        return s
    r['spawnWaitMs'] = {label:wait_stats(w['waitMs'] for w in waits if flag is None or w['respawn'] == flag)
                        for label, flag in [('all', None), ('initial', False), ('respawn', True)]}
    r['initialSpawnWaitMsIncludingWarmup'] = wait_stats(w['waitMs'] for w in all_waits if not w['respawn'])
    r['pendingRequestsAtEnd'] = len(pending)
    r['pendingAgeMs'] = wait_stats(w['ageMs'] for w in pending)
    # Keep the actual operator readout; only whole stats intervals inside the hold.
    host_counters = json.loads((host/'host.json').read_text())
    a, b = host_counters['holdStartUnix'], host_counters['holdEndUnix']
    records = json.loads((host/'stats-lines.json').read_text())
    inside = [x for i, x in enumerate(records) if i and records[i-1]['unix'] >= a and x['unix'] <= b]
    import re
    loads = [float(re.search(r'simulation_load=([\d.]+)%', x['line'])[1]) for x in inside]
    r['operatorStats'] = {'completeIntervals':len(inside), 'simulationLoadPercent':stats(loads),
                          'meanSimulationLoadPercent':sum(loads)/len(loads) if loads else None}
    return r


if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('host', type=Path)
    p.add_argument('load', type=Path)
    p.add_argument('count', type=int)
    p.add_argument('output', type=Path)
    a = p.parse_args()
    a.output.write_text(json.dumps(reduce(a.host,a.load,a.count),indent=2)+'\n')
