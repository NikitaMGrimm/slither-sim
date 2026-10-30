"""Render report tables from selected-results.json; --check verifies report text."""
import json
from pathlib import Path
import sys

r=Path(__file__).resolve().parent


def number(value,digits=2):return f'{value:,.{digits}f}'


def render(value):
    census=['| Bots | Radius | Alive | Mean / p90 / p99 length | Food / prey | Warm virtual s |',
            '| ---: | ---: | ---: | ---: | ---: | ---: |']
    throughput=['| Bots | Simulation-only ticks/s | Serial ticks/s | Three-worker ticks/s |',
                '| ---: | ---: | ---: | ---: |']
    costs=['| Bots | Mode | World CPU p50 / p99 ms/tick | Admission mean ms/tick | Observe CPU µs/bot | A4 loop CPU µs/bot | RSS MiB |',
           '| ---: | --- | ---: | ---: | ---: | ---: | ---: |']
    phases=['| Bots | Profile mode | Motion CPU bounds ms/tick | Collision | Death | Food pickup | Prey | Ambient | Profile / ordinary CPU ratio range |',
            '| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |']
    def interval(s):return f"{s['p50']:.1f} ({s['min']:.1f}–{s['max']:.1f})"
    def bounds(values):return f'{values[0]:.3f}–{values[1]:.3f}'
    for p in value['populations']:
        n=p['population'];c=p['census'];w=c['world']
        census.append(f"| {n:,} | {number(w['arenaRadius'],0)} | {w['living']:,} | {c['meanLength']:.2f} / {c['p90Length']} / {c['p99Length']} | {w['food']:,} / {w['prey']:,} | {w['time']:.1f} |")
        if not p['qualification']['stabilized']:
            throughput.append(f'| {n:,} | Warmup censored | — | — |')
            continue
        modes=p['modes']
        throughput.append(f"| {n:,} | {interval(modes['simulation']['wholeWindowTicksPerSecond'])} | {interval(modes['serial']['wholeWindowTicksPerSecond'])} | {interval(modes['parallel']['wholeWindowTicksPerSecond'])} |")
        for mode in ['simulation','serial','parallel']:
            m=modes[mode]
            observe='—' if m['observationProcessCpuUsPerBot'] is None else number(m['observationProcessCpuUsPerBot'])
            decide='—' if m['policyLoopCpuUsPerBot'] is None else number(m['policyLoopCpuUsPerBot'])
            rss=max(v['VmRSS'] for v in m['finalMemoryKiB'])/1024
            costs.append(f"| {n:,} | {mode} | {m['worldCpuMs']['p50']:.3f} / {m['worldCpuMs']['p99']:.3f} | {m['admissionCpuMs']['mean']:.3f} | {observe} | {decide} | {rss:.1f} |")
        for mode in ['serial','parallel']:
            d=p['phaseDiagnostics'][mode]
            columns=[bounds(d['phases'][key]['cpuMsPerTickBounds']) for key in
                     ['movementSeconds','collisionSeconds','deathSeconds','foodSeconds','preySeconds','ambientSeconds']]
            ratios=d['profileToOrdinaryCpuRatios']
            phases.append(f"| {n:,} | {mode} | "+' | '.join(columns)+f" | {min(ratios):.3f}–{max(ratios):.3f} |")
    return {'census':'\n'.join(census),'throughput':'\n'.join(throughput),'costs':'\n'.join(costs),'phases':'\n'.join(phases)}


def main():
    data=json.loads((r/'selected-results.json').read_text())
    tables=render(data)
    if '--check' in sys.argv:
        report=(r/'REPORT.md').read_text()
        for key,table in tables.items():
            expected=f'<!-- {key}:start -->\n{table}\n<!-- {key}:end -->'
            assert expected in report, f'{key} table differs'
        print('All four report tables match selected data.')
    else:
        for key,table in tables.items():print(f'<!-- {key}:start -->\n{table}\n<!-- {key}:end -->\n')


if __name__=='__main__':main()
