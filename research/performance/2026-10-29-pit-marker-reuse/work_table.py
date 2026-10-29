"""Recompute the operation table without building or running a simulator.

Usage: python work_table.py work-counts.json
"""
from pathlib import Path
import json,sys
data=json.loads(Path(sys.argv[1]).read_text())
print('| Protocol | Row / group | Ticks | Empty overlay calls, base to candidate | Nonempty overlays, base to candidate | Added coverage queries | Coverage input / unique keys |')
print('| --- | --- | ---: | ---: | ---: | ---: | ---: |')
for protocol in [19,13]:
    for row,group in [('markers','marker'),('minimap','all')]:
        arms=data['rows'][f'{row}-{protocol}']
        a,b=[arms[arm]['groups'][group] for arm in ['base','candidate']]
        assert a['ticks']==b['ticks'] and a['coverageQueries']==0
        label='shrinking / marker ticks' if row=='markers' else 'fixed pit / all ticks'
        print(f"| {protocol} | {label} | {a['ticks']} | {a['emptyOverlays']} to {b['emptyOverlays']} | {a['nonemptyOverlays']} to {b['nonemptyOverlays']} | {b['coverageQueries']} | {b['coverageInputKeys']} / {b['coverageUniqueKeys']} |")
