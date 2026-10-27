"""Extend the #178 reduction with tick busy/gap nearest-rank diagnostics."""
import importlib.util
import json
from pathlib import Path

path=Path(__file__).parent.parent/'2026-10-26-epyc-host/analyze.py'
if not path.exists(): path=Path(__file__).with_name('epyc_analysis.py')
spec=importlib.util.spec_from_file_location('epyc_analysis',path)
base=importlib.util.module_from_spec(spec)
spec.loader.exec_module(base)


def tick_stats(host):
    counter=json.loads((host/'host.json').read_text())
    origin, records=json.loads((host/'ticks.json').read_text())
    selected=[r for r in records if counter['holdStartUnix'] <= origin+r[0]*1e-9 < counter['holdEndUnix']]
    return {'tickBusyMs':base.stats(r[1]*1e-6 for r in selected),
            'tickStartGapMs':base.stats(r[2]*1e-6 for r in selected if r[2])}


def reduce(host,load,count):
    r=base.reduce(host,load,count)
    r.update(tick_stats(host))
    return r


if __name__=='__main__':
    import argparse
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('host',type=Path); p.add_argument('load',type=Path)
    p.add_argument('count',type=int); p.add_argument('output',type=Path)
    a=p.parse_args()
    a.output.write_text(json.dumps(reduce(a.host,a.load,a.count),indent=2)+'\n')
