"""Reduce perf-script leaf-first stacks, retaining explicit category rules.

Categories are mutually exclusive sample attribution, not instrumented phase timers.
Allocation wins first, then socket/framing and specific visibility/food work,
then world stepping and remaining per-recipient encoding. Unknown stacks stay unknown.
"""
import argparse
from collections import Counter
import json
from pathlib import Path
import re

FRAME = re.compile(r'^\s*[0-9a-f]+\s+(.+?)\s+\((.+)\)\s*$')

def category(stack):
    stack=[name.replace('>::','::') for name in stack]
    leaf = stack[0] if stack else ''
    leaf_base=leaf.split('<',1)[0]
    joined = '\n'.join(stack)
    stems={name.split('<',1)[0] for name in stack}
    flush_inline=bool(stems & {'flush','send_due','write_out_buffer','write_pending','write_frame','buffer_frame'})
    if any(x in leaf_base for x in ['malloc','calloc','realloc','free','__rust_alloc','__rust_dealloc','alloc::alloc::','alloc::raw_vec::','_int_malloc','_int_free','unlink_chunk','sysmalloc','tcache_']):
        return 'allocation'
    if any(x in joined for x in ['slither_sim::client::server::Gateway','::Socket::service','::Socket::receive','::Session::service','::Session::receive','::Gate::','accept4','tcp_recvmsg','__sys_recvfrom']):
        # Framing/flush is split out of the gateway's inclusive service stack.
        if flush_inline or any(x in joined for x in ['tungstenite::protocol::frame::','::flush','::send_due','tcp_sendmsg','__sys_sendto']):
            return 'framing/flush'
        return 'socket service'
    if any(x in joined for x in ['tungstenite::protocol::frame::','::send_due','::flush','tcp_sendmsg','__sys_sendto']):
        return 'framing/flush'
    if stems & {'food_updates','sync_food_sectors','covered','eaten_sectors','food_identity'} or any(x in joined for x in ['::food::','::visibility::','::coverage::','::CoverageGrid::','::KnownSectors::','::FoodIndex::','::sync_food_sectors','::FoodInterest::','::snakes::covered']):
        return 'visibility/food-sector work'
    if 'step' in stems or any(name.startswith(('advance<false>','advance<true>')) for name in stack) or any(x in joined for x in ['::Simulator::step','slither_sim::simulator::','slither_sim::food_grid::','slither_sim::prey::','slither_sim::spawn::']):
        return 'physics'
    if stems & {'updates','broadcast_with','changes','emit','emit_presented','minimap','hud'} or any(x in joined for x in ['::simulation::snakes::','::simulation::connection::','slither_sim::client::encode::','::SimulatorBackend::updates','::SimulatorBackend::broadcast_with']) or any('::simulation::broadcast::' in f and any(x in f for x in ['::updates','::broadcast_with','::minimap','::hud']) for f in stack):
        return 'per-recipient delta encoding'
    return 'other/unknown'

def read_samples(path, window=None):
    stack=[]
    period=None
    at=None
    event=False
    with path.open(encoding='utf-8',errors='replace') as f:
        for line in f:
            if not line.strip():
                if event and (window is None or at is not None and window[0] <= at < window[1]): yield period, stack or ['[no callchain]']
                stack=[]; period=None; at=None; event=False
                continue
            match=FRAME.match(line)
            if match:
                symbol=re.sub(r'\+0x[0-9a-f]+$', '',match[1])
                stack.append(symbol)
            elif 'cpu-clock:' in line or 'cycles:' in line:
                event=True
                match=re.search(r'\b(\d+)\s+(?:cpu-clock|cycles):',line)
                if match: period=int(match[1])
                clock=re.search(r'\s(\d+\.\d+):',line)
                if clock: at=float(clock[1])
        if event and (window is None or at is not None and window[0] <= at < window[1]): yield period, stack or ['[no callchain]']

def reduce(path, window=None):
    own, inclusive, categories = Counter(), Counter(), Counter()
    own_n, inclusive_n, category_n = Counter(), Counter(), Counter()
    total=0; total_weight=0; periods=0
    for period, stack in read_samples(path,window):
        total+=1
        weight = period if period is not None else 1
        periods += period is not None
        total_weight += weight
        own[stack[0]]+=weight; own_n[stack[0]]+=1
        for name in set(stack): inclusive[name]+=weight; inclusive_n[name]+=1
        group=category(stack)
        categories[group]+=weight; category_n[group]+=1
    if total == 0:
        raise ValueError('No parsed perf samples; inspect perf record/script output')
    if periods not in [0,total]: raise ValueError('Mixed weighted/unweighted samples')
    if 'period' in path.name and periods != total: raise ValueError('Period export did not preserve sample weights')
    def top(weights, counts, application=False):
        entries=[(name,n) for name,n in sorted(weights.items(),key=lambda item:(-item[1],item[0])) if not application or name.startswith(('slither_sim::','<slither_sim::'))]
        return [{'function':name,'samples':counts[name],'weight':n,'percent':100*n/total_weight} for name,n in entries[:30]]
    event='cycles' if 'cycles:' in path.read_text(encoding='utf-8',errors='replace')[:4096] else 'cpu-clock'
    return {'event':event,'sampleWindowPerfSeconds':window,'sampleWeight':('hardware cycles' if event=='cycles' else 'cpu-clock period nanoseconds') if periods else 'sample count', 'samples':total,'totalWeight':total_weight,
            'topSelf':top(own,own_n),'topInclusive':top(inclusive,inclusive_n),
            'topApplicationSelf':top(own,own_n,True),'topApplicationInclusive':top(inclusive,inclusive_n,True),
            'categories':{name:{'samples':category_n[name],'weight':n,'percent':100*n/total_weight} for name,n in sorted(categories.items(),key=lambda item:(-item[1],item[0]))},
            'limits':'Categories use retained call chains and ordered rules in profile.py; allocation is removed from its callers. Inline attribution, incomplete unwind and kernel symbol restrictions limit precision. Inclusive percentages overlap.'}

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('stacks',type=Path);p.add_argument('output',type=Path)
    a=p.parse_args();a.output.write_text(json.dumps(reduce(a.stacks),indent=2)+'\n')
