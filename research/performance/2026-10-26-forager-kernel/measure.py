"""Balanced adjacent pairs; one pinned CPU, warmed inputs, retained raw samples."""
import argparse
import csv
import hashlib
import json
import os
import statistics
import subprocess
import math
from pathlib import Path

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--baseline', required=True)
parser.add_argument('--candidate', required=True)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--cpu', type=int, default=0)
parser.add_argument('--pairs', type=int, default=12)
parser.add_argument('--rounds', type=int, default=4000)
args = parser.parse_args()
assert args.pairs > 0 and args.rounds > 0 and args.cpu >= 0
OUT = args.output
OUT.mkdir(parents=True, exist_ok=False)
binary = {'baseline': args.baseline, 'candidate': args.candidate}
records = []
reference = {}
for pair in range(args.pairs):
    order = ['baseline', 'candidate'] if pair % 2 == 0 else ['candidate', 'baseline']
    for policy in ['A1', 'A4-mean']:
        for suite in ['all', 'dense']:
            for variant in order:
                label = f'{pair:02}-{policy}-{suite}-{variant}'
                raw = OUT / (label + '.csv')
                env = dict(os.environ, FORAGER_REPLAY_OUTPUT=str(raw), FORAGER_REPLAY_POLICY=policy, FORAGER_REPLAY_SUITE=suite, FORAGER_REPLAY_ROUNDS=str(args.rounds))
                with (OUT / (label + '.log')).open('w') as log:
                    subprocess.run(['taskset', '-c', str(args.cpu), binary[variant], '--ignored', '--exact', 'client::forager::conformance::replay_benchmark', '--nocapture'], env=env, stdout=log, stderr=subprocess.STDOUT, check=True)
                samples = []
                digest = hashlib.sha256()
                with raw.open() as f:
                    for row in csv.DictReader(f):
                        samples.append(int(row['ns']))
                        digest.update((row['round'] + ',' + row['case'] + ',' + row['heading_bits'] + ',' + row['boost'] + '\n').encode())
                identity = digest.hexdigest()
                key = (policy, suite)
                if key in reference:
                    assert identity == reference[key], f'Action mismatch {label}'
                else:
                    reference[key] = identity
                samples.sort()
                quantile = lambda p: samples[min(len(samples) - 1, math.ceil(p * len(samples)) - 1)]
                result = {'pair': pair, 'policy': policy, 'suite': suite, 'variant': variant, 'samples': len(samples), 'actionSha256': identity, 'rawSha256': hashlib.sha256(raw.read_bytes()).hexdigest(), 'mean': statistics.mean(samples), 'p50': quantile(.5), 'p90': quantile(.9), 'p99': quantile(.99), 'p99.9': quantile(.999), 'max': max(samples)}
                records.append(result)
                (OUT / 'passes.json').write_text(json.dumps(records, indent=2) + '\n')
                print(label, 'mean ns', round(result['mean'], 1), flush=True)
summary = []
for policy in ['A1', 'A4-mean']:
    for suite in ['all', 'dense']:
        row = {'policy': policy, 'suite': suite, 'pairs': args.pairs, 'statistics': {}}
        for metric in ['mean', 'p50', 'p90', 'p99', 'p99.9', 'max']:
            values = {v: [r[metric] for r in records if r['policy'] == policy and r['suite'] == suite and r['variant'] == v] for v in binary}
            ratios = [c / b for b, c in zip(values['baseline'], values['candidate'])]
            row['statistics'][metric] = {'baseline_ns': statistics.median(values['baseline']), 'candidate_ns': statistics.median(values['candidate']), 'median_paired_ratio': statistics.median(ratios), 'ratio_min': min(ratios), 'ratio_max': max(ratios)}
        summary.append(row)
(OUT / 'summary.json').write_text(json.dumps(summary, indent=2) + '\n')
