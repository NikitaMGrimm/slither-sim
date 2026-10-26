"""Reproduce table figures from the retained analysis and synthetic simulation."""
import argparse
import bisect
from collections import Counter
import hashlib
import json
import math
from pathlib import Path
import statistics as st

ap = argparse.ArgumentParser()
ap.add_argument('--study', type=Path, required=True)
STUDY = ap.parse_args().study
r = json.loads((STUDY/'results.json').read_text())
windows = json.loads((STUDY/'direction-windows.json').read_text())
sim = [json.loads(line) for line in (STUDY/'simulation.jsonl').read_text().splitlines()]
summary = sim[-1]
samples = [{'time': 0, 'radius': summary['initial_radius'], 'living': 480,
            'total_variation': 0, 'direction_changes': 0}] + sim[:-1]
official = json.loads((STUDY/'series/run4-radius-long-20260927/bot0.jsonl.json').read_text())
connection = next(c for c in official['connections'] if c['conn'] == 4)
rows = connection['radius']
cs = r['connections']


def held_samples(data, key, timekey, start, end, step):
    times = [p[timekey] for p in data]
    values, ages = [], []
    for i in range(int((end-start)/step)+1):
        time = start+i*step
        j = bisect.bisect_right(times, time)-1
        if j >= 0:
            values.append(data[j][key])
            ages.append(time-times[j])
    delta = [b-a for a, b in zip(values, values[1:])]
    signs = [(d > 0)-(d < 0) for d in delta if abs(d) > 1e-8]
    return {'samples': len(values), 'range': max(values)-min(values),
            'total_variation': sum(abs(d) for d in delta),
            'reversals': sum(a != b for a, b in zip(signs, signs[1:])), 'max_age': max(ages)}


def ba(data, fn):
    m = Counter((p['direction'], fn(p)) for p in data)
    positive = sum(p['direction'] == 1 for p in data)
    negative = len(data)-positive
    return (m[1, 1]/positive + m[-1, -1]/negative)/2 if positive and negative else None


matched = [p for key, data in windows.items() if key != 'run3-large-radius-20260927' for p in data
           if p['map_age_ms'] is not None and p['map_age_ms'] <= 5000
           and all(p[k] is not None and p[k] > 0 for k in ('visible_cache', 'map_fraction', 'leader_mass', 'largest_mass'))]
proxies = {}
for key, data in r['proxy_models'].items():
    coefficient = data.get('coefficient')
    fn = (lambda p: p['previous_direction']) if key == 'persistence' else (
        lambda p, k=key, c=coefficient: 1 if c*math.sqrt(p[k]) > p['radius'] else -1)
    proxies[key] = {'coefficient': coefficient, 'train': data['train'],
                    'holdout_n': len(matched), 'holdout_pooled_ba': ba(matched, fn),
                    'holdout_directory_mean_ba': st.mean(v['balanced_accuracy'] for v in data['tests'].values()
                                                         if v['balanced_accuracy'] is not None)}
model_macro = {}
for key in r['models_pooled_holdout']:
    values = [p[key]['test']['balanced_accuracy'] for p in r['models_by_directory'].values()
              if p[key]['test']['balanced_accuracy'] is not None]
    model_macro[key] = {'directories': len(values), 'mean_ba': st.mean(values), 'median_ba': st.median(values)}

simulation_bins = []
for start in range(0, 180, 30):
    points = [p for p in samples if start <= p['time'] < start+30]
    simulation_bins.append({'start_s': start, 'end_s': start+30, 'radius_first': points[0]['radius'],
                            'radius_last': points[-1]['radius'], 'radius_min': min(p['radius'] for p in points),
                            'radius_max': max(p['radius'] for p in points), 'living_min': min(p['living'] for p in points),
                            'living_max': max(p['living'] for p in points),
                            'variation': points[-1]['total_variation']-points[0]['total_variation'],
                            'reversals': points[-1]['direction_changes']-points[0]['direction_changes']})
pulse = []
for start in range(280, 314, 2):
    points = [p for p in rows if start*1000 <= p['S'] < (start+2)*1000]
    pulse.append({'start_s': start, 'first_radius': points[0]['radius'], 'last_radius': points[-1]['radius'],
                  'rate': (points[-1]['radius']-points[0]['radius'])*1000/(points[-1]['S']-points[0]['S']),
                  'players_min': min(p['players'] for p in points), 'players_max': max(p['players'] for p in points)})
free_runs = list(r['free_run'].values())
holdout_free_runs = [data for key, data in r['free_run'].items()
                     if not key.startswith('run3-large-radius-20260927/')]
comparison = {str(step): {'synthetic_0_180s': held_samples(samples, 'radius', 'time', 0, 180, step),
                           'official_180_360s': held_samples(rows, 'radius', 'S', 180000, 360000, step*1000)}
              for step in (.128, 1, 5)}
source_pin = {v['label']: v['sha256'] for v in r['source_pins']}
out = {'proxies': proxies, 'model_macro': model_macro, 'simulation_summary': summary,
       'simulation_bins_30s': simulation_bins, 'pulse_2s': pulse, 'comparison_sampling': comparison,
       'simulation_samples': {'min_radius': min(p['radius'] for p in samples),
                              'max_radius': max(p['radius'] for p in samples),
                              'mean_living_at_10Hz': st.mean(p['living'] for p in samples),
                              'min_living': min(p['living'] for p in samples), 'max_living': 480},
       'overall': {'files': len(r['files']), 'complete_files': sum(p['eligible'] for p in r['files']),
                   'excluded_files': sum(not p['eligible'] for p in r['files']), 'connections': len(cs),
                   'capture_directories': len({p['archive'].split('/')[0] for p in cs}),
                   'recipient_seconds': sum(p['game_seconds'] for p in cs),
                   'z_packets': sum(p['z_packets'] for p in cs),
                   'changed_connections': sum(p['radius_min'] != p['radius_max'] for p in cs),
                   'both_direction_connections': sum(p['growth'] > 0 and p['shrink'] > 0 for p in cs),
                   'total_variation': sum(p['total_variation'] for p in cs),
                   'growth_intervals': sum(p['growth'] for p in cs), 'shrink_intervals': sum(p['shrink'] for p in cs),
                   'same_intervals': sum(p['same'] for p in cs)},
       'free_run_all': {'connections': len(free_runs),
                        'hold_initial_lower_mae': sum(v['held_initial_mae'] < v['population_model_mae'] for v in free_runs),
                        'population_model_mean_connection_mae': st.mean(v['population_model_mae'] for v in free_runs),
                        'hold_initial_mean_connection_mae': st.mean(v['held_initial_mae'] for v in free_runs)},
       'free_run_holdout': {'connections': len(holdout_free_runs),
                            'hold_initial_lower_mae': sum(v['held_initial_mae'] < v['population_model_mae'] for v in holdout_free_runs),
                            'population_model_mean_connection_mae': st.mean(v['population_model_mae'] for v in holdout_free_runs),
                            'hold_initial_mean_connection_mae': st.mean(v['held_initial_mae'] for v in holdout_free_runs)},
       'pins': source_pin}
with (STUDY/'tables.json').open('x', encoding='utf-8', newline='\n') as stream:
    json.dump(out, stream, indent=2, allow_nan=False)
    stream.write('\n')
print(json.dumps({k: out[k] for k in ('overall', 'simulation_samples', 'comparison_sampling', 'free_run_all', 'proxies')}, indent=2))
