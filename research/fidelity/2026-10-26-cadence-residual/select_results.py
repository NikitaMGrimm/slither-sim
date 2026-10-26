"""Select bounded summaries, or check exact reproduction and publication limits."""
import argparse
import hashlib
import json
from pathlib import Path

STUDY = Path(__file__).resolve().parent


def load(path):
    return json.loads(path.read_text(encoding='utf-8'))


def select(out):
    rows = load(out / 'results.json')
    expected = load(STUDY / 'sources.json')
    assert len(rows) == len(expected) == 12
    assert {(r['id'], r['inputSha256']) for r in rows} == {
        (s['id'], s['sha256']) for s in expected}
    # Keep capture summaries; full interval histograms and compressed CSV identities
    # stay external (gzip timestamps make the latter execution-dependent).
    selected = [{k: v for k, v in row.items() if k not in ('intervalHist', 'csvSha256')}
                for row in rows]
    counterfactuals = load(out / 'counterfactuals.json')
    local = []
    for row in counterfactuals['local']:
        local.append(dict(
            id=row['id'], windows=row['windows'],
            substitution={name: dict(
                medianLe9=value['pooledRuns']['p50'],
                donorCaptureMedians={alias: q['p50'] for alias, q in value['perOfficialCapture'].items()})
                for name, value in row['substitution'].items()},
            combinedWithPhaseMedianLe9=row['combinedWithPhase']['p50'],
            maxPhaseShareMedian=row['maxPhaseShare']['p50']))
    counterfactuals = {k: v for k, v in counterfactuals.items()
                       if k not in ('local', 'scriptSha256')}
    counterfactuals['local'] = local
    verification = {k: v for k, v in load(out / 'verification.json').items()
                    if k != 'scriptSha256'}
    assert verification['status'] == 'pass'
    assert sum(r['n'] for r in rows if r['id'].startswith('official')) == 195617
    assert [r['n'] for r in rows if r['id'].startswith('bots-500')] == [17147, 4425, 13071]
    return {'results.json': selected, 'counterfactuals.json': counterfactuals,
            'verification.json': verification}


def publication_checks():
    assert len((STUDY / 'REPORT.md').read_text(encoding='utf-8').splitlines()) <= 120
    allowed = {'REPORT.md', 'results.json', 'counterfactuals.json', 'verification.json',
               'sources.json', 'source-pins.json', 'analyze.py', 'counterfactual.py',
               'verify.py', 'select_results.py'}
    files = [p for p in STUDY.rglob('*') if p.is_file()]
    assert {p.relative_to(STUDY).as_posix() for p in files} == allowed
    size = sum(p.stat().st_size for p in files)
    assert size <= 150000, size
    assert all(b'\r' not in p.read_bytes() for p in files)
    pins = load(STUDY / 'source-pins.json')
    for name, digest in pins['publicationScripts'].items():
        assert hashlib.sha256((STUDY / name).read_bytes()).hexdigest() == digest, name
    assert pins['archives'] == {s['id']: s['sha256'] for s in load(STUDY / 'sources.json')}
    print(f'Publication limits pass: {size} bytes; report <=120 lines; LF; pins match')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True, help='External full analysis output')
    parser.add_argument('--check', action='store_true', help='Compare with committed selections')
    args = parser.parse_args()
    for name, value in select(args.out.resolve()).items():
        target = STUDY / name
        if args.check:
            assert value == load(target), 'selected result drift: ' + name
            print('Exact selection match: ' + name)
        else:
            if target.exists():
                raise FileExistsError(target)
            target.write_text(json.dumps(value, indent=2, allow_nan=False) + '\n',
                              encoding='utf-8', newline='\n')
    publication_checks()
