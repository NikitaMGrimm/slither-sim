"""Recompute report tables from selected, aggregate diagnostic results."""
import json, statistics
from pathlib import Path

root = Path(__file__).resolve().parent
rows = json.loads((root / 'counter-results.json').read_text())
footprints = json.loads((root / 'footprint-results.json').read_text())
profiles = json.loads((root / 'profile-results.json').read_text())

for protocol in ['stock', 'legacy']:
    for maintenance in [False, True]:
        selected = {r['event']: r['runs'] for r in rows
                    if r['protocol'] == protocol and r['maintenance'] == maintenance}
        cycles = selected['cycles']
        ipc = statistics.median(r['instructionsPerTick'] / r['eventPerTick'] for r in cycles)
        mpki = {event: statistics.median(r['eventPerKInstructions'] for r in runs)
                for event, runs in selected.items() if event != 'cycles'}
        print(protocol, 'maintenance' if maintenance else 'ordinary',
              'instructions M', statistics.median(r['instructionsPerTick'] for r in cycles) / 1e6,
              'cycles M', statistics.median(r['eventPerTick'] for r in cycles) / 1e6,
              'IPC', ipc, 'events per 1000 instructions', mpki,
              'CPU ms range', [min(r['cpuMsPerTick'] for r in cycles),
                               max(r['cpuMsPerTick'] for r in cycles)])

for f in footprints:
    sizes, known, food, bounds = f['sizes'], f['knownSnakes'], f['food'], f['bounds']
    body = f['bodies']['len'] * sizes['point']
    witness = bounds['bodyLen'] * sizes['point']
    blocks = bounds['blocksLen'] * bounds['blockSize']
    # A hypothetical u32 slot-to-record index is a byte estimate, not an
    # implemented representation or a measured speedup.
    compact = known['denseLengths'] * 4 + known['occupiedPayloadBytes']
    print(f['protocol'], 'body bytes', body, 'witness bytes', witness, 'blocks bytes', blocks,
          'dense history capacity bytes', known['denseCapacityBytes'],
          'dense history logical bytes', known['denseLengths'] * sizes['optionKnownSnake'],
          'occupied history bytes', known['occupiedPayloadBytes'],
          'hypothetical compact bytes', compact,
          'history logical reduction bytes', known['denseLengths'] * sizes['optionKnownSnake'] - compact,
          'listing payload bytes', food['listingLen'] * sizes['listingEntry'],
          'listing capacity bytes', food['listingCapacity'] * sizes['listingEntry'],
          'held capacity bytes', food['heldCapacity'] * sizes['knownSectorEntry'],
          'food-index capacity bytes', food['indexCapacity'] * food['indexEntrySize'],
          'slot array bytes', f['slotsCapacity'] * sizes['slot'])
    nominal = body + witness + blocks + known['occupiedPayloadBytes']
    print('nominal bodies/bounds/history bytes', nominal,
          'cache-capacity multiples', {k: nominal / (k * 1024) for k in [256, 512, 1024, 2048]})

for p in profiles:
    if not p.get('primary', True):
        continue
    print(p['protocol'], p['event'], 'maintenance' if p['maintenance'] else 'ordinary',
          'samples', p['samples'], 'lost', p['lost'],
          [(r['source'], round(r['samples'] / p['samples'] * 100, 2))
           for r in p['sourceGroups'][:6]])
