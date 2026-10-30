"""Conservative source-context buckets for the separate user-CPU samples."""
import collections
import json
from pathlib import Path
import re
import sys


def records(path):
    record=[]
    with Path(path).open() as stream:
        for line in stream:
            if line.strip():record.append(line)
            elif record:
                yield ''.join(record);record=[]
        if record:yield ''.join(record)


def operation_phase(record):
    if not any(name in record for name in ['5Scene4tick','::Scene::tick']):return None
    for phase,names in [
        ('A4',['3bot6kernel','7Forager6decide','::bot::kernel::','::bot::Forager::decide']),
        ('observation',['11observation','::observation::']),
        ('admission',['5Scene9admission','::Scene::admission']),
        ('world',['9simulator','::simulator::']),
    ]:
        if any(name in record for name in names):return phase
    return 'other operation'


def classify(root):
    root=Path(root)
    lines=(root/'src/observation/mod.rs').read_text().splitlines()
    def locate(text):
        found=[i+1 for i,line in enumerate(lines) if text in line]
        assert len(found)==1,(text,found)
        return found[0]
    ranges={
        'food order restoration':(locate('for (word, bits) in marks.iter_mut()'),locate('for (i, p) in w.prey.iter()')-1),
        'prey candidate scan':(locate('for (i, p) in w.prey.iter()'),locate('let visible_opponents =')-1),
        'snake candidate scan':(locate('let visible_opponents ='),locate('for i in visible_opponents {')+2),
        'food candidate queries':(locate('for row in grid.row(b.min.y)'),locate('// Ascending bit order restores')-1),
    }
    overhead=json.loads((root/'diagnostic/overhead.json').read_text())
    result={'attribution':'sampled user CPU, source context; inclusive subfunctions assigned once; not removable shares',
            'sourceRanges':ranges,'windows':[]}
    for n in [1000,10000]:
        all_samples=collections.Counter();phases=collections.Counter();observations=collections.Counter()
        for record in records(root/f'diagnostic/{n}-stacks.txt'):
            match=re.search(r':\s+(\d+) cpu-clock:u:',record.splitlines()[0])
            if not match:continue
            weight=int(match[1]);all_samples['all']+=weight
            phase=operation_phase(record)
            if phase is None:
                all_samples['outside operation or unwound']+=weight
                continue
            all_samples['operation']+=weight
            phases[phase]+=weight
            if phase=='observation':
                if any(name in record for name in ['8FoodGrid3new','::FoodGrid::new']):label='food grid construction'
                elif any(name in record for name in ['8Observer3new','::Observer::new']):label='other shared-index construction'
                else:
                    matches=re.findall(r'/src/observation/mod\.rs:(\d+)',record)
                    line=int(matches[0]) if matches else None
                    label=next((key for key,(first,last) in ranges.items() if line is not None and first<=line<=last),
                               'other observation / copying / unresolved source')
                observations[label]+=weight
        assert all_samples['all']>0,'No recognized cpu-clock periods; inspect perf script format'
        assert all_samples['operation']>0,'No tick stacks recognized; inspect symbols/unwinding'
        report=(root/f'diagnostic/{n}-report.txt').read_text()
        lost=re.search(r'Total Lost Samples:\s*([\d,]+)',report)
        def shares(counts,total):
            return {key:{'sampleNanoseconds':value,'share':value/total if total else None}
                    for key,value in counts.most_common()}
        result['windows'].append({'population':n,'overhead':next(v for v in overhead if v['population']==n),
             'lostSamples':int(lost[1].replace(',','')) if lost else None,'coverage':dict(all_samples),
             'operationPhases':shares(phases,all_samples['operation']),
             'observationBuckets':shares(observations,phases['observation'])})
    (root/'diagnostic/selected.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(result,indent=2))


if __name__=='__main__':classify(sys.argv[1])
