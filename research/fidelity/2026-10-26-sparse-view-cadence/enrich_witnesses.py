"""Replay complete prefixes with the repository decoder to substantiate witnesses."""
from pathlib import Path
import analyze as a

import argparse
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--out', type=Path, required=True, help='External analysis output directory')
args = parser.parse_args()
out=args.out.resolve()
witnesses=a.load(out/'selected-witnesses.json')
inputs={s['id']:s for s in a.load(out/'inputs.json')}
for witness in witnesses.values():
    world=None; conn=None; target=int(witness['frame']); number=-1
    for frame in a.sp.read(inputs[witness['capture']]['path']):
        if not isinstance(frame,a.sp.Frame): continue
        number+=1
        if world is None or conn!=frame.conn or any(p[:1]==b'a' for p in frame.packets):
            world=a.sp.World(contract=False); conn=frame.conn
        ids=[int(s) for s in witness['reportedIds'].split(';') if s]
        if number==target:
            witness['decodedValuesBefore']={str(sid):a.snake_signature(world.snakes.get(sid)) for sid in ids}
        events=[]
        for p in frame.packets:
            world.feed(p,frame.server)
            if number==target: events.extend(world.events)
            world.events.clear()
        if number==target:
            witness['decodedValuesAfter']={str(sid):a.snake_signature(world.snakes.get(sid)) for sid in ids}
            witness['decodedEvents']=events
            assert frame.server==int(witness['server']) and frame.conn==int(witness['conn'])
            assert [p.hex() for p in frame.packets]==witness['packetsHex']
            break
a.save(out/'selected-witnesses.json',witnesses)
print('Verified five complete-prefix witnesses')
