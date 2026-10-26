"""Cross-check P2 raw witnesses and motion denominator against the standard census."""
import argparse
import base64
import gzip
import json
from pathlib import Path
import sys

def main():
    p=argparse.ArgumentParser();p.add_argument('analysis',type=Path);p.add_argument('capture',type=Path);p.add_argument('census',type=Path);p.add_argument('--source',type=Path,required=True);p.add_argument('--output',type=Path,required=True);args=p.parse_args()
    sys.path.insert(0,str(args.source/'tools/fidelity'))
    import slither_protocol as sp
    result=json.loads(args.analysis.read_text());census=json.loads(args.census.read_text())
    assert result['archiveSha256']==census['archiveSha256']
    with gzip.open(args.capture,'rt') as stream:raw=[json.loads(line) for line in stream]
    checked=0
    for rows in result['earliestWitnesses'].values():
        for row in rows:
            record=raw[row['jsonlLine']-1]
            assert record[0]=='i'
            packets=sp.split(base64.b64decode(record[2]))
            if 'packetHex' in row:assert packets[row['packetIndex']].hex()==row['packetHex']
            if 'packetsHex' in row:assert [p.hex() for p in packets]==row['packetsHex']
            checked+=1
    census_motion=sum(command['packets'] for segment in census['connections'] for command in segment['commands'] if command['command'] in set('eE345d7'))
    decoded_motion=len(result['packetRows']['motion'])
    missing=result['counts'].get('excluded-motion-target-unknown',0)
    assert census_motion==decoded_motion+missing,(census_motion,decoded_motion,missing)
    status=dict(status='passed',rawWitnessesChecked=checked,censusMotionPackets=census_motion,decodedMotionPackets=decoded_motion,excludedUnknownTargetMotionPackets=missing)
    with args.output.open('x') as out:json.dump(status,out,indent=2)
    print(json.dumps(status,indent=2))

if __name__=='__main__':main()
