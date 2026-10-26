"""Five descriptive alternating pairs after an untallied warmup pair."""
import argparse
import json
from pathlib import Path
import resource
import statistics

from loopback import run


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('base', type=Path, help='directory holding copied host_probe and host_capacity_load')
    parser.add_argument('candidate', type=Path, help='directory holding copied host_probe')
    parser.add_argument('output', type=Path)
    parser.add_argument('--load', type=Path, help='shared load binary (defaults to baseline host_capacity_load)')
    parser.add_argument('--count', type=int, default=500)
    parser.add_argument('--preset', choices=['battledome'])
    args = parser.parse_args()
    soft, hard = resource.getrlimit(resource.RLIMIT_NOFILE)
    resource.setrlimit(resource.RLIMIT_NOFILE, (min(max(soft, 4096), hard), hard))
    args.output.mkdir(parents=True, exist_ok=False)
    results = []
    for index in range(-1, 5):
        order = ['base', 'candidate'] if index < 0 or index % 2 == 0 else ['candidate', 'base']
        row = {'pair': index, 'warmup': index < 0, 'order': order}
        for label in order:
            output = args.output / f'{index}-{label}'
            run(getattr(args, label).resolve() / 'host_probe',
                args.load.resolve() if args.load else args.base.resolve() / 'host_capacity_load',
                output.resolve(), 15 if index < 0 else 45, '', args.count, args.preset)
            report = json.loads((output / 'result.json').read_text())
            row[label] = {k: report[k] for k in ['cpuMsPerTick', 'ticks', 'deferredTicks', 'binarySha256']}
            row[label]['playing'] = sum(x['clients']['timeWeightedMean']['playing'] for x in report['loads'])
            row[label]['errors'] = [x['errors'] for x in report['loads']]
        row['ratio'] = row['candidate']['cpuMsPerTick'] / row['base']['cpuMsPerTick']
        results.append(row)
        (args.output / 'selected.json').write_text(json.dumps(results, indent=2) + '\n')
        print(json.dumps(row), flush=True)
    print('median paired ratio', statistics.median(r['ratio'] for r in results if not r['warmup']), flush=True)
