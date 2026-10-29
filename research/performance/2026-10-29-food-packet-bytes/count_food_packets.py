"""Count exact food bytes in the retained mature base proof; no new workload."""
from pathlib import Path
from collections import Counter
import hashlib,json,math,struct,tarfile
r=Path(__file__).resolve().parent;out=r/'food-packet-census';out.mkdir(exist_ok=False)
need={'packet-bytes/sessions/stock/proof-base-0.trace','packet-bytes/sessions/stock/proof-base-0.json'}
with tarfile.open(r/'packet-v1-complete.tar.gz','r|gz') as tar:
    for member in tar:
        if member.name in need:
            (out/Path(member.name).name).write_bytes(tar.extractfile(member).read())
            need.remove(member.name)
            if not need:break
assert not need
sample=json.loads((out/'proof-base-0.json').read_text())
flags={x[0]:bool(x[1]) for x in sample['ticks']};assert len(flags)==360
counts={phase:Counter() for phase in ['all','maintenance','ordinary']}
sizes={phase:Counter() for phase in counts};frames=Counter();summaries=[];pending=None
def split(frame):
    payload=memoryview(frame)[2:];assert len(payload)>0
    if payload[0]>=32:yield payload;return
    at=0
    while at<len(payload):
        first=payload[at]
        if first<32:n=first*256+payload[at+1];at+=2
        else:n=first-32;at+=1
        assert 0<n<=len(payload)-at
        yield payload[at:at+n];at+=n
with (out/'proof-base-0.trace').open('rb') as f:
    while head:=f.read(16):
        assert len(head)==16;tag,n=struct.unpack('<QQ',head);data=f.read(n);assert len(data)==n
        if tag==2:pending=struct.unpack('<QQQ',data)
        elif tag==3:
            assert pending is not None
            tick,slot,_=pending;tick+=1;assert tick in flags and slot<500
            phase='maintenance' if flags[tick] else 'ordinary'
            for p in ['all',phase]:frames[p]+=1
            for packet in split(data):
                code=chr(packet[0])
                if code in 'Ffbc<w':
                    for p in ['all',phase]:counts[p][code]+=1;sizes[p][code]+=len(packet)
            pending=None
        elif tag==4:summaries.append(json.loads(data))
assert tag==4 and len(summaries)==2 and pending is None
assert frames['all']==summaries[-1]['frames']-summaries[0]['frames']
result={'scope':'Exact encoded food-packet census in the retained mature stock base proof; bytes/counts only, not allocation CPU attribution. RawSimulator::take returns all generated frames including future deliveries. Header tick is the pre-advance tick; samples label its successor.','traceSha256':hashlib.file_digest((out/'proof-base-0.trace').open('rb'),'sha256').hexdigest(),'recipients':500,'phases':{}}
for phase in counts:
    n=len(flags) if phase=='all' else sum(v==(phase=='maintenance') for v in flags.values())
    incremental=sum(counts[phase][c] for c in 'fbc<');b=sum(sizes[phase][c] for c in 'fbc<')
    result['phases'][phase]={'ticks':n,'frames':frames[phase],'packetsByCode':dict(counts[phase]),'bytesByCode':dict(sizes[phase]),'incrementalPacketsPerRecipientTick':incremental/500/n,'incrementalBytesPerRecipientTick':b/500/n}
(r/'food-packet-census.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps(result,indent=2))
