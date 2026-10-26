"""Bounded selection and tables for the fixed offline gate diagnostic."""
import json
import math
from pathlib import Path

P2_COMMIT = "6051b4c0fbb4630b126001309c64712761c0c6af"
BUCKETS = ["1", "2", "3", "4", "5", "6", "open", "reopen-boundary", "report-in-clock-interval", "no-prior"]


def quantile(values, q):
    return sorted(values)[max(0, math.ceil(len(values)*q)-1)] if values else None


def summarize(rows):
    return dict(n=len(rows),
        lowerMs=[round(v,3) if v is not None else None for q in (.5,.9,.95)
                 for v in [quantile([r['reflectionDelayLower'] for r in rows],q)]],
        upperMs=[round(v,3) if v is not None else None for q in (.5,.9,.95)
                 for v in [quantile([r['reflectionDelayUpper'] for r in rows],q)]],
        armed=sum(r['state']=='armed' for r in rows),
        early=sum(r['contradictionUpperBound'] for r in rows))


def groups(rows):
    for n in range(1,7):
        for kind in ('wanted','boost'):
            yield f'ordinal-{n:02d}/{kind}', [r for r in rows if r['split']!='older' and r['ordinal']==n and r['kind']==kind]
    for cohort in ('development','heldout','p2-pooled','older'):
        if cohort=='older' and not any(r['split']=='older' for r in rows):
            continue
        for kind in ('wanted','boost'):
            yield cohort+'/'+kind, [r for r in rows if r['kind']==kind and
                (r['split']==cohort or cohort=='p2-pooled' and r['split']!='older')]


def screened(r):
    return (r['clockWidthMs']<=10 and not r['reflectionGrowthTail'] and not r['previousGrowthTail']
        and not r['reflectionDirectionOnly'] and not r['previousDirectionOnly']
        and r.get('wantedActuallyChangesAtReflection',r.get('speedActuallyChangesAtReflection',False))
        and r.get('encodedWantedChangesAtSend',True) and r['commandActiveAtSend'])


def select(rows):
    result = dict(quantiles=[50,90,95], units='ms',
                  tickLabels='Nominal midpoint ticks remaining at mapped send, not measured ingress', groups={})
    for name, group in groups(rows):
        armed=[r for r in group if r['state']=='armed']
        result['groups'][name]=dict(all=summarize(group),
            buckets={b:summarize([r for r in group if r['bucket']==b]) for b in BUCKETS},
            screens=summarize([r for r in group if screened(r)]),
            clockAtMost10=summarize([r for r in group if r['clockWidthMs']<=10]),
            absoluteEarly=sum(r['contradictionAbsolutePaired'] for r in armed),
            earlyWith2msMargin=sum(r['contradictionPlus2ms'] for r in armed),
            earlyWith35msFloor=sum(r['contradiction35msFloor'] for r in armed),
            screenedEarlyWith35msFloor=sum(r['contradiction35msFloor'] and screened(r) for r in armed),
            motionGap41to44=sum(41<=r['reflectionServerMs']-r['previousMotionServerMs']<=44 for r in armed),
            tickBinCertain=sum(len(set(r['ticksRemainingRange']))==1 for r in armed),
            speedWithoutWanted=summarize([r for r in group if not r['reflectionChangesWantedAndSpeed']]),
            nextFrameDefinite=sum(r['nextEmittedFrameAfterSendDefinite'] for r in armed),
            nextFramePossible=sum(r['nextEmittedFrameAfterSendPossible'] for r in armed))
    return result


def tables(rows):
    text = ('# Bucket tables\n\nNearest-rank endpoint quantiles in ms; midpoint tick labels apply only to mapped send.\n'
            'See REPORT.md for selection, ambiguity bins and the conservative early test.\n'
            'These command-pooled quantities are descriptive; empty quantiles are undefined.\n\n')
    for name, group in groups(rows):
        text+=f'## {name}\n\n| Bucket | n | p50 lower–upper | p90 lower–upper | p95 lower–upper | Early / armed |\n| --- | ---: | --- | --- | --- | --- |\n'
        for bucket in BUCKETS:
            s=summarize([r for r in group if r['bucket']==bucket])
            intervals=['—' if lo is None else f'{lo:.3f}–{hi:.3f}' for lo,hi in zip(s['lowerMs'],s['upperMs'])]
            fraction=f"{s['early']}/{s['armed']}" if s['armed'] else '—'
            text+=f"| {bucket} | {s['n']} | {' | '.join(intervals)} | {fraction} |\n"
        text+='\n'
    return text.rstrip()+'\n'


def write_json(path, value):
    # One group per line keeps bounded summaries legible without trace-sized output.
    if 'groups' in value:
        header={k:v for k,v in value.items() if k!='groups'}
        prefix=json.dumps(header,ensure_ascii=True)[:-1]
        text=prefix+', "groups": {\n'+',\n'.join(json.dumps(k)+': '+json.dumps(v,ensure_ascii=True)
                for k,v in value['groups'].items())+'\n}}\n'
    else:
        text=json.dumps(value,indent=2,ensure_ascii=True)+'\n'
    path.write_text(text,encoding='utf-8',newline='\n')
