"""Independent consistency checks using the existing census and transport tools."""
import csv
import gzip
import json
from pathlib import Path
import sys
import analyze as a
import packet_census as pc
import transport_parity as tp

import argparse
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--out', type=Path, required=True, help='External analysis output directory')
args = parser.parse_args()
out=args.out.resolve()
results=a.load(out/'results.json')
checks=[]
for result in results:
    assert 'error' not in result, result
    name=result['id']
    census=pc.census(Path(result['path']))
    # Census reports by connection; compare the independent packet family totals.
    got={}
    for segment in census['connections']:
        for entry in segment['commands']:
            c=entry['command']
            got[c]=got.get(c,0)+entry['packets']
    assert got==result['families'], (name,got,result['families'])
    prior=None; eligible=0
    with gzip.open(out/(name+'.frames.csv.gz'),'rt',encoding='utf-8') as stream:
        for row in csv.DictReader(stream):
            if row['ownPresent']=='True':
                ids=set(row['visibleIds'].split(';'))
                assert row['ownId'] in ids
                assert len(ids)==int(row['after'])+1
                own=json.loads(row['ownState'])
                assert own['anchorServer']<=int(row['server'])
            if prior and prior['epoch']==row['epoch']:
                assert row['before']==prior['after']
                assert int(row['server'])-int(prior['server'])==int(row['gap'])
                assert abs(float(row['receipt'])-float(prior['receipt'])-float(row['receiptGap']))<.0011
                if prior['active']=='True' and row['active']=='True': eligible+=1
            prior=row
    assert eligible==result['summaries']['all']['server']['n'],(name,eligible)
    # Reproduce original gates independently on exactly the original tool's population.
    measured,_=tp.measure_capture(Path(result['path']))
    checks.append(dict(id=name,censusPacketCountsMatch=True,cacheAndClockInvariants=True,
        activeIntervals=eligible,originalFrameGaps=measured['summary']['frameGapsMs']['server']))
    print('Verified '+name,flush=True)

# Whole-window rate integration must include an interval spanning no receipts.
synthetic=[dict(epoch=1,server=500,gap=500,leftBin='0',endBin='0'),
           dict(epoch=1,server=2500,gap=2000,leftBin='0',endBin='0')]
w=a.fixed_windows(synthetic)
assert [(x['start'],x['frames']) for x in w]==[(0,1),(1000,0)],w
assert not a.fixed_windows([dict(epoch=1,server=900,gap=800,leftBin='0',endBin='0')])
assert not a.fixed_windows([dict(epoch=1,server=500,gap=500,leftBin='0',endBin='1-2'),
                           dict(epoch=1,server=1000,gap=500,leftBin='1-2',endBin='1-2')])
a.save(out/'verification.json',dict(checks=checks,syntheticWindowChecks=True,
    limits='These are analysis consistency checks, not new protocol tests or scientific validation.'))
