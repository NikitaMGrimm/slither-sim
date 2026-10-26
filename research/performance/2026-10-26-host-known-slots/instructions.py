"""Exploratory whole-process instruction counts for all four workloads."""
import argparse
import json
from pathlib import Path
import statistics
import subprocess


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('base', type=Path)
    parser.add_argument('candidate', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--perf', required=True)
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=False)
    results = {}
    for count in [500, 40, 64, 80]:
        case = 'client-official-500' if count == 500 else f'client-battledome-{count}'
        binary = 'perf_baseline' if count == 500 else 'perf_battledome'
        pairs = []
        for index in range(-1, 3):
            order = ['base', 'candidate'] if index < 0 or index % 2 == 0 else ['candidate', 'base']
            row = {'pair': index, 'warmup': index < 0, 'order': order}
            for label in order:
                prefix = args.output / f'{case}-{index}-{label}'
                with prefix.with_suffix('.stdout').open('w') as out, prefix.with_suffix('.stderr').open('w') as err:
                    subprocess.run(['taskset', '-c', '0', args.perf, 'stat', '-x;', '-e', 'instructions:u',
                                    '-o', str(prefix.with_suffix('.stat')),
                                    '--', str(getattr(args, label).resolve() / binary), case, '100'],
                                   stdout=out, stderr=err, check=True, timeout=120)
                entries = [line.split(';') for line in prefix.with_suffix('.stat').read_text().splitlines()]
                fields = next(x for x in entries if len(x) > 4 and x[2] == 'instructions:u')
                assert float(fields[4]) >= 99, 'multiplexed counter'
                row[label] = int(float(fields[0]))
                report = json.loads(prefix.with_suffix('.stdout').read_text().splitlines()[-1])
                assert report['case'] == case and report['iterations'] == 100
            row['ratio'] = row['candidate'] / row['base']
            pairs.append(row)
        results[case] = {'pairs': pairs, 'medianPairedRatio': statistics.median(x['ratio'] for x in pairs[1:]),
                         'minPairedRatio': min(x['ratio'] for x in pairs[1:]), 'maxPairedRatio': max(x['ratio'] for x in pairs[1:])}
        (args.output / 'selected.json').write_text(json.dumps(results, indent=2) + '\n')
        print(case, json.dumps(results[case]), flush=True)
