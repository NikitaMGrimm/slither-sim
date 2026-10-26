"""Render the selected measurement tables; narrative decisions remain in REPORT.md."""
import argparse
import json
from pathlib import Path

def f(x): return f'{x:,.3f}'
def quantiles(d): return ' / '.join(f(d[k]) for k in ['p50','p99','max'])

def tables(data):
    trials=data['trials']
    lines=['| Run | Requested / admitted / playing | Ticks/s | Deferred in hold | CPU, one core | Peak RSS MiB | Own gap p50 / p99 / max ms | Excess p50 / p99 / max ms |',
           '| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |']
    for name,d in trials.items():
        if d['scenario']=='profiled sockets': continue
        h=d['hostResources']
        lines.append(f"| {name} | {d['requested']} / {f(d['admittedTimeWeighted'])} / {f(d['clientsTimeWeighted']['playing'])} | {f(d['ticksPerSecond'])} | {d['deferredTicks']:,} | {h['cpuPercentOneCore']:.2f}% | {h['peakRssBytes']/1048576:.2f} | {quantiles(d['ownUpdateGapMs'])} | {quantiles(d['excessMs'])} |")
    lines+=['','| Run | Payload B / playing-client / s | NIC TX / RX B / playing-client / s | Generator cores / 16 | Busiest generator process, one core | Deaths / rejoins / refusals |',
            '| --- | ---: | ---: | ---: | ---: | ---: |']
    for name,d in trials.items():
        if d['scenario']=='profiled sockets': continue
        c=d['holdCounters'];g=d['generatorResources'];nic=d['hostNicBytesPerPlayingClientSecond']
        nic_text='loopback / in-process' if name=='bots-500' else f"{nic['txBytes']:.2f} / {nic['rxBytes']:.2f}"
        lines.append(f"| {name} | {d['payloadBytesPerPlayingClientSecond']:.2f} | {nic_text} | {g['cpuPercentOneCore']/100:.3f} | {max(g['perProcessCpuPercentOneCore']):.2f}% | {c.get('deaths',0)} / {c.get('rejoins',0)} / {c.get('refusals',0)} |")
    lines+=['','| Run | Deferrals at start / end | Relative world lag at start / end ms | Simulation busy-wall load | Machine busy / steal |',
            '| --- | ---: | ---: | ---: | ---: |']
    for name,d in trials.items():
        if d['scenario']=='profiled sockets':continue
        h=d['hostResources']
        lines.append(f"| {name} | {d['deferredAtStart']} / {d['deferredAtEnd']} | {d['worldLagStartMs']:.2f} / {d['worldLagEndMs']:.2f} | {d['simulationLoadPercent']:.2f}% | {h['systemBusyPercentAllCpus']:.2f}% / {h['systemStealPercentAllCpus']:.3f}% |")
    for name,p in data['profiles'].items():
        lines+=['',f'### {name}', '',f"{p['samples']:,} retained samples; weighting: {p['sampleWeight']}. Percentages use all sampled host threads.", '',
                '| Exclusive category | Share of sampled CPU |','| --- | ---: |']
        for category,value in p['categories'].items():lines.append(f"| {category} | {value['percent']:.2f}% |")
        for key,title in [('topSelf','Self'),('topApplicationInclusive','Application inclusive (overlapping)')]:
            lines+=['',f'| {title} function | Share |','| --- | ---: |']
            for row in p[key][:12]:lines.append(f"| `{row['function']}` | {row['percent']:.2f}% |")
    return '\n'.join(lines)+'\n'

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('selected',type=Path);p.add_argument('output',type=Path)
    a=p.parse_args();a.output.write_text(tables(json.loads(a.selected.read_text())),encoding='utf-8',newline='\n')
