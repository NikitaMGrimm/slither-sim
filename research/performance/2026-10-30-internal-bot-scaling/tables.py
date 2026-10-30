"""Render report tables from selected-results.json; --check verifies report text."""
import json
from pathlib import Path
import sys

r=Path(__file__).resolve().parent


def number(value,digits=2):return f'{value:,.{digits}f}'


def render(value):
    census=['| Bots | Radius | Alive | Mean / p90 / p99 length | Food / prey | Warm virtual s |',
            '| ---: | ---: | ---: | ---: | ---: | ---: |']
    throughput=['| Bots | Ecology screen | Simulation-only ticks/s | Serial ticks/s | Three-worker ticks/s |',
                '| ---: | --- | ---: | ---: | ---: |']
    costs=['| Bots | Mode | World CPU p50 / p99 / max ms/tick | Admission mean ms/tick | Observe CPU µs/bot | A4 loop CPU µs/bot | RSS / peak RSS MiB |',
           '| ---: | --- | ---: | ---: | ---: | ---: | ---: |']
    phases=['| Bots | Profile mode | Motion CPU bounds ms/tick | Collision | Death | Food pickup | Prey | Ambient | Profile / ordinary CPU ratio range |',
            '| ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |']
    def interval(s):return f"{s['p50']:.1f} ({s['min']:.1f}–{s['max']:.1f})"
    def bounds(values):return f'{values[0]:.3f}–{values[1]:.3f}'
    for p in value['populations']:
        n=p['population'];c=p['census'];w=c['world']
        census.append(f"| {n:,} | {number(w['arenaRadius'],0)} | {w['living']:,} | {c['meanLength']:.2f} / {c['p90Length']} / {c['p99Length']} | {w['food']:,} / {w['prey']:,} | {w['time']:.1f} |")
        screen='pass' if p['qualification']['stabilized'] else 'unsettled / guarded'
        if 'modes' not in p:
            throughput.append(f'| {n:,} | {screen} | Not measured | — | — |')
            continue
        modes=p['modes']
        throughput.append(f"| {n:,} | {screen} | {interval(modes['simulation']['wholeWindowTicksPerSecond'])} | {interval(modes['serial']['wholeWindowTicksPerSecond'])} | {interval(modes['parallel']['wholeWindowTicksPerSecond'])} |")
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
    diagnostic=r/'diagnostic-selected.json'
    if diagnostic.exists():tables.update(render_diagnostic(json.loads(diagnostic.read_text())))
    if '--check' in sys.argv:
        report=(r/'REPORT.md').read_text()
        for key,table in tables.items():
            expected=f'<!-- {key}:start -->\n{table}\n<!-- {key}:end -->'
            assert expected in report, f'{key} table differs'
        print(f'All {len(tables)} report tables match selected data.')
    else:
        for key,table in tables.items():print(f'<!-- {key}:start -->\n{table}\n<!-- {key}:end -->\n')


def render_storm(value):
    population=['| Bots | Ecology screen | Fixed radius | Alive | Mean / p90 / p99 length | Deaths / admissions per virtual s |',
                '| ---: | --- | ---: | ---: | ---: | ---: |']
    cpu=['| Bots | Physics CPU p50 / p99 / max ms | Admission mean / max ms | Physics+admission CPU p50 / p99 / max ms | CPU ticks >7 ms | Physics+admission wall p99 / max ms | Wall ticks >7 ms |',
         '| ---: | ---: | ---: | ---: | ---: | ---: | ---: |']
    phases=['| Bots | Motion CPU bounds ms/tick | Collisions | Death processing | Food pickup | Prey | Ambient | Center steering mean ms/tick |',
            '| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |']
    def triple(s):return f"{s['p50']:.3f} / {s['p99']:.3f} / {s['max']:.3f}"
    for p in value['populations']:
        n=p['population']
        if 'worldCpuMs' not in p:
            population.append(f"| {n:,} | {p['status']} | — | — | — | — |")
            continue
        c=p['census'];w=c['world']
        population.append(f"| {n:,} | {p['status']} | {w['arenaRadius']:,.2f} | {w['living']:,} | {c['meanLength']:.2f} / {c['p90Length']} / {c['p99Length']} | {p['deathsPerVirtualSecond']:.2f} / {p['admissionsPerVirtualSecond']:.2f} |")
        a=p['admissionCpuMs'];wall=p['worldPlusAdmissionWallMs']
        cpu.append(f"| {n:,} | {triple(p['worldCpuMs'])} | {a['mean']:.3f} / {a['max']:.3f} | {triple(p['worldPlusAdmissionCpuMs'])} | {100*p['worldPlusAdmissionCpuFractionAbove7ms']:.2f}% | {wall['p99']:.3f} / {wall['max']:.3f} | {100*p['worldPlusAdmissionWallFractionAbove7ms']:.2f}% |")
        phase=p['phaseDiagnostic']['phases']
        columns=[]
        for key in ['movementSeconds','collisionSeconds','deathSeconds','foodSeconds','preySeconds','ambientSeconds']:
            lo,hi=phase[key]['cpuMsPerTickBounds'];columns.append(f'{lo:.3f}–{hi:.3f}')
        phases.append(f"| {n:,} | "+' | '.join(columns)+f" | {p['centerCpuMs']['mean']:.3f} |")
    return {'storm-census':'\n'.join(population),'storm-cpu':'\n'.join(cpu),'storm-phases':'\n'.join(phases)}


def render_diagnostic(value):
    guards=['| Workload | Ticks | Box cells min–max | Food at pickup min–max | Cover area min–max | Declines a / b / c ticks | Dense / hashed / linear ticks | Pickup CPU share on matched slow ticks |',
            '| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |']
    slow=['| Workload | Sample group | Samples | Movement | Collisions | Food pickup/cache | Grid build | Death | Admission | Prey | Ambient | Unresolved |',
          '| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |']
    overhead=['| Workload | Diagnostic / primary world CPU range | Sampled / ordinary CPU | Phase-profile / ordinary world CPU | Lost samples | Unparsed records |',
              '| --- | ---: | ---: | ---: | ---: | ---: |']
    def span(s):return f"{s['min']:,}–{s['max']:,}"
    for window in value['windows']:
        label=window['label'];o=window['overhead'];ratios=o['timestampDebugToPrimaryWorldCpuRatios']
        lost=window['lostSamples'] if window['lostSamples'] is not None else 'unknown'
        overhead.append(f"| {label} | {min(ratios):.3f}–{max(ratios):.3f} | {o['cpuRatio']:.3f} | {window['stepProfileDiagnostic']['profileToOrdinaryCpuRatios'][0]:.3f} | {lost} | {window['unparsedRecords']} |")
        if label=='1000':continue
        g=window['pickupGuards']['allTicks'];d=g['declines'];p=g['paths']
        bounds=window['sameVirtualSlowTicksPhaseDiagnostic']['phases']['foodSeconds']['cpuShareBounds']
        guards.append(f"| {label} | {g['ticks']} | {span(g['boxCells'])} | {span(g['foodLen'])} | {span(g['coverArea'])} | {d['declineFixedCells']} / {d['declineFoodLength']} / {d['declineCoverArea']} | {p.get('dense',0)} / {p.get('hashed indexed',0)} / {p.get('linear',0)} | {100*bounds[0]:.1f}–{100*bounds[1]:.1f}% |")
        for key in ['all world ticks','slowest 1% excluding first']:
            row=window['worldTickAttribution'][key]
            columns=[100*row['phases'].get(phase,{}).get('share',0) for phase in
                ['movement','collisions','food pickup/cache','pickup grid construction','death processing',
                 'spawn/admission','prey','ambient/maintenance','unresolved world / clock brackets']]
            slow.append(f"| {label} | {key} | {row['samples']} | "+' | '.join(f'{v:.1f}%' for v in columns)+' |')
    return {'pickup-guards':'\n'.join(guards),'slow-attribution':'\n'.join(slow),'diagnostic-overhead':'\n'.join(overhead)}


if __name__=='__main__':main()
