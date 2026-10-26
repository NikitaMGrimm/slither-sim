"""Select bounded summaries from the full offline outputs; never copy row series."""
import argparse
from collections import Counter
import json
from pathlib import Path
import re


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--study', type=Path, required=True)
    ap.add_argument('--output', type=Path, required=True)
    args = ap.parse_args()
    result = json.loads((args.study / 'results.json').read_text())
    forecasts = json.loads((args.study / 'free-run.json').read_text())
    # Cross-check every retained forecast, including the 20 holding losses.
    assert all({k: v for k, v in data.items() if k != 'rows'} == result['free_run'][key]
               for key, data in forecasts.items())
    connection = next(c for c in result['connections']
                      if c['archive'] == 'run4-radius-long-20260927/bot0.jsonl.gz' and c['conn'] == 4)
    selected = {
        'source_commit': '8890b2ebd3370a2b98348b6620f9e419eb4a55a7',
        'source_pins': [{**p, 'label': p['label'].replace('\\', '/')} for p in result['source_pins']],
        'pooled': {k: v for k, v in result['pooled'].items()
                   if k not in ('opposite_states_selected', 'fastest_growth', 'fastest_shrink')},
        'long_connection': {k: v for k, v in connection.items() if k not in ('endpoint_group', 'runs')},
        'models_pooled_holdout': result['models_pooled_holdout'],
        'models_by_directory': {name: {key: data['test'] for key, data in models.items()}
                                for name, models in result['models_by_directory'].items()},
        'motion': result['motion'],
    }
    tables = json.loads((args.study / 'tables.json').read_text())
    tables['pins'] = {k.replace('\\', '/'): v for k, v in tables['pins'].items()}
    inventory = json.loads((args.study / 'inventory-before.json').read_text())
    ordinals = Counter()
    for entry in inventory:
        if re.search(r'(?:\d{1,3}\.){3}\d{1,3}', entry['archive']):
            # Excluded survey filenames contain endpoints; hashes still identify
            # their bytes without publishing those names or a mapping to them.
            directory = entry['archive'].rsplit('/', 1)[0]
            ordinals[directory] += 1
            entry['archive'] = f'{directory}/excluded-{ordinals[directory]:02}.jsonl.gz'
    verification = json.loads((args.study / 'verification.json').read_text())
    args.output.mkdir(parents=True, exist_ok=True)
    for name, data in [('results.json', selected), ('free-run-summary.json', result['free_run']),
                       ('tables.json', tables), ('source-inventory.json', inventory),
                       ('verification.json', verification)]:
        with (args.output / name).open('x', encoding='utf-8', newline='\n') as stream:
            json.dump(data, stream, indent=2, allow_nan=False)
            stream.write('\n')


if __name__ == '__main__':
    main()
