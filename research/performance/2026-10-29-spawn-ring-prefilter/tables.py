"""Reduce fixed-work pairs; actual event groups and CPU slow tails stay separate."""
from pathlib import Path
import json,math,statistics,sys
root=Path(sys.argv[1]);output={}
fixture=sys.argv[2] if len(sys.argv)>2 else 'spawn'

def q(values,p):
    values=sorted(values);return values[math.ceil((len(values)-1)*p)]

for protocol in [19,13]:
    pairs=json.loads((root/f'pairs-{fixture}-{protocol}.json').read_text());rows=[]
    assert len(pairs)==6 and pairs[0]['warmup'] and [r['pair'] for r in pairs[1:]]==list(range(5))
    for pair in pairs:
        result={'pair':pair['pair'],'warmup':pair['warmup']}
        for arm in ['base','candidate']:
            d=json.loads((root/pair['paths'][arm]).read_text());ticks=d['ticks']
            groups={'all':ticks,'search':[r for r in ticks if r['sample']['events'][0]],
                'exhausted':[r for r in ticks if r['sample']['events'][2]],
                'other':[r for r in ticks if not r['sample']['events'][0]],
                'foodEpoch':[r for r in ticks if r['foodEpoch']],
                'minimap':[r for r in ticks if r['minimap']],
                'marker':[r for r in ticks if r['marker']],
                'slowestCpu1Percent':sorted(ticks,key=lambda r:r['sample']['cpuNs'],reverse=True)[:math.ceil(len(ticks)*.01)]}
            result[arm]={}
            for name,group in groups.items():
                if not group:result[arm][name]={'n':0};continue
                instructions=[r['sample']['instructions'] for r in group];cpu=[r['sample']['cpuNs']/1e6 for r in group]
                result[arm][name]={'n':len(group),'instructionsMean':statistics.mean(instructions),
                    'cpuMs':{'p50':q(cpu,.5),'p99':q(cpu,.99),'max':max(cpu)}}
        rows.append(result)
    output[str(protocol)]={'pairs':rows,'pairedInstructions':{}}
    for group in ['all','search','exhausted','other','foodEpoch','minimap','marker','slowestCpu1Percent']:
        ratios=[r['candidate'][group]['instructionsMean']/r['base'][group]['instructionsMean'] for r in rows[1:]
                if r['base'][group]['n'] and r['candidate'][group]['n']]
        if ratios:
            output[str(protocol)]['pairedInstructions'][group]={'ratioMedian':statistics.median(ratios),'ratioMin':min(ratios),'ratioMax':max(ratios),'n':len(ratios)}
(root/f'selected-fixed-{fixture}.json').write_text(json.dumps(output,indent=2)+'\n')
print(json.dumps({p:v['pairedInstructions'] for p,v in output.items()},indent=2))
