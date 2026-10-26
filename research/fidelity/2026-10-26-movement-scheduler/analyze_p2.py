"""Descriptive P2 coverage and observable constraints; never invent dirty times."""
import argparse
from bisect import bisect_left, bisect_right
from collections import Counter, defaultdict
import hashlib
import json
import math
from pathlib import Path
import sys

MOTION=set('eE345d7')
FIELDS=('heading','wanted','speed')

def quantile(values,q):
    return sorted(values)[max(0,math.ceil(q*len(values))-1)] if values else None

def summary(values):
    return dict(n=len(values),min=min(values) if values else None,p50=quantile(values,.5),
                p95=quantile(values,.95),max=max(values) if values else None)

def angular_distance(a,b,period=256):
    return abs((a-b+period/2)%period-period/2)

def motion_form(packet,own_id):
    code=chr(packet[0]); n=len(packet)
    if code not in MOTION:return None
    own=code in 'd7' or n-1<=2
    sid=own_id if own else int.from_bytes(packet[1:3],'big')
    m=1 if own else 3
    if n==6 or (n in (4,2) and code in 'd7'):mask=(1,1,1)
    elif n in (5,3) and code in 'eE435':mask=(1,0,1) if code=='e' else (0,1,1) if code in 'E4' else (1,1,0)
    elif n in (4,2) and code in 'eE43':mask=(1,0,0) if code=='e' else (0,1,0) if code in 'E4' else (0,0,1)
    else:raise ValueError('Unsupported motion form')
    values={}; cursor=m
    for field,present in zip(FIELDS,mask):
        if present:values[field]=packet[cursor];cursor+=1
    assert cursor==n
    direction=None
    if mask[1] and not(n==6 and code not in 'eE'):
        direction=2 if (n==6 and code=='E') or (n!=6 and code in '457') else 1
    return dict(sid=sid,own=own,mask=''.join(f[0] for f,p in zip(FIELDS,mask) if p),values=values,direction=direction)

def finish_heading(command,ended_at,reason):
    command['heldMs']=ended_at-command['sentReceiptMs']
    command['holdEndedBy']=reason
    command['isolatedQualified']=bool(command['priorWantedByte'] is not None and
        command['ownActiveAtSend'] and
        command['priorTargetDistance']>2.1 and command['heldMs']>=150 and
        command['firstReflection'] is not None)
    if command['firstReflection'] is not None:command['status']='reflected'
    elif reason=='next-heading':command['status']='superseded-before-reflection'
    else:command['status']='right-censored'
    if command['priorTargetDistance'] is not None and command['priorTargetDistance']<=2.1:
        command['status']='unchanged-or-small-change'

def merge_windows(windows):
    result=[]
    for lo,hi in sorted(windows):
        if result and lo<=result[-1][1]:result[-1][1]=max(hi,result[-1][1])
        else:result.append([lo,hi])
    return result

def telemetry_mask(rows):
    header=next(r for r in rows if isinstance(r,dict) and r.get('type')=='header')
    timing=[r for r in rows if isinstance(r,dict) and (r.get('event')=='collector-timing' or r.get('event')=='capture-end')]
    windows=[]; previous=header['t']; late=0
    for row in timing:
        value=row.get('collectorTiming',row)
        if 'lateTicks' not in value:windows.append([previous-100,row['t']+100])
        else:
            if value['lateTicks']>late:windows.append([previous-100,row['t']+100])
            if row['t']-previous>1500:windows.append([previous-100,row['t']+100])
            late=value['lateTicks']
        previous=row['t']
    terminal=rows[-1]
    if terminal.get('integrity',{}).get('backpressureWrites',0):
        windows.append([header['t'],terminal['t']])
    for row in rows:
        if isinstance(row,dict) and row.get('event') in ('receive-error','receive-timeout'):
            windows.append([row['t']-100,row['t']+100])
    if not timing:windows.append([header['t'],terminal['t']])
    return merge_windows(windows)

def phase_residual(server_ms,period,phase):
    return abs((server_ms-phase+period/2)%period-period/2)

def best_phase(times,period):
    if not times:return None
    return min(range(period),key=lambda phase:sum(phase_residual(t,period,phase) for t in times))

def analyze(path,sp):
    records=list(sp.read(path)); archive_sha=hashlib.sha256(path.read_bytes()).hexdigest()
    windows=telemetry_mask(records)
    def clean(lo,hi):return not any(a<=hi and b>=lo for a,b in windows)
    world=sp.World(contract=False); conn=0; line=0; life=0; alive=False; setup_at=None;initialized=False
    joined=0.; joined_intervals=[]; pending_heading=None; pending_boost=None
    headings=[]; boosts=[]; pickups=[]; pending_pickups=[]; boost_state=False
    prior={}; direction={}; incarnations=Counter(); epoch=0; controls=None
    counts=Counter(); strata=Counter(); gaps=defaultdict(list); field_gaps=defaultdict(list)
    witnesses=defaultdict(list); motion_rows=[]; mass_rows=[]; order_rows=[]; prey_rows=[]
    context=defaultdict(dict); frames_by_conn=defaultdict(list); pings_by_conn=defaultdict(list)
    outstanding_ping=None
    def witness(kind,row):
        counts[kind]+=1
        # Earliest witness only; every exception remains in the full event ledger.
        if not witnesses[kind]:witnesses[kind].append(row)
    def finish_pending(t,reason):
        nonlocal pending_heading,pending_boost,pending_pickups,alive,joined,setup_at
        if pending_heading is not None:
            finish_heading(pending_heading,t,reason);pending_heading=None
        if pending_boost is not None:
            pending_boost['endedAt']=t;pending_boost['endReason']=reason;pending_boost=None
        for pickup in pending_pickups:pickup['rightCensored']=True;pickup['censoredAt']=t
        pending_pickups=[]
        if alive and setup_at is not None:
            joined+=t-setup_at;joined_intervals.append((setup_at,t))
        alive=False;setup_at=None
    for line,item in enumerate(records,1):
        if isinstance(item,dict):
            event=item.get('event');t=item.get('t')
            if event=='connect':
                finish_pending(t,'reconnect');world=sp.World(contract=False);prior={};direction={};context=defaultdict(dict)
                boost_state=False;life=item.get('life',life+1);controls=None;outstanding_ping=None;initialized=False
            elif event=='movement-control':
                controls=item;strata[f"control/{item.get('tier')}/{item.get('phase')}/{item.get('interruption') or 'uninterrupted'}"]+=1
                if pending_boost and pending_boost['firstDirectionalSpeedReflection'] is None and item.get('interruption'):
                    pending_boost['interruptedBeforeReflection']=True
            elif event in ('close','capture-end','capture-interrupted'):
                finish_pending(t,event)
            continue
        if isinstance(item,sp.Out):
            if len(item.data)==1:
                byte=item.data[0]
                if byte==251:outstanding_ping=item.t
                elif byte<=250:
                    if pending_heading is not None:finish_heading(pending_heading,item.t,'next-heading')
                    own=world.snakes.get(world.self_id)
                    prior_wanted=own.wang*256/sp.TAU if own and alive else None
                    target=math.floor(byte*256/251)
                    pending_heading=dict(line=line,connection=item.conn,life=life,epoch=epoch,sentReceiptMs=item.t,
                        byte=byte,targetWantedByte=target,priorWantedByte=prior_wanted,
                        priorTargetDistance=angular_distance(target,prior_wanted) if prior_wanted is not None else None,
                        firstReflection=None,phase=controls.get('phase') if controls else None,
                        tier=controls.get('tier') if controls else None,ownActiveAtSend=alive)
                    headings.append(pending_heading)
                elif byte in (253,254):
                    desired=byte==253
                    if desired!=boost_state:
                        if pending_boost is not None:pending_boost['endedAt']=item.t;pending_boost['endReason']='opposite-boost-command'
                        own=world.snakes.get(world.self_id)
                        pending_boost=dict(line=line,connection=item.conn,life=life,epoch=epoch,sentReceiptMs=item.t,
                            enabled=desired,priorSpeed=own.sp if own else None,firstDirectionalSpeedReflection=None,
                            interruptedBeforeReflection=False,sizeOrPickupChangeBeforeReflection=False,
                            ownActiveAtSend=alive,phase=controls.get('phase') if controls else None)
                        boosts.append(pending_boost);boost_state=desired
            continue
        conn=item.conn
        frame_order=defaultdict(list)
        for packet_index,packet in enumerate(item.packets):
            code=chr(packet[0]); form=motion_form(packet,world.self_id)
            if code=='a':
                finish_pending(item.t,'setup-reset');world=sp.World(contract=False);prior={};direction={};context=defaultdict(dict)
                epoch+=1;setup_at=item.t;alive=True;initialized=True
            if code=='p' and outstanding_ping is not None:
                pings_by_conn[(conn,epoch)].append((outstanding_ping,item.t,item.server));outstanding_ping=None
            reference=dict(archiveSha256=archive_sha,jsonlLine=line,connection=conn,life=life,epoch=epoch,
                serverMs=item.server,receiptMs=item.t,packetIndex=packet_index,packetHex=packet.hex())
            world.feed(packet,item.server)
            events=world.events[:];world.events.clear()
            if form is not None and not any(e[0]=='rot' for e in events):
                witness('excluded-motion-target-unknown',reference)
            if form is not None and form['sid'] is not None:
                sid=form['sid'];ctx=context[sid]
                reference.update(entity=sid,incarnation=incarnations[sid],own=sid==world.self_id,
                    fieldMask=form['mask'],direction=form['direction'],phase=controls.get('phase') if controls and sid==world.self_id else None,
                    boostCommandState=boost_state if sid==world.self_id else None,
                    sinceCreditedPickupMs=item.server-ctx['pickup'] if 'pickup' in ctx else None,
                    sinceGrowthMs=item.server-ctx['growth'] if 'growth' in ctx else None,
                    sinceTailMs=item.server-ctx['tail'] if 'tail' in ctx else None,
                    lastHeadingCommandByte=pending_heading['byte'] if pending_heading and sid==world.self_id else None,
                    headingHoldSoFarMs=item.t-pending_heading['sentReceiptMs'] if pending_heading and sid==world.self_id else None)
            for event in events:
                kind=event[0]
                if kind in ('spawn','remove'):
                    sid=event[2]
                    prior={k:v for k,v in prior.items() if len(k)<2 or k[1]!=sid};context.pop(sid,None)
                    if kind=='spawn':
                        incarnations[sid]+=1
                        direction[sid]=packet[6]-48 if 48<=packet[6]<=50 else None
                    else:direction.pop(sid,None)
                if kind=='game_over':finish_pending(item.t,'own-death')
                if kind=='rot' and form is not None:
                    sid=event[2];before=event[3];after=event[4];key=('motion',sid)
                    row=dict(reference,before=list(before),after=list(after),tupleUnchanged=before==after,ownActive=alive,
                        directionBefore=direction.get(sid),directionAfter=form['direction'] if form['direction'] is not None else direction.get(sid))
                    row['directionChanged']=row['directionAfter'] is not None and row['directionBefore'] is not None and row['directionAfter']!=row['directionBefore']
                    direction[sid]=row['directionAfter'];frame_order[sid].append(('motion',packet_index))
                    if key in prior:
                        old=prior[key];gap=item.server-old['serverMs'];row['serverGapMs']=gap
                        row['receiptGapMs']=item.t-old['receiptMs'];row['coarseReceiptMaskClean']=clean(old['receiptMs'],item.t)
                        gaps['motion/own' if row['own'] else 'motion/other'].append(gap)
                        if gap<35:witness('motion-under35',row)
                        if gap<40:witness('motion-under40',row)
                        if gap<42:witness('motion-under42',row)
                    if row['tupleUnchanged']:
                        witness('unchanged-decoded-motion-tuple',row)
                        witness('unchanged-tuple/'+('direction-changed' if row['directionChanged'] else 'direction-unchanged' if row['directionBefore'] is not None else 'direction-unknown'),row)
                    for field in form['values']:
                        key_field=(field,sid)
                        if key_field in prior:
                            gap=item.server-prior[key_field]['serverMs'];field_gaps[field+('/own' if row['own'] else '/other')].append(gap)
                            if gap<40:witness('field-'+field+'-under40',row)
                        prior[key_field]=dict(reference)
                    prior[key]=dict(reference);motion_rows.append(row)
                    if sid==world.self_id and pending_heading and 'wanted' in form['values'] and form['values']['wanted']==pending_heading['targetWantedByte'] and pending_heading['firstReflection'] is None:
                        pending_heading['firstReflection']=dict(reference)
                    if sid==world.self_id and pending_boost and 'speed' in form['values'] and pending_boost['firstDirectionalSpeedReflection'] is None:
                        previous_speed=pending_boost['priorSpeed']
                        if previous_speed is not None and ((pending_boost['enabled'] and after[2]>previous_speed) or (not pending_boost['enabled'] and after[2]<previous_speed)):
                            pending_boost['firstDirectionalSpeedReflection']=dict(reference,reportedSpeed=after[2])
                if kind in ('eat','prey_eaten') and event[-1]==world.self_id and alive:
                    row=dict(reference,entity=world.self_id,pickupKind=kind,foodOrPreyId=event[2],positivelyListed=event[3] is not None,
                        nextMassBearing=None,rightCensored=False)
                    pickups.append(row);pending_pickups.append(row);context[world.self_id]['pickup']=item.server
                    if pending_boost and pending_boost['firstDirectionalSpeedReflection'] is None:pending_boost['sizeOrPickupChangeBeforeReflection']=True
                if kind=='anchor':
                    frame_order[event[2]].append(('growth-anchor' if event[5] else 'body-anchor',packet_index))
                    if event[5]:context[event[2]]['growth']=item.server
                if kind=='tail':context[event[2]]['tail']=item.server
                if ((kind=='tail' or (kind=='anchor' and event[5])) and event[2]==world.self_id and
                    pending_boost and pending_boost['firstDirectionalSpeedReflection'] is None):
                    pending_boost['sizeOrPickupChangeBeforeReflection']=True
                mass_sid=event[2] if kind in ('fam','tail','anchor') else None
                mass_bearing=kind=='fam' or (kind=='tail' and len(packet)==6) or (kind=='anchor' and event[5])
                if mass_bearing:
                    mass_reference=dict(reference,entity=mass_sid,own=mass_sid==world.self_id,family=code,
                        phase=controls.get('phase') if controls and mass_sid==world.self_id else None,
                        boostCommandState=boost_state if mass_sid==world.self_id else None,
                        sinceCreditedPickupMs=item.server-context[mass_sid]['pickup'] if 'pickup' in context[mass_sid] else None,
                        sinceGrowthMs=item.server-context[mass_sid]['growth'] if 'growth' in context[mass_sid] else None,
                        sinceTailMs=item.server-context[mass_sid]['tail'] if 'tail' in context[mass_sid] else None,
                        lastHeadingCommandByte=pending_heading['byte'] if pending_heading and mass_sid==world.self_id else None,
                        headingHoldSoFarMs=item.t-pending_heading['sentReceiptMs'] if pending_heading and mass_sid==world.self_id else None,
                        beforeFam=event[3] if kind in ('fam','tail') else None,
                        afterFam=world.snakes[mass_sid].fam if mass_sid in world.snakes else None)
                    mass_rows.append(mass_reference)
                    if code=='h':
                        key=('h',mass_sid)
                        if key in prior:
                            gap=item.server-prior[key]['serverMs'];mass_reference['serverGapMs']=gap
                            gaps['h/own' if mass_reference['own'] else 'h/other'].append(gap)
                            if gap<100:witness('h-under100',mass_reference)
                            if gap<105:witness('h-under105',mass_reference)
                        prior[key]=mass_reference
                    if mass_sid==world.self_id:
                        for pickup in pending_pickups:pickup['nextMassBearing']=dict(mass_reference)
                        pending_pickups=[]
                if kind in ('fam','tail'):frame_order[event[2]].append(('mass-tail',packet_index))
                if kind=='prey':prior[('prey',event[2])]=dict(reference,speed=event[3]['sp'],entry=True)
                if kind=='prey_move':
                    key=('prey',event[2]);row=dict(reference,entity=event[2],speed=event[3]['sp'])
                    if key in prior:
                        old=prior[key];row.update(serverGapMs=item.server-old['serverMs'],receiptGapMs=item.t-old['receiptMs'],
                            intervalKind='entry-to-first' if old.get('entry') else 'repeat',previousSpeed=old['speed'],speed11=old['speed']==11 or row['speed']==11)
                        gaps['prey/'+row['intervalKind']].append(row['serverGapMs'])
                    prey_rows.append(row);prior[key]=dict(reference,speed=row['speed'],entry=False)
            if code=='y' and len(packet) in (3,5):prior.pop(('prey',int.from_bytes(packet[1:3],'big')),None)
        for sid,ordered in frame_order.items():
            for kind,pi in ordered:
                if kind!='motion':continue
                for other,pj in ordered:
                    if other=='motion':continue
                    name=f"{other}/"+('motion-first' if pi<pj else 'motion-last')
                    row=dict(archiveSha256=archive_sha,jsonlLine=line,connection=conn,life=life,epoch=epoch,entity=sid,
                        serverMs=item.server,receiptMs=item.t,own=sid==world.self_id,order=name,packetsHex=[p.hex() for p in item.packets],
                        phase=controls.get('phase') if controls and sid==world.self_id else None)
                    order_rows.append(row);counts['order/'+name]+=1
                    if other=='mass-tail' and pi<pj:witness('reverse-same-frame-mass-motion',row)
        if initialized:frames_by_conn[(conn,epoch)].append((item.t,item.server))
    # Clock bounds use only receipts and ping inequalities; never RTT/2.
    slopes={}
    for connection,frames in frames_by_conn.items():
        bins={}
        for t,s in frames:
            k=int(s//5000)
            if k not in bins or t-s<bins[k][0]-bins[k][1]:bins[k]=(t,s)
        points=list(bins.values())
        if len(points)>=2:
            mt=sum(t for t,s in points)/len(points);ms=sum(s for t,s in points)/len(points)
            den=sum((s-ms)**2 for t,s in points)
            slopes[connection]=sum((s-ms)*(t-mt) for t,s in points)/den if den else None
        else:slopes[connection]=None
    for command in headings:
        command['clockBounds']=None
        reflection=command['firstReflection']
        if reflection is None:continue
        q=command['sentReceiptMs'];c=(command['connection'],command['epoch']);b=slopes.get(c)
        if b is None:continue
        frames=frames_by_conn[c];times=[t for t,s in frames]
        lo=bisect_left(times,q-5000);hi=bisect_right(times,q+5000)
        uppers=[t-b*s for t,s in frames[lo:hi]]
        lowers=[sent-b*s for sent,received,s in pings_by_conn[c] if abs(received-q)<=5000]
        if uppers and lowers and b>0:
            lower=max(lowers);upper=min(uppers)
            if lower<=upper:
                sent_lo=(q-upper)/b;sent_hi=(q-lower)/b
                command['clockBounds']=dict(slope=b,offsetLower=lower,offsetUpper=upper,sendServerLower=sent_lo,sendServerUpper=sent_hi,
                    reflectionDelayLower=reflection['serverMs']-sent_hi,reflectionDelayUpper=reflection['serverMs']-sent_lo,
                    sensitivityIntegerClockMs=1,delayLowerWith1msSensitivity=reflection['serverMs']-sent_hi-1,
                    delayUpperWith1msSensitivity=reflection['serverMs']-sent_lo+1,
                    interpretation='Offline +/-5-second inequality envelope conditional on drift fit and callback timestamps; input application time remains unobserved')
    isolated=sum(c.get('isolatedQualified',False) for c in headings)
    for toggle in boosts:
        toggle['qualifiedObservedTransition']=bool(toggle['firstDirectionalSpeedReflection'] is not None and toggle['ownActiveAtSend'] and
            not toggle['interruptedBeforeReflection'] and not toggle['sizeOrPickupChangeBeforeReflection'])
    confirmed_boosts=sum(b['qualifiedObservedTransition'] for b in boosts)
    paired_pickups=sum(p['nextMassBearing'] is not None for p in pickups)
    coverage=dict(joinedSeconds=joined/1000,isolatedChangedHeadingHolds=isolated,confirmedDirectionalBoostReflections=confirmed_boosts,
        creditedPickupMassPairs=paired_pickups,rightCensoredPickups=sum(p['rightCensored'] for p in pickups),
        directionalSpeedResponsesIncludingConfounds=sum(b['firstDirectionalSpeedReflection'] is not None for b in boosts),
        motionEligible=joined>=300000 and isolated>=20 and confirmed_boosts>=4,
        massEligible=joined>=300000 and paired_pickups>=50,
        batchEligible=joined>=300000 and isolated>=20 and confirmed_boosts>=4 and paired_pickups>=50)
    contextual_counts=defaultdict(Counter)
    for label,selected in [('motion-under35',[r for r in motion_rows if r.get('serverGapMs',math.inf)<35]),
                           ('h-under100',[r for r in mass_rows if r['family']=='h' and r.get('serverGapMs',math.inf)<100])]:
        for row in selected:
            target=contextual_counts[label]
            target['own' if row['own'] else 'other']+=1
            target['phase/'+str(row.get('phase'))]+=1
            target['boost/'+str(row.get('boostCommandState'))]+=1
            target['fieldMask/'+str(row.get('fieldMask'))]+=1
            target['directionChanged/'+str(row.get('directionChanged'))]+=1
            for field in ('sinceCreditedPickupMs','sinceGrowthMs','sinceTailMs'):
                age=row.get(field)
                target[field+'/'+('unknown' if age is None else 'within-report-gap' if age<=row['serverGapMs'] else 'earlier')]+=1
    mask_gaps=defaultdict(list)
    for row in motion_rows:
        if 'serverGapMs' in row:mask_gaps[('own' if row['own'] else 'other')+'/'+row['fieldMask']].append(row['serverGapMs'])
    phase_profiles={}
    for label,rows,period in [('motion42',motion_rows,42),('h105',[r for r in mass_rows if r['family']=='h'],105)]:
        times=[r['serverMs'] for r in rows]
        phase_profiles[label]=[dict(phase=phase,residualMs=summary([phase_residual(t,period,phase) for t in times])) for phase in range(period)]
    e_constraints=Counter()
    for row in motion_rows:
        if row.get('serverGapMs',math.inf)>=40:continue
        bypass=row.get('directionChanged') or any(row.get(k) is not None and row[k]<=row['serverGapMs'] for k in ('sinceCreditedPickupMs','sinceGrowthMs','sinceTailMs'))
        e_constraints['motion/observed-causal-bypass' if bypass else 'motion/no-observed-causal-bypass']+=1
    for row in mass_rows:
        if row['family']!='h' or row.get('serverGapMs',math.inf)>=100:continue
        bypass=any(row.get(k) is not None and row[k]<=row['serverGapMs'] for k in ('sinceCreditedPickupMs','sinceGrowthMs','sinceTailMs'))
        e_constraints['h/observed-causal-bypass' if bypass else 'h/no-observed-causal-bypass']+=1
    candidates={
        'a':dict(motionUnderGate=counts['motion-under42'],hUnderGate=counts['h-under105']),
        'b':dict(motionUnderGate=counts['motion-under40'],hUnderGate=counts['h-under100']),
        'c':dict(perFieldUnder40={f:counts['field-'+f+'-under40'] for f in FIELDS},hUnderGate=counts['h-under100']),
        'd':dict(motionPeriod=42,massPeriod=105,developmentOnlyPhaseFitRequired=True),
        'e':dict(baseUnder40=counts['motion-under40'],baseHUnder100=counts['h-under100'],observableBypassContext=dict(e_constraints),
                 limitation='No observed causal bypass is not proof that authoritative higher-precision/direction state stayed unchanged between reports.'),
    }
    for candidate in candidates.values():candidate.update(packetStateMismatch=None,medianAbsoluteNextReportErrorMs=None,
        predictionWindowsScored=0,unscoredMotionReportsMissingDirtyTime=len(motion_rows),unscoredMassReportsMissingDirtyTime=len(mass_rows),
        exclusion='Authoritative field/direction dirty times and input application times between reports are not observed; no point dirty-time substitution or positive model selection from gap contradictions.')
    result=dict(archiveSha256=archive_sha,coverage=coverage,counts=dict(counts),headingStatuses=dict(Counter(c.get('status','unfinalized') for c in headings)),
        gaps={k:summary(v) for k,v in gaps.items()},fieldGaps={k:summary(v) for k,v in field_gaps.items()},controlStrata=dict(strata),
        motionGapByFieldMask={k:summary(v) for k,v in mask_gaps.items()},exceptionContexts={k:dict(v) for k,v in contextual_counts.items()},
        quantizedPhaseResidualProfiles=phase_profiles,
        coarseMaskWindows=windows,joinedIntervals=joined_intervals,candidates=candidates,earliestWitnesses=dict(witnesses),
        packetRows=dict(motion=motion_rows,mass=mass_rows,ordering=order_rows,prey=prey_rows),
        headings=headings,boosts=boosts,pickups=pickups,
        limits=['One acquisition invocation is the batch; events/lives are correlated.',
            'Tuple equality includes field/direction representation checks; latent precision remains unknown.',
            'Directional speed reflection is an observed response to a boost toggle, not an application timestamp.',
            'Coarse telemetry mask cannot certify fine 5-40 ms receipt timing.',
            'No law fit, clock RTT/2 subtraction, or packet-level population inference.'])
    return result

def main():
    parser=argparse.ArgumentParser();parser.add_argument('capture',type=Path);parser.add_argument('--source',type=Path,required=True);parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args();sys.path.insert(0,str(args.source/'tools/fidelity'))
    import slither_protocol as sp
    result=analyze(args.capture,sp)
    result['analysisSha256']=hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    with args.output.open('x',encoding='utf-8') as out:json.dump(result,out,indent=2,allow_nan=False)
    print(json.dumps(dict(coverage=result['coverage'],counts=result['counts'],headingStatuses=result['headingStatuses']),indent=2))

if __name__=='__main__':main()
