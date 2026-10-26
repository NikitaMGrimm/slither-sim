"""Exact nearest-rank reduction of retained per-frame timings and aligned counters."""
from array import array
import argparse
from collections import defaultdict
import csv
import json
import math
from pathlib import Path

def stats(values):
    values = sorted(values)
    if not values:
        return {'count': 0}
    n = len(values)
    return {'count': n, 'p50': values[math.ceil(n * .5)-1], 'p99': values[math.ceil(n * .99)-1],
            'max': values[-1], 'min': values[0]}

def timing(root):
    excess, gaps = array('d'), array('d')
    per_client = defaultdict(lambda: {'excess': array('d'), 'gap': array('d')})
    for path in sorted(root.glob('load-*.frames.csv')):
        connections = defaultdict(lambda: array('d'))
        with path.open() as f:
            for row in csv.DictReader(f):
                connections[(int(row['client']), int(row['connection']))].append(float(row['receipt_ms']) - float(row['server_ms']))
        for (client, _), values in connections.items():
            least = min(values)
            normalized = array('d', (x-least for x in values))
            excess.extend(normalized)
            per_client[client]['excess'].extend(normalized)
        del connections
    for path in sorted(root.glob('load-*.intervals.csv')):
        with path.open() as f:
            for row in csv.DictReader(f):
                gap = float(row['interval_ms'])
                gaps.append(gap)
                per_client[int(row['client'])]['gap'].append(gap)
    return {'excessMs': stats(excess), 'ownUpdateGapMs': stats(gaps),
            'perClient': {str(c): {'excessMs':stats(v['excess']), 'ownUpdateGapMs':stats(v['gap'])} for c,v in per_client.items()}}

def boundaries(path):
    d = json.loads(path.read_text())
    return d, next(r for r in d['rows'] if r.get('boundary') == 'start'), next(r for r in d['rows'] if r.get('boundary') == 'end')

def resources(path):
    d, a, b = boundaries(path)
    elapsed = b['unix'] - a['unix']
    if not a['processes'] or any(v is None or b['processes'].get(p) is None for p,v in a['processes'].items()):
        raise ValueError('Missing live process counter at hold boundary')
    cpu = sum(b['processes'][p]['cpuSeconds'] - v['cpuSeconds'] for p,v in a['processes'].items() if v and b['processes'].get(p))
    nic = {key: sum(b['nic'][n][key]-v[key] for n,v in a['nic'].items()) for key in ['rxBytes','txBytes','rxPackets','txPackets','rxDrops','txDrops']}
    rows = [r for r in d['rows'] if a['unix'] <= r['unix'] <= b['unix']]
    rss = max(sum(v['rssBytes'] for v in r['processes'].values() if v) for r in rows)
    thread_cpu = defaultdict(float)
    for p, threads in a['threads'].items():
        for tid, v in threads.items():
            other = b['threads'].get(p, {}).get(tid)
            if other:
                thread_cpu[v['name']] += (other['cpuSeconds']-v['cpuSeconds'])/elapsed*100
    cpu_a, cpu_b = [int(x) for x in a['cpu'].split()[1:]], [int(x) for x in b['cpu'].split()[1:]]
    delta = [y-x for x,y in zip(cpu_a,cpu_b)]
    # guest/guest_nice are already included in user/nice, so use the first eight.
    total = sum(delta[:8])
    return {'windowSeconds':elapsed, 'cpuPercentOneCore':cpu/elapsed*100, 'peakRssBytes':rss,
            'nic':nic, 'threadCpuPercentOneCore':dict(thread_cpu),
            'systemBusyPercentAllCpus':100*(total-delta[3]-delta[4])/total,
            'systemStealPercentAllCpus':100*delta[7]/total,
            'nofile':d.get('nofile'),
            'perProcessCpuPercentOneCore': {p:(b['processes'][p]['cpuSeconds']-v['cpuSeconds'])/elapsed*100 for p,v in a['processes'].items() if v and b['processes'].get(p)}}

def reduce_trial(host, load, requested):
    d, a, b = boundaries(host/'host.json')
    elapsed = b['unix']-a['unix']
    result = {'requested':requested, 'holdSeconds':elapsed, 'ticksPerSecond':(b['ticks']-a['ticks'])/elapsed,
              'deferredTicks':b['deferred']-a['deferred'], 'deferredAtStart':a['deferred'], 'deferredAtEnd':b['deferred'],
              'executedTicks': b['ticks']-a['ticks'],
              'worldLagStartMs': (a['unix']-d['boundUnix'])*1000-a['ticks']*7,
              'worldLagEndMs': (b['unix']-d['boundUnix'])*1000-b['ticks']*7,
              'simulationLoadPercent':(b['simulationSeconds']-a['simulationSeconds'])/elapsed*100,
              'hostResources':resources(host/'telemetry.json'), 'generatorResources':resources(load/'telemetry.json')}
    clients, counters = defaultdict(float), defaultdict(int)
    generator = []
    payload = 0
    for path in load.glob('load-*.json'):
        shard = json.loads(path.read_text())
        for key, value in shard['clients']['timeWeightedMean'].items(): clients[key] += value
        for key, value in shard['holdCounters'].items(): counters[key] += value
        payload += shard['traffic']['bytesIn']
        generator.append({k:v for k,v in shard['generator'].items() if k in ['pollIntervalMs','controlLatenessMs','connectCallMs','busySecondsInWindow']})
        if shard['errors']: result.setdefault('errors', []).append(shard['errors'])
    result['clientsTimeWeighted'] = dict(clients)
    result['admittedTimeWeighted'] = sum(clients[x] for x in ['setup','playing','dead'])
    result['holdCounters'] = dict(counters)
    result['generatorService'] = generator
    # Use configured hold duration for generator bytes/population (its exact clipping window).
    hold = json.loads((load/'telemetry.json').read_text())['hold']
    result['payloadBytesPerPlayingClientSecond'] = payload/hold/clients['playing']
    result['hostNicBytesPerPlayingClientSecond'] = {k:v/result['hostResources']['windowSeconds']/clients['playing'] for k,v in result['hostResources']['nic'].items() if k.endswith('Bytes')}
    result.update(timing(load))
    result['criterionA'] = result['excessMs'].get('p99', math.inf) <= 3
    result['criterionB'] = result['excessMs'].get('p99', math.inf) <= 10 and result['deferredTicks'] == 0
    return result

if __name__ == '__main__':
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('host', type=Path); p.add_argument('load', type=Path)
    p.add_argument('requested', type=int); p.add_argument('output', type=Path)
    a = p.parse_args()
    a.output.write_text(json.dumps(reduce_trial(a.host,a.load,a.requested),indent=2)+'\n')
