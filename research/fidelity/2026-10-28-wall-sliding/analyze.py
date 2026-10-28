"""Frozen offline received-anchor wall comparison. No network acquisition."""
import argparse
import bisect
from collections import Counter, defaultdict
import hashlib
import importlib.util
import json
import math
import os
from pathlib import Path
import statistics
import sys

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
MODELS = ('P16', 'P15', 'P0', 'H', 'T16', 'D90')
TAU = 2 * math.pi
JITTER = (0, 7, 14, 42)
CHORD_ERROR = 84 * math.sin(math.pi / 65536)


def cpu_limit():
    if os.name == 'nt':
        import ctypes
        k = ctypes.WinDLL('kernel32', use_last_error=True)
        k.GetCurrentProcess.restype = ctypes.c_void_p
        k.SetProcessAffinityMask.argtypes = [ctypes.c_void_p, ctypes.c_size_t]
        if not k.SetProcessAffinityMask(k.GetCurrentProcess(), 1):
            raise ctypes.WinError(ctypes.get_last_error())
    else:
        os.sched_setaffinity(0, [min(os.sched_getaffinity(0))])
    os.environ.update(CARGO_BUILD_JOBS='1', RAYON_NUM_THREADS='1', RUST_TEST_THREADS='1')


def sha(path):
    with Path(path).open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def dump(path, data):
    Path(path).write_text(json.dumps(data, indent=2, allow_nan=False) + '\n', encoding='utf-8', newline='\n')


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    obj = importlib.util.module_from_spec(spec)
    sys.modules[name] = obj
    spec.loader.exec_module(obj)
    return obj


def quant(values):
    values = sorted(values)
    if not values:
        return dict(n=0, median=None, p95=None, maximum=None, rmse=None, mean=None)
    return dict(n=len(values), median=statistics.median(values),
        p95=values[math.ceil(.95 * len(values)) - 1], maximum=values[-1],
        rmse=math.sqrt(statistics.fmean(v*v for v in values)), mean=statistics.fmean(values))


def scale(n):
    return min(6., max(1., 1 + (n - 2) / 106))


def constrain(p, h, sc, radius, lead=16., head_projection=False):
    x, y = p
    dd = x*x + y*y
    length = lead * sc
    if dd < max(0., radius-length)**2:
        return p, False
    if not head_projection and math.hypot(x, y) >= radius:
        return p, True
    u, v = math.cos(h), math.sin(h)
    tx, ty = x + length*u, y + length*v
    distance = math.hypot(tx, ty)
    if distance < radius:
        return p, False
    if tx*u + ty*v > math.sin(.7)*distance:
        return p, True
    ratio = radius / distance
    return (tx*ratio-length*u, ty*ratio-length*v), False


def step(p, state, seconds, model):
    h, speed, n, radius = state
    u, v = math.cos(h), math.sin(h)
    dx, dy = speed/.032*seconds*u, speed/.032*seconds*v
    proposed = (p[0]+dx, p[1]+dy)
    if model == 'H':
        return proposed, math.hypot(*proposed) >= radius
    lead = 15. if model == 'P15' else 0. if model == 'P0' else 16.
    if model in ('T16', 'D90'):
        length = 16*scale(n)
        tip = (proposed[0]+length*u, proposed[1]+length*v)
        if math.hypot(*tip) >= radius:
            normal = (p[0]+length*u, p[1]+length*v)
            size = math.hypot(*normal)
            nx, ny = normal[0]/size, normal[1]/size
            outward = max(0., dx*nx + dy*ny)
            damping = .9 if model == 'D90' else 1.
            proposed = (p[0]+damping*(dx-outward*nx), p[1]+damping*(dy-outward*ny))
    return constrain(proposed, h, scale(n), radius, lead, model == 'P0')


def heading_present(p):
    c = chr(p[0])
    return len(p) == 6 or c in 'ed7' or c in '35' and len(p) in (3, 5)


def extract(path, sp):
    """Retain own received anchors; never call Snake.head or extrapolate a pose."""
    world, conn, life, active, order, frames = None, None, None, False, 0, 0
    lives, radius_events, protocols, events = [], [], set(), Counter()
    complete, interrupted, heading_at, origin = False, False, None, None
    near, absolute, anchors = 0, 0, 0
    relevant = set('aszrhvgnGN+=eE345d7')
    for record in sp.read(path):
        if isinstance(record, dict):
            events[record.get('event', 'header')] += 1
            complete |= record.get('event') == 'capture-end'
            interrupted |= record.get('event') == 'capture-interrupted'
            continue
        if not isinstance(record, sp.Frame):
            continue
        frames += 1
        if record.conn != conn:
            conn, world, active = record.conn, sp.World(), False
        for packet in record.packets:
            order += 1
            c = chr(packet[0])
            if c not in relevant:
                continue
            if c == 'a':
                protocols.add(packet[23])
                world, active = sp.World(), False
            old = world.snakes.get(world.self_id)
            world.feed(packet, record.server)
            pending, world.events = world.events, []
            own = world.snakes.get(world.self_id)
            if c in 'az':
                radius_events.append((record.conn, record.server, order, world.radius))
            for event in pending:
                if event[0] == 'spawn' and event[2] == world.self_id:
                    life = dict(connection=record.conn, incarnation=len(lives)+1, anchors=[], states=[],
                        terminal=None, spawnCount=own.sct)
                    lives.append(life)
                    active, heading_at = True, record.server
                    origin = math.sqrt(2)*(.2+.5*max(0, own.sct-2)) + CHORD_ERROR
                if event[0] == 'rot' and event[2] == world.self_id and heading_present(packet):
                    heading_at = record.server
                if event[0] in ('remove', 'game_over') and active and (
                        event[0] == 'game_over' or event[2] == world.self_id):
                    life['terminal'] = dict(server=record.server, precedingAnchor=life['anchors'][-1]['index'] if life['anchors'] else None,
                        kind=event[0], cause='unidentified')
                    active = False
            if active and own:
                state = (own.ang, own.sp, own.sct, world.radius)
                if not life['states'] or tuple(life['states'][-1][2:6]) != state or (
                        c in 'eE345d7' and heading_present(packet)):
                    life['states'].append((record.server, order, *state, heading_at))
                for event in pending:
                    if event[0] != 'anchor' or event[2] != world.self_id:
                        continue
                    is_absolute = c in '+='
                    origin = math.sqrt(2) if is_absolute else origin + CHORD_ERROR
                    x, y = event[3]-world.grd, event[4]-world.grd
                    distance = math.hypot(x, y)
                    alpha = math.asin(max(-1., min(1., (x*math.cos(own.ang)+y*math.sin(own.ang))/distance))) if distance else 0.
                    tangent = (x*math.sin(own.ang)-y*math.cos(own.ang))
                    gap = world.radius-distance
                    nearest = min((math.hypot(px-event[3], py-event[4]) for sid, snake in world.snakes.items()
                        if sid != world.self_id for px, py in snake.pts), default=math.inf) if gap <= 250 else math.inf
                    row = dict(index=len(life['anchors']), server=record.server, order=order,
                        frame=frames, xy=(x,y), heading=own.ang, speed=own.sp, count=own.sct,
                        radius=world.radius, alpha=alpha, direction=1 if tangent >= 0 else -1,
                        headingAt=heading_at, age=record.server-heading_at,
                        gap=gap, absolute=is_absolute, originBound=origin,
                        nearest=nearest if math.isfinite(nearest) else None)
                    life['anchors'].append(row)
                    anchors += 1
                    absolute += is_absolute
                    near += gap <= 250
    return dict(complete=complete, interrupted=interrupted, protocols=sorted(protocols), frames=frames,
        ownAnchors=anchors, nearRimAnchors=near, absoluteAnchors=absolute,
        metadataEvents=dict(events), lives=lives, radiusEvents=radius_events)


def eligible(a, b, states):
    return (50 <= b['server']-a['server'] <= 700 and
        all(-5 <= p['gap'] <= 40 and .35 <= p['alpha'] <= .68 and 0 < p['speed'] <= 14.1 and
            (p['nearest'] is None or p['nearest'] > 200) for p in (a,b)) and
        a['direction'] == b['direction'] and a['count'] == b['count'] and
        all(e[4] == a['count'] for e in states if a['order'] < e[1] <= b['order']))


def replay(a, b, states, model, dt=.007, shift=0.):
    start, end = a['server']/1000, b['server']/1000
    # Preserve packet order at equal clock times, including the initial anchor.
    initial = [e for e in states if (e[0]/1000+shift, e[1]) <= (start, a['order'])]
    current = initial[-1][2:6] if initial else states[0][2:6]
    changes = [e for e in states if (e[0]/1000+shift, e[1]) > (start, a['order']) and
        (e[0]/1000+shift, e[1]) <= (end, b['order'])]
    p, t, dead = tuple(a['xy']), start, False
    free = list(p)
    for event in changes + [(b['server'], b['order']+1, *current, 0)]:
        target = min(end, max(start, event[0]/1000+shift))
        # The terminal sentinel always reaches the received endpoint clock.
        if event[1] == b['order']+1:
            target = end
        while t < target-1e-12:
            seconds = min(dt, target-t)
            h, v, _, _ = current
            free[0] += v/.032*seconds*math.cos(h)
            free[1] += v/.032*seconds*math.sin(h)
            if not dead:
                p, dead = step(p, current, seconds, model)
            t += seconds
        current = event[2:6]
    return dict(xy=p, dead=dead, free=tuple(free))


def budget(a, b, states, j):
    seconds = (b['server']-a['server'])/1000
    middle = [e for e in states if a['order'] < e[1] <= b['order']]
    speed = max([a['speed'], b['speed']]+[e[3] for e in middle])/.032
    begin = a['server']-j-7
    end = b['server']+j+7
    previous = [e for e in states if e[0] < begin]
    radii = [a['radius'], b['radius']] + [e[5] for e in states if begin <= e[0] <= end]
    if previous:
        radii.append(previous[-1][5])
    heading_at, age = a['headingAt'], max(a['age'], b['age'])
    for e in middle:
        age = max(age, e[0]-heading_at)
        heading_at = e[6]
    parts = dict(position=a['originBound']+b['originBound'], heading=speed*seconds*TAU/256,
        speed=seconds/(18*.032), clock=speed*(.001+2*j/1000), radius=max(radii)-min(radii)+2)
    stale = speed*seconds*min(2., (.033/.008)*(age+j)/1000)
    return dict(parts=parts, held=sum(parts.values()), staleAllowance=stale,
        withStaleness=sum(parts.values())+stale, maximumHeadingAgeMs=age)


def score_lives(data, source, output):
    rows = []
    for life in data['lives']:
        states = life['states']
        for a,b in zip(life['anchors'], life['anchors'][1:]):
            if not eligible(a,b,states):
                continue
            middle = [e for e in states if a['order'] < e[1] <= b['order']]
            stable_radius = all(e[5] == a['radius'] for e in middle) and a['radius'] == b['radius']
            primary = stable_radius and max(a['age'],b['age']) <= 150
            speed_class = 'cruise' if max(a['speed'],b['speed']) < 7 else 'boost' if min(a['speed'],b['speed']) > 13.5 else 'intermediate'
            normal = (b['xy'][0]/math.hypot(*b['xy']),b['xy'][1]/math.hypot(*b['xy']))
            row = dict(source=source, connection=life['connection'], incarnation=life['incarnation'],
                first=a['index'], last=b['index'], firstFrame=a['frame'], lastFrame=b['frame'],
                seconds=(b['server']-a['server'])/1000, count=a['count'], direction=a['direction'],
                speedClass=speed_class, primary=primary, fresh42=primary and max(a['age'],b['age']) <= 42,
                budgets={j:budget(a,b,states,j) for j in JITTER}, models={}, sensitivities={})
            for model in MODELS:
                result = replay(a,b,states,model)
                error = (result['xy'][0]-b['xy'][0],result['xy'][1]-b['xy'][1])
                row['models'][model] = dict(dead=result['dead'], endpoint=math.hypot(*error),
                    radial=error[0]*normal[0]+error[1]*normal[1],
                    tangential=error[1]*normal[0]-error[0]*normal[1],
                    freeEndpoint=math.dist(result['free'],b['xy']), xy=result['xy'])
                row['sensitivities'][model] = dict(
                    step={str(dt):dict(dead=(r:=replay(a,b,states,model,dt))['dead'], error=math.dist(r['xy'],b['xy'])) for dt in (.0035,.014)},
                    eventShift={str(shift):dict(dead=(r:=replay(a,b,states,model,shift=shift/1000))['dead'], error=math.dist(r['xy'],b['xy'])) for shift in (-42,-14,-7,7,14,42)})
            output.write(json.dumps(row,allow_nan=False)+'\n')
            rows.append(row)
    return rows


def aggregate(rows):
    result = dict(intervals=len(rows), seconds=sum(r['seconds'] for r in rows),
        lives=len({(r['source'],r['connection'],r['incarnation']) for r in rows}), models={})
    for model in MODELS:
        live = [r for r in rows if not r['models'][model]['dead']]
        result['models'][model] = dict(modelLethal=len(rows)-len(live),
            aliveEndpoint=quant([r['models'][model]['endpoint'] for r in live]),
            aliveRadial=quant([r['models'][model]['radial'] for r in live]),
            aliveTangential=quant([r['models'][model]['tangential'] for r in live]),
            freeEndpoint=quant([r['models'][model]['freeEndpoint'] for r in rows]),
            outsideHeldBudget42=sum(r['models'][model]['endpoint']>r['budgets'][42]['held'] for r in live),
            outsideStaleBudget42=sum(r['models'][model]['endpoint']>r['budgets'][42]['withStaleness'] for r in live))
    result['budget42'] = {name:quant([r['budgets'][42][name] for r in rows]) for name in ('held','withStaleness')}
    result['pairs'] = {}
    for model in MODELS[1:]:
        comparable = [r for r in rows if not r['models']['P16']['dead'] and not r['models'][model]['dead']]
        qualified = lambda r, key: r['models']['P16']['endpoint'] <= r['budgets'][42][key] and r['models'][model]['endpoint'] > r['budgets'][42][key]
        result['pairs'][model] = dict(aliveComparisons=len(comparable),
            separation=quant([math.dist(r['models']['P16']['xy'],r['models'][model]['xy']) for r in comparable]),
            separatedBeyondTwiceBudget42=sum(math.dist(r['models']['P16']['xy'],r['models'][model]['xy'])>2*r['budgets'][42]['held'] for r in comparable),
            rejectingIntervalsHeld=sum(qualified(r,'held') for r in comparable),
            rejectingIntervalsStale=sum(qualified(r,'withStaleness') for r in comparable))
    return result


def fixture_scores():
    data = json.loads((REPO/'tests/fixtures/boundary-contact.json').read_text(encoding='utf-8'))
    return [dict(name=c['name'], rawSha256=c['sha256'], rows=len(c['rows']),
        leads={str(lead):quant([abs(math.hypot(r['x']+lead*r['scale']*math.cos(r['heading']),
            r['y']+lead*r['scale']*math.sin(r['heading']))-r['radius']) for r in c['rows']]) for lead in (0,15,16)}) for c in data['captures']]


def power():
    """Conservative separation ceilings, not favorable synthetic point estimates."""
    rows=[]
    for n in (2,7,37,100,174,343,500):
        sc=scale(n)
        for alpha in (.35,.5,.6,.65):
            for radius in (6000,10000,14000):
                for mode,v in (('cruise',(5.39+.4*sc)/.032),('boost',14/.032)):
                    for seconds in (.3,1,3,10,30,120,1200):
                        # Optimistic independent absolute anchors. Accumulation only increases E.
                        e=2*math.sqrt(2)+2+v*seconds*TAU/256+seconds/(18*.032)+v*.085
                        p15_ceiling=sc+2*v*sc*seconds/(radius-16*sc)
                        t16_ceiling=2*v*v*.007*seconds/(radius-16*sc)
                        d90_signal=.1*v*math.cos(alpha)*seconds
                        stale=v*seconds*min(2.,(.033/.008)*(.150+.042))
                        rows.append(dict(count=n,angle=alpha,radius=radius,mode=mode,seconds=seconds,
                            twiceHeldBudget=2*e,twiceStaleBudget=2*(e+stale),
                            P15SeparationCeiling=p15_ceiling,T16SeparationCeiling=t16_ceiling,D90IdealSignal=d90_signal))
    return dict(origin='authored conditional power bounds; no official capture', designs=len(rows),
        P15MaximumCeilingOverBudget=max(r['P15SeparationCeiling']/r['twiceHeldBudget'] for r in rows),
        T16MaximumCeilingOverBudget=max(r['T16SeparationCeiling']/r['twiceHeldBudget'] for r in rows),
        D90HeldPassingDesigns=sum(r['D90IdealSignal']>r['twiceHeldBudget'] for r in rows),
        D90StalePassingDesigns=sum(r['D90IdealSignal']>r['twiceStaleBudget'] for r in rows),
        rows=rows)


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--recordings',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True)
    a=p.parse_args();cpu_limit();a.output.mkdir(parents=True,exist_ok=False)
    sp=module('wall_protocol',REPO/'tools/fidelity/slither_protocol.py')
    inventory=json.loads((REPO/'research/fidelity/2026-10-26-arena-radius/source-inventory.json').read_text(encoding='utf-8'))
    census,rows,seen=[],[],set()
    with (a.output/'intervals-private.jsonl').open('x',encoding='utf-8') as stream:
        for item in inventory:
            rel=item['archive'];source=a.recordings/rel
            entry=dict(archive=rel,expectedSha256=item['sha256'],status='excluded')
            try:
                if sha(source)!=item['sha256']:raise ValueError('hash mismatch')
                if item['sha256'] in seen:raise ValueError('duplicate archive')
                seen.add(item['sha256'])
                data=extract(source,sp)
                dump(a.output/(rel.replace('/','--')+'.decoded-private.json'),data)
                entry.update({k:data[k] for k in ('complete','interrupted','protocols','frames','ownAnchors','nearRimAnchors','absoluteAnchors')})
                entry['lives']=len(data['lives'])
                if not data['complete'] or data['interrupted'] or data['protocols']!=[19]:raise ValueError('incomplete or unsupported recording')
                selected=score_lives(data,rel,stream)
                entry.update(eligible=len(selected),primary=sum(r['primary'] for r in selected),
                    fresh42=sum(r['fresh42'] for r in selected))
                if sha(source)!=item['sha256']:raise ValueError('source changed during analysis')
                entry['status']='admitted'
                rows.extend(selected)
            except (OSError,EOFError,ValueError,KeyError,IndexError) as error:
                entry['failure']=type(error).__name__
                (a.output/(rel.replace('/','--')+'.failure-private.txt')).write_text(str(error),encoding='utf-8')
            census.append(entry)
            print(json.dumps(dict(source=rel,status=entry['status'],near=entry.get('nearRimAnchors'),primary=entry.get('primary'))),flush=True)
    cohorts={'development':lambda r:r['source'].startswith('rim-angle-discovery-'),
        'validation':lambda r:not r['source'].startswith('rim-angle-discovery-')}
    groups={}
    for name,predicate in cohorts.items():
        chosen=[r for r in rows if predicate(r)]
        groups[name]={stratum:aggregate([r for r in chosen if test(r)]) for stratum,test in (
            ('all',lambda r:True),('primary',lambda r:r['primary']),('fresh42',lambda r:r['fresh42']))}
    per_source={name:aggregate([r for r in rows if r['source']==name and r['primary']]) for name in sorted({r['source'] for r in rows})}
    rejection={}
    for model in MODELS[1:]:
        directory_counts=Counter()
        stale_counts=Counter()
        for name,s in per_source.items():
            directory_counts[name.split('/')[0]]+=s['pairs'][model]['rejectingIntervalsHeld']
            stale_counts[name.split('/')[0]]+=s['pairs'][model]['rejectingIntervalsStale']
        directories=[name for name,count in directory_counts.items() if count>=5]
        stale=[name for name,count in stale_counts.items() if count>=5]
        rejection[model]=dict(heldDirectories=directories,staleDirectories=stale,
            conditionalHeldReject=len(directories)>=2,conditionalStaleReject=len(stale)>=2)
    pow=power();dump(a.output/'power-private.json',pow);pow.pop('rows')
    result=dict(schemaVersion=1,simulatorBase='d66ebc08ecc557187748280e2b8745aaabd92f2c',
        clientRevision='8d0772c015d6b9b73b80253db8c7349aa600b201',
        planCommit='372f7171',planSha256=sha(HERE/'PLAN.md'),
        fixture=fixture_scores(),inventory=census,cohorts=groups,perSourcePrimary=per_source,
        rejection=rejection,power=pow,sourceSha256={str(f.relative_to(REPO)):sha(f) for f in (HERE/'analyze.py',REPO/'src/boundary.rs',REPO/'tools/fidelity/slither_protocol.py',REPO/'tests/fixtures/boundary-contact.json')},
        decision='retrospective conditional comparison; no mechanics correction or official capture')
    dump(a.output/'selected-results.json',result)
    print(json.dumps(dict(eligible=len(rows),primary=sum(r['primary'] for r in rows),decision=result['decision'])))


if __name__=='__main__':main()
