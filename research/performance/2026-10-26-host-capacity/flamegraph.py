"""Render local-only SVGs with a pinned upstream FlameGraph revision and Perl."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import urllib.request
from collections import Counter
from profile import read_samples, reduce

REVISION = '41fee1f99f9276008b7cd112fca19dc3ea84ac32'

def main(stacks, output, perl, window=None):
    output.parent.mkdir(parents=True,exist_ok=True)
    tools=output.parent/'flamegraph-tools'
    tools.mkdir(exist_ok=True)
    pins={}
    for name in ['flamegraph.pl']:
        path=tools/name
        url=f'https://raw.githubusercontent.com/brendangregg/FlameGraph/{REVISION}/{name}'
        data=urllib.request.urlopen(url,timeout=30).read()
        path.write_bytes(data)
        pins[name]={'url':url,'sha256':hashlib.sha256(data).hexdigest()}
    # This helper is intended for native Linux (or WSL paths), with Perl on PATH.
    collapsed=output.with_suffix('.folded')
    weights=Counter()
    event=reduce(stacks,window)['event']
    for period, stack in read_samples(stacks,window):
        if period is None: raise ValueError('Weighted flamegraph requires preserved cpu-clock periods')
        weights[';'.join(name.replace(';',':') for name in reversed(stack))] += period
    with collapsed.open('w') as f:
        for stack,weight in sorted(weights.items()): f.write(f'{stack} {weight}\n')
    with collapsed.open('rb') as source, output.open('wb') as dest:
        subprocess.run([perl,str(tools/'flamegraph.pl'),'--title','SlitherSim protocol-19 host CPU','--bgcolors','#ffffff','--countname', 'cycles' if event=='cycles' else 'nanoseconds'],stdin=source,stdout=dest,check=True)
    output.write_text(output.read_text().replace('rx="2" ry="2"','rx="0" ry="0"'),encoding='utf-8',newline='\n')
    (output.parent/'flamegraph-source-pins.json').write_text(json.dumps({'revision':REVISION,'tools':pins},indent=2)+'\n')

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('stacks',type=Path);p.add_argument('output',type=Path);p.add_argument('--perl',default='perl')
    p.add_argument('--window',type=float,nargs=2)
    a=p.parse_args();main(a.stacks,a.output,a.perl,a.window)
