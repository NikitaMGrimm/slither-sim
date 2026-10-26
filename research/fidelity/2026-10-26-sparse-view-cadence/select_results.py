"""Select bounded, path-free publication results from external full analysis."""
import argparse
from pathlib import Path
import analyze as a

STUDY = Path(__file__).resolve().parent


def cadence(summary):
    return dict(n=summary['server']['n'], medianMs=summary['server']['p50'],
                p95Ms=summary['server']['p95'], le9=summary['le9'],
                exposureMs=summary['exposureMs'])


def selected_results(rows):
    groups = {'all', 'noOtherChange', 'noSnakeChange', 'entityQuiet'}
    groups.update('left_' + b for b in a.BINS)
    selected = []
    for row in rows:
        if 'error' in row:
            raise ValueError('analysis failure: ' + row['id'])
        result = {key: row[key] for key in ('id', 'cohort', 'sha256', 'counts', 'activity', 'windows')}
        result['summaries'] = {key: cadence(value) for key, value in row['summaries'].items() if key in groups}
        audit = row['audit']
        result['audit'] = None if audit is None else {
            key: audit[key] for key in ('status', 'issues', 'integrityVerified', 'completeGzip')}
        selected.append(result)
    return selected


def select(out):
    rows = a.load(out / 'results.json') + a.load(out / 'older-results.json')
    expected = a.load(STUDY / 'sources.json')
    assert {(r['id'], r['cohort'], r['sha256']) for r in rows} == {
        (r['id'], r['cohort'], r['sha256']) for r in expected}
    primary = [row for row in rows if row['cohort'] != 'older']
    older = [row for row in rows if row['cohort'] == 'older']
    # Primary per-capture tables; the historical cohort retains its complete
    # zero-peer capture coverage, not just the captures exceeding 1,000 gaps.
    historical = [dict(id=row['id'], sha256=row['sha256'],
                       zeroPeer=cadence(row['summaries']['left_0']),
                       activeIntervals=row['summaries']['all']['server']['n']) for row in older]
    return {
        'results.json': dict(primary=selected_results(primary), olderZeroPeerCaptures=historical),
        'pooled.json': a.load(out / 'pooled.json'),
        'selected-witnesses.json': a.load(out / 'selected-witnesses.json'),
        'verification.json': a.load(out / 'verification.json'),
    }


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True, help='External full analysis directory')
    parser.add_argument('--check', action='store_true', help='Compare against committed selections instead of writing')
    args = parser.parse_args()
    for name, value in select(args.out.resolve()).items():
        target = STUDY / name
        if args.check:
            assert value == a.load(target), 'selected result drift: ' + name
            print('Exact selection match: ' + name)
        else:
            if target.exists():
                raise FileExistsError(target)
            a.save(target, value)
