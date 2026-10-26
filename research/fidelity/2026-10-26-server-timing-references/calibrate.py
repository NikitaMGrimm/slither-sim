"""Evaluate the report's fixed six-capture LOO rule, without raw data or network."""
import argparse
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'tools/fidelity'))
import transport_parity as tp

ALLOWANCES = (0.5, 1, 2)
MINIMUM_SAMPLES = 100


def evaluate(captures):
    if len(captures) != 6 or len({c['batch'] for c in captures}) != 6:
        raise ValueError('expected six distinct completed batches')
    values = {}
    for name, (path, _) in tp.GATES.items():
        values[name] = []
        for capture in captures:
            summary = capture['summary']
            if tp.lookup(summary, path.rsplit('.', 1)[0] + '.n') < MINIMUM_SAMPLES:
                raise ValueError('insufficient samples: ' + capture['batch'] + '/' + name)
            values[name].append(tp.lookup(summary, path))
    trials = []
    for allowance in ALLOWANCES:
        rows = []
        for i, capture in enumerate(captures):
            metrics = {}
            for name, observed in values.items():
                other = observed[:i] + observed[i+1:]
                low, high = min(other), max(other)
                upper = high + allowance * (high-low)
                metrics[name] = dict(candidate=observed[i], referenceMin=low,
                                     referenceMax=high, upper=upper, passed=observed[i] <= upper)
            rows.append(dict(batch=capture['batch'], metrics=metrics,
                             allPassed=all(m['passed'] for m in metrics.values())))
        trials.append(dict(spreadAllowance=allowance, allPassed=all(r['allPassed'] for r in rows), rows=rows))
    selected = next((trial for trial in trials if trial['allPassed']), None)
    allowance = selected['spreadAllowance'] if selected else None
    final = ({name: dict(min=min(v), max=max(v), upper=max(v)+allowance*(max(v)-min(v)))
              for name, v in values.items()} if selected else None)
    return dict(schemaVersion=1,
        rule='Smallest common allowance in {0.5, 1, 2} passing all four gates for every capture against the other five; minimum 100 samples per metric.',
        limits='False-reject control is only this LOO check over six captures; batch drift observed. No population error-rate guarantee or independent held-out validation.',
        selectedAllowance=allowance, minimumSamples=MINIMUM_SAMPLES, trials=trials, finalLimits=final)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--reference', type=Path, default=tp.DEFAULT_REFERENCE)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    result = evaluate(json.loads(args.reference.read_text(encoding='utf-8'))['captures'])
    result['referenceSha256'] = tp.pc.sha256(args.reference)
    result['calibratorSha256'] = tp.pc.sha256(Path(__file__))
    with args.out.open('x', encoding='utf-8', newline='\n') as stream:
        json.dump(result, stream, indent=2, allow_nan=False)
        stream.write('\n')
    if result['selectedAllowance'] is None:
        raise SystemExit('No allowance in the fixed set passes all six captures and four gates; failure table retained.')
