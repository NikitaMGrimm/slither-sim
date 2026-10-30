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
    costs=['| Bots | Mode | World CPU p50 / p99 / max ms/tick | Admission mean ms/tick | Observe CPU µs/bot | A4 loop CPU µs/bot | RSS / peak RSS MiB |',
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
            peak=max(v['VmHWM'] for v in m['finalMemoryKiB'])/1024
            costs.append(f"| {n:,} | {mode} | {m['worldCpuMs']['p50']:.3f} / {m['worldCpuMs']['p99']:.3f} / {m['worldCpuMs']['max']:.3f} | {m['admissionCpuMs']['mean']:.3f} | {observe} | {decide} | {rss:.1f} / {peak:.1f} |")
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
    storm=r/'botstorm-selected.json'
    if storm.exists():tables.update(render_storm(json.loads(storm.read_text())))
    if '--check' in sys.argv:
        report=(r/'REPORT.md').read_text()
        for key,table in tables.items():
            expected=f'<!-- {key}:start -->\n{table}\n<!-- {key}:end -->'
            assert expected in report, f'{key} table differs'
        print(f'All {len(tables)} report tables match selected data.')
    else:
        for key,table in tables.items():print(f'<!-- {key}:start -->\n{table}\n<!-- {key}:end -->\n')


def render_storm(value):
    population=['| Bots | Fixed radius | Alive | Mean / p90 / p99 length | Deaths / admissions per virtual s |',
                '| ---: | ---: | ---: | ---: | ---: |']
    cpu=['| Bots | Physics CPU p50 / p99 / max ms | Admission mean / max ms | Physics+admission CPU p50 / p99 / max ms | CPU ticks >7 ms | Physics+admission wall p99 / max ms | Wall ticks >7 ms |',
         '| ---: | ---: | ---: | ---: | ---: | ---: | ---: |']
    phases=['| Bots | Motion CPU bounds ms/tick | Collisions | Death processing | Food pickup | Prey | Ambient | Center steering mean ms/tick |',
            '| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |']
    def triple(s):return f"{s['p50']:.3f} / {s['p99']:.3f} / {s['max']:.3f}"
    for p in value['populations']:
        n=p['population']
        if p['status']!='stabilized':
            population.append(f"| {n:,} | {p['status']} | — | — | — |")
            continue
        c=p['census'];w=c['world']
        population.append(f"| {n:,} | {w['arenaRadius']:,.2f} | {w['living']:,} | {c['meanLength']:.2f} / {c['p90Length']} / {c['p99Length']} | {p['deathsPerVirtualSecond']:.2f} / {p['admissionsPerVirtualSecond']:.2f} |")
        a=p['admissionCpuMs'];wall=p['worldPlusAdmissionWallMs']
        cpu.append(f"| {n:,} | {triple(p['worldCpuMs'])} | {a['mean']:.3f} / {a['max']:.3f} | {triple(p['worldPlusAdmissionCpuMs'])} | {100*p['worldPlusAdmissionCpuFractionAbove7ms']:.2f}% | {wall['p99']:.3f} / {wall['max']:.3f} | {100*p['worldPlusAdmissionWallFractionAbove7ms']:.2f}% |")
        phase=p['phaseDiagnostic']['phases']
        columns=[]
        for key in ['movementSeconds','collisionSeconds','deathSeconds','foodSeconds','preySeconds','ambientSeconds']:
            lo,hi=phase[key]['cpuMsPerTickBounds'];columns.append(f'{lo:.3f}–{hi:.3f}')
        phases.append(f"| {n:,} | "+' | '.join(columns)+f" | {p['centerCpuMs']['mean']:.3f} |")
    return {'storm-census':'\n'.join(population),'storm-cpu':'\n'.join(cpu),'storm-phases':'\n'.join(phases)}


if __name__=='__main__':main()
