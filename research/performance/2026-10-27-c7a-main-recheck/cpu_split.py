"""Boundary-aligned user/system CPU percentages; delivery reducer is unchanged."""
import json
from pathlib import Path


def split(path):
    rows = json.loads(Path(path).read_text())['rows']
    a = next(r for r in rows if r.get('boundary') == 'start')
    b = next(r for r in rows if r.get('boundary') == 'end')
    elapsed = b['unix'] - a['unix']

    def delta(first, last):
        return {k: sum(last[p][k + 'Seconds'] - v[k + 'Seconds']
                       for p, v in first.items()) / elapsed * 100
                for k in ['user', 'system']}

    result = {'processPercentOneCore': delta(a['processes'], b['processes']),
              'threadPercentOneCore': {}}
    for pid, threads in a['threads'].items():
        for tid, v in threads.items():
            last = b['threads'][pid][tid]
            d = delta({tid: v}, {tid: last})
            d['niceStart'], d['niceEnd'] = v['nice'], last['nice']
            assert v['name'] not in result['threadPercentOneCore']
            result['threadPercentOneCore'][v['name']] = d
    return result
