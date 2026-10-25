"""Materialize the already-declared receipt-only exceptions and extreme prey witnesses."""
from collections import defaultdict
import analyze as a


def collect(path):
    frames, timing, active = [], [], False
    for item in a.sp.read(path):
        if isinstance(item, dict):
            if item.get('event') == 'study-timing': timing.append(item)
            if item.get('event') in ('connect', 'close'): active = False
        elif isinstance(item, a.sp.Frame):
            if any(p[:1] == b'a' for p in item.packets): active = True
            if active: frames.append(item)
    clean = a.Clean(timing)
    counts, selected = defaultdict(lambda: defaultdict(int)), defaultdict(list)
    maxima, previous, world, conn = {}, {}, None, None
    for fi, frame in enumerate(frames):
        if frame.conn != conn or any(p[:1] == b'a' for p in frame.packets):
            previous, world, conn = {}, a.sp.World(contract=False), frame.conn
        for pi, packet in enumerate(frame.packets):
            if packet[:1] == b'y': previous.pop(('prey', a.sp.u16(packet,1)), None)
            world.feed(packet,frame.server)
            for event in world.events:
                if event[0] in ('spawn', 'remove'):
                    previous.pop(('motion',event[2]),None); previous.pop(('mass',event[2]),None)
                family = {'rot':'motion','fam':'mass','prey_move':'prey'}.get(event[0])
                if family is None: continue
                sid=event[2]; key=family,sid
                current=dict(frame=fi, packet=pi, serverMs=frame.server, receiptMs=frame.t,
                             packetHex=packet.hex(), speed=event[3]['sp'] if family=='prey' else None)
                if key in previous:
                    old=previous[key]
                    sg=frame.server-old['serverMs']; rg=frame.t-old['receiptMs']
                    is_clean=clean.interval(old['receiptMs'],frame.t)
                    threshold={'motion':35,'mass':100,'prey':300}[family]
                    row=dict(conn=conn,entity=sid,first=old,last=current,serverGapMs=sg,receiptGapMs=rg,
                             clean=is_clean,interveningFrames=fi-old['frame']-1,
                             speed11=(old['speed']==11 or current['speed']==11) if family=='prey' else None)
                    for label in ['full']+(['clean'] if is_clean else []):
                        c=counts[family+'-'+label]; c['gaps']+=1
                        c['serverBelowThreshold']+=sg<threshold
                        c['receiptBelowThreshold']+=rg<threshold
                        c['receiptOnlyBelowThreshold']+=rg<threshold and sg>=threshold
                        c['serverAndReceiptBelowThreshold']+=rg<threshold and sg<threshold
                        if family=='prey':
                            c['above500ms']+=sg>500
                            k='prey-'+label
                            if k not in maxima or sg>maxima[k]['serverGapMs']: maxima[k]=row
                    category=family+('-server-short' if sg<threshold else '-receipt-only-short' if rg<threshold else '-long' if family=='prey' and sg>500 else '')
                    if is_clean and category!=family and len(selected[category])<4:
                        selected[category].append(row)
                previous[key]=current
            world.events.clear()
    return dict(counts=dict(counts),selected=dict(selected),maximumPreyGaps=maxima)
