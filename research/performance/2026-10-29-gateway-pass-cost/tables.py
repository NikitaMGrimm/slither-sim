"""Recompute the report tables from bounded selected values (standard library)."""
from pathlib import Path
import json, statistics

r=Path(__file__).resolve().parent
attribution=json.loads((r/'attribution.json').read_text())
coarse=attribution['modes']['1']
print('Attribution: maintenance / ordinary, ms per tick')
for phase in coarse['maintenance']['phaseCpuMsMean']:
    if phase.startswith('transport'):continue
    print(phase,*(f"{coarse[k]['phaseCpuMsMean'][phase]:.6f}" for k in ['maintenance','ordinary']))
print('Fine diagnostic, separate window; includes timer/recorder overhead')
fine=attribution['modes']['2']
for phase in ['transportRead','transportWrite']:
    print(phase,*(f"{fine[k]['phaseCpuMsMean'][phase]:.6f}" for k in ['maintenance','ordinary']))
for phase in fine['maintenance']['websocketCpuExcludingTransportMs']:
    print(phase+' excluding transport',*(f"{fine[k]['websocketCpuExcludingTransportMs'][phase]['mean']:.6f}" for k in ['maintenance','ordinary']))
c=attribution['completeRecordCounters']['1']
print('Proven-empty flush calls, percent:',100*c['emptyCalls']/(c['emptyCalls']+c['pendingCalls']))

fixed=json.loads((r/'fixed-results.json').read_text())
print('\nNo-socket fixture instruction changes: median [min, max], percent')
for name,row in fixed['rows'].items():
    for phase in ['all','maintenance','ordinary']:
        changes=[100*(p['candidate'][phase]['instructionsMean']/p['base'][phase]['instructionsMean']-1) for p in row['pairs']]
        print(name,phase,f'{statistics.median(changes):+.6f}',f'[{min(changes):+.6f}, {max(changes):+.6f}]')

if (r/'live-incomplete.json').exists():
    live=json.loads((r/'live-incomplete.json').read_text())
    print('\nINCOMPLETE live series: excluded windows only; zero completed primary pairs')
    print('Isolation failure:',live['isolationFailure'])
    for name,case in live['cases'].items():
        print(name,case['exclusion'])
        for phase,row in case['summary'].items():
            print(phase,'ticks',row['ticks'],*(f"{metric}={row[metric]}" for metric in ['gatewayCpuMs','worldCpuMs']))
elif (r/'live-results.json').exists():
    live=json.loads((r/'live-results.json').read_text())
    print('\nLive pairs: base -> candidate median-per-arm ms/tick; paired percent median [min, max]')
    for phase in ['all','maintenance','ordinary']:
        for metric in ['gatewayCpuMs','worldCpuMs']:
            for stat in ['mean','p50','p99']:
                entry=live['aggregates'][phase][metric][stat]
                values=entry['medianPerArm']; change=entry['pairedChangePercent']; lo,hi=change['range']
                print(phase,metric,stat,f"{values['base']:.6f} -> {values['candidate']:.6f}",
                      f"{change['median']:+.3f}% [{lo:+.3f}%, {hi:+.3f}%]")
