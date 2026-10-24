"""Bounded, offline audit of protocol-19 field grids in retained captures.

    python -B probe.py ARCHIVE_DIR OUTPUT_JSON

ARCHIVE_DIR holds the three `<capture>/bot0.jsonl.gz` recordings named in
REPORT.md; they are local-only. The output JSON contains capture-derived values
and stays outside Git.
"""
import collections
import hashlib
import json
import math
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'tools/fidelity'))
import slither_protocol as sp

ARCHIVE = Path(sys.argv[1])
PATHS = [ARCHIVE / name / 'bot0.jsonl.gz' for name in (
    'fidelity-paired-coil-20260927', 'prey-controlled-20260927-first',
    'anchor-encoder-holdout-20260928')]
STATS = {}
SAVINGS = collections.defaultdict(list)
DECODED_CHECKS = collections.Counter()
OFF_GRID = collections.Counter()

def check(name, value, step, offset=0, raw=None):
    s = STATS.setdefault(name, dict(n=0, violations=0, maxGridResidual=0,
        min=value, max=value, first=value, rawFirst=raw, rawDifferenceGcd=0))
    residual = abs((value - offset) / step - round((value - offset) / step))
    s['n'] += 1
    s['violations'] += residual > 1e-7
    s['maxGridResidual'] = max(s['maxGridResidual'], residual)
    s['min'], s['max'] = min(s['min'], value), max(s['max'], value)
    if raw is not None:
        s['rawDifferenceGcd'] = math.gcd(s['rawDifferenceGcd'], abs(raw-s['rawFirst']))

def code(name, raw, step, offset=0):
    check(name, offset + raw * step, step, offset, raw)

def angle24(p, at, name='angle24'):
    code(name, sp.u24(p, at), math.tau/16777215)

def speed16(p, at, name):
    code(name, sp.u16(p, at), 1/32)

def fam(p, at):
    code('fam24', sp.u24(p, at), 1/16777215)

def bytes_sample(group, value, places):
    before = json.dumps(value, separators=(',', ':'))
    after = format(value, f'.{places}f').rstrip('0').rstrip('.')
    if after == '-0':
        after = '0'
    SAVINGS[group].append(len(before) - len(after))

reports = []
for path in PATHS:
    counts = collections.Counter()
    shapes = collections.Counter()
    setup = []
    own_forms = collections.Counter()
    world, conn, frames, packets, records = sp.World(), None, 0, 0, 0
    known_sector = None
    for event in sp.read(path):
        records += 1
        if not isinstance(event, sp.Frame):
            continue
        if conn != event.conn:
            conn, world, known_sector = event.conn, sp.World(), None
        frames += 1
        for p in event.packets:
            packets += 1
            c, n = chr(p[0]), len(p)
            counts[c] += 1
            shapes[f'{c}:{n}'] += 1
            if c == 'a':
                assert p[23] == 19
                setup.append(dict(protocol=p[23], sector=sp.u16(p,6), spacing=p[24], grid=sp.u24(p,1)))
            elif c == 's' and n > 7:
                for at in (16,19): code('snakeSpawnXY', sp.u24(p,at), .2)
                for at in (3,7): angle24(p,at,'snakeSpawnAngle24')
                speed16(p,10,'snakeSpawnSpeed16')
                fam(p,12)
                m=23+p[22]
                m += 1+p[m]
                m += 1
                if m<n:
                    for at in (m,m+3): code('bodyFirstXY',sp.u24(p,at),.2)
                    m+=6
                    while m<n:
                        if m+2==n:
                            code('bodyDirection16',sp.u16(p,m),math.tau/65536)
                        else:
                            for at in (m,m+1): code('bodyRelativeXY',p[at]-127,.5)
                        m+=2
            elif c in 'gGnN+=':
                own = c in 'GN' or (c=='=' and n==7) or (c=='+' and n==10)
                m=1 if own else 3
                own_forms[f'{"own" if own else "id"}:{c}:{n}']+=1
                if c in '+=':
                    code('anchorDirection16',sp.u16(p,m),math.tau/65536)
                    for at in (m+2,m+4): code('anchorAbsoluteXY',sp.u16(p,at),1)
                    m+=6
                elif n-m in (2,5):
                    code('anchorDirection16',sp.u16(p,m),math.tau/65536)
                    m+=2
                if c in 'nN+': fam(p,m)
            elif c in 'eE345d7':
                own=c in 'd7' or n-1<=2
                m=1 if own else 3
                angle_at,speed_at=[],None
                if c in 'd7' or n==6: angle_at,speed_at=[m,m+1],m+2
                elif n in (5,3):
                    if c in 'eE4': angle_at,speed_at=[m],m+1
                    else: angle_at=[m,m+1]
                elif n in (4,2):
                    if c in 'eE4': angle_at=[m]
                    elif c=='3': speed_at=m
                for at in angle_at: code('snakeUpdateAngle8',p[at],math.tau/256)
                if speed_at is not None: code('snakeUpdateSpeed8',p[speed_at],1/(18*.032))
            elif c in 'hr' and n==6: fam(p,3)
            elif c=='F':
                for m in range(3,n,4):
                    for sector,raw in ((p[1],p[m+1]),(p[2],p[m+2])):
                        value=sector*world.sector_size+raw*(world.sector_size/256)
                        check('foodXY',value,world.sector_size/256,raw=sector*256+raw)
                    code('foodSize',p[m+3],.2)
            elif c in 'bf':
                if n-1 in (5,6): known_sector=(p[1],p[2]);m=3
                else: m=1
                if known_sector:
                    for sector,raw in zip(known_sector,p[m:m+2]):
                        check('foodXY',sector*world.sector_size+raw*(world.sector_size/256),world.sector_size/256,raw=sector*256+raw)
                code('foodSize',p[-1],.2)
            elif c=='y' and n==20:
                for at in (4,7): code('preySpawnXY',sp.u24(p,at),.2)
                for at in (12,15): angle24(p,at,'preyAngle24')
                speed16(p,18,'preySpeed16')
                code('preySize',p[10],.2)
            elif c=='j':
                for at in (3,5): code('preyUpdateXY',sp.u16(p,at),3,1)
                angle_at,speed_at={6:([],None),8:([],7),9:([7],None),10:([8],None),11:([7],10),12:([8],11),13:([8,11],None),15:([8,11],14)}[n-1]
                for at in angle_at: angle24(p,at,'preyAngle24')
                if speed_at is not None: speed16(p,speed_at,'preySpeed16')
            elif c=='z': code('arenaRadius',sp.u24(p,1),1)
            world.feed(p,event.server)
            if c == 's' and n > 7:
                snake = world.snakes[sp.u16(p,1)]
                pairs = [(snake.base[0],sp.u24(p,16)/5),
                         (snake.base[1],sp.u24(p,19)/5),
                         (snake.ang,sp.u24(p,3)*math.tau/16777215),
                         (snake.sp/.032,sp.u16(p,10)/1000/.032)]
                for actual,expected in pairs:
                    assert math.isclose(actual,expected,rel_tol=0,abs_tol=1e-10)
                    DECODED_CHECKS['snakeSpawnScalars'] += 1
            if c in 'yj' and sp.u16(p,1) in world.prey and (c=='j' or n==20):
                prey = world.prey[sp.u16(p,1)]
                step,offset = (.2,0) if c=='y' else (3,1)
                for axis in ('x','y'):
                    residual = abs((prey[axis]-offset)/step-round((prey[axis]-offset)/step))
                    assert residual < 1e-7
                    DECODED_CHECKS['preyPositionScalars'] += 1
            world.events.clear()
        if frames%100==0:
            for snake in world.snakes.values():
                for value in snake.head(event.server):
                    bytes_sample('predictedHeadXY',value-world.grd,2)
                    OFF_GRID['headTotal'] += 1
                    OFF_GRID['headNotPoint2'] += abs(value/.2-round(value/.2))>1e-7
                for xy in snake.pts:
                    for value in xy:
                        bytes_sample('reconstructedBodyXY',value-world.grd,2)
                        OFF_GRID['bodyTotal'] += 1
                        OFF_GRID['bodyNotPoint2'] += abs(value/.2-round(value/.2))>1e-7
                bytes_sample('snakeHeading',snake.ang,4)
                bytes_sample('snakeSpeed',snake.sp/.032,2)
            for x,y,size in world.foods.values():
                for value in (x,y): bytes_sample('foodXY',value-world.grd,2)
            for prey in world.prey.values():
                for axis in ('x','y'): bytes_sample('preyXY',prey[axis]-world.grd,2)
                bytes_sample('preyHeading',prey['ang'],4)
                bytes_sample('preySpeed',prey['sp']/.032,2)
        if frames>=5000: break
    reports.append(dict(path=str(path),sha256=hashlib.sha256(path.read_bytes()).hexdigest(),
        recordsThroughPrefix=records,receivedFrames=frames,logicalPackets=packets,
        setup=setup,commandCounts=dict(counts),anchorForms=dict(own_forms)))

result=dict(method='first 5000 incoming timed frames per archive, all logical packets; raw fields, not all reconstructed coordinates',
    captures=reports,stats=STATS,decodedChecks=dict(DECODED_CHECKS),offGridReconstruction=dict(OFF_GRID),savings={k:dict(n=len(v),mean=sum(v)/len(v),min=min(v),max=max(v)) for k,v in SAVINGS.items()},
    scales=dict(minSpeed=5.79/.032,boostSpeed=14/.032,cruiseStep=5.79/.032*.007,
        boostStep=14/.032*.007,turnPerTick=.033/.008*.007,
        anchorDirectionStep=42*math.tau/65536,preyTurnPerTick=.029/.008*.007))
Path(sys.argv[2]).write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
print(json.dumps(result,indent=2))
