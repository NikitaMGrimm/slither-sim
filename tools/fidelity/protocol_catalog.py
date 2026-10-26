"""Independent observational wire-13/19 decoder. No networking or JavaScript.

Integers are big-endian; positions are wire coordinates, not simulator state.
Unknown codes/lengths fail visibly. Text contents are never returned.
"""
import math

def uint(p, at, n):
    if at < 0 or at + n > len(p):
        raise ValueError('truncated field')
    return int.from_bytes(p[at:at+n], 'big')

def split(frame):
    if len(frame) < 3:
        raise ValueError('short elapsed frame')
    if frame[2] >= 32:
        return [(2, frame[2:])]
    result, at = [], 2
    while at < len(frame):
        if frame[at] < 32:
            n = uint(frame, at, 2); at += 2
        else:
            n = frame[at] - 32; at += 1
        if n == 0 or at+n > len(frame):
            raise ValueError('invalid bundle')
        result.append((at, frame[at:at+n])); at += n
    return result

def decode(p, version, own=None):
    if version not in (13, 19) or not p:
        raise ValueError('unsupported version or empty packet')
    code, n = chr(p[0]), len(p)
    out = {'code': code, 'bytes': n}
    u = lambda at, width=2: uint(p, at, width)
    if code == 'a':
        if n < 25 or p[23] != version:
            raise ValueError('setup/version mismatch')
        out.update(grid=u(1,3), maxSegments=u(4), sectorSize=u(6),
                   sectorCount=u(8), protocol=p[23], spacing=p[24])
        if n >= 30: out['radius'] = u(27,3)
        out['extensionBytes'] = max(0,n-30)
    elif code == 's':
        out['id'] = u(1)
        if n <= 7:
            if n not in (3,4): raise ValueError('unknown removal form')
            out['removed'] = True
            if n == 4: out['died'] = bool(p[3])
        else:
            at = 23
            name = p[22]; uint(p,at,name); at += name
            custom = u(at,1); at += 1
            uint(p,at,custom); at += custom
            cosmetic = u(at,1); at += 1
            body = []
            if at < n:
                body.append([u(at,3)/5,u(at+3,3)/5]); at += 6
                while at < n:
                    if at+2 > n: raise ValueError('odd appearance body')
                    if version == 19 and at+2 == n:
                        angle = u(at)/65536*math.tau
                        body.append([body[-1][0]+42*math.cos(angle), body[-1][1]+42*math.sin(angle)])
                    else:
                        body.append([body[-1][0]+(p[at]-127)/2,body[-1][1]+(p[at+1]-127)/2])
                    at += 2
            out.update(heading=u(3,3)/16777215*math.tau, direction=p[6],
                       wanted=u(7,3)/16777215*math.tau, speed=u(10)/1000,
                       fam=u(12,3), skin=p[15], head=[u(16,3)/5,u(19,3)/5],
                       nameBytes=name, customBytes=custom, cosmetic=cosmetic,body=body)
    elif code in 'gGnN=+':
        grow = code in 'nN+'
        self_form = version == 19 and (code in 'GN' or (code=='=' and n==7) or (code=='+' and n==10))
        at = 1 if self_form else 3
        out.update(id=own if self_form else u(1), grow=grow, own=self_form)
        remaining = n-at-(3 if grow else 0)
        if version == 13:
            if code in 'gn' and remaining == 4:
                out['position']=[u(at),u(at+2)]
            elif code in 'GN' and remaining == 2:
                out['delta']=[p[at]-128,p[at+1]-128]
            else: raise ValueError('unknown wire13 anchor')
        elif code in '=+' and remaining == 6:
            out.update(angle=u(at), position=[u(at+2),u(at+4)])
        elif remaining in (0,2):
            if remaining: out['angle']=u(at)
        else: raise ValueError('unknown wire19 anchor')
        if grow: out['fam']=u(n-3,3)
    elif code in 'eE345d7':
        self_form = version == 19 and (code in 'd7' or n <= 3)
        at = 1 if self_form else 3; count=n-at
        out.update(id=own if self_form else u(1),own=self_form)
        if count == 3:
            fields=('headingByte','wantedByte','speedByte')
        elif count == 2:
            fields= {'e':('headingByte','speedByte'),'E':('wantedByte','speedByte'),
                     '4':('wantedByte','speedByte'),'3':('headingByte','wantedByte'),
                     '5':('headingByte','wantedByte')}.get(code)
        elif count == 1:
            fields={'e':('headingByte',),'E':('wantedByte',),'4':('wantedByte',),'3':('speedByte',)}.get(code)
        else: fields=None
        if not fields: raise ValueError('unknown motion form')
        out.update(zip(fields,p[at:]))
    elif code in 'hr':
        if n not in ((6,) if code=='h' else (3,6)): raise ValueError('mass/tail length')
        out['id']=u(1)
        if n==6: out['fam']=u(3,3)
    elif code == 'R' and n == 2: out['credit']=p[1]
    elif code == 'F':
        foods=[]
        if version == 13:
            if (n-1)%6: raise ValueError('wire13 food listing length')
            for at in range(1,n,6): foods.append([p[at],u(at+1),u(at+3),p[at+5]])
        else:
            if n<3 or (n-3)%4: raise ValueError('wire19 sector length')
            out['sector']=[p[1],p[2]]
            for at in range(3,n,4): foods.append([p[at],p[at+1],p[at+2],p[at+3]])
        out['foods']=foods
    elif code in 'fb':
        if version == 13:
            if n!=7: raise ValueError('wire13 food birth length')
            out.update(color=p[1],position=[u(2),u(4)],size=p[6])
        else:
            if n not in (4,5,6,7): raise ValueError('wire19 food birth length')
            at=1
            if n>=6: out['sector']=[p[1],p[2]];at=3
            out['offset']=[p[at],p[at+1]];out['size']=p[-1]
            if n in (5,7): out['color']=p[-2]
    elif code in 'cC<':
        if version == 13:
            if code!='c' or n not in (5,7): raise ValueError('wire13 pickup length')
            out['position']=[u(1),u(3)]
            if n==7: out['eater']=u(5)
        else:
            has_eater=code=='<'; content=n-1-2*has_eater
            if content not in (2,4): raise ValueError('wire19 pickup length')
            at=1
            if content==4: out['sector']=[p[1],p[2]];at=3
            out['offset']=[p[at],p[at+1]]
            if has_eater: out['eater']=u(n-2)
    elif code in 'wW' and n==3: out['sector']=[p[1],p[2]]
    elif code == 'l':
        if n<6: raise ValueError('short leaderboard')
        out.update(ownRank=p[1],rank=u(2),population=u(4));rows=[];at=6
        while at<n:
            length=u(at+6,1);uint(p,at+7,length)
            rows.append({'segments':u(at),'fam':u(at+2,3),'color':p[at+5],'nameBytes':length})
            at+=7+length
        out['rows']=rows
    elif code == 'm':
        length=u(7,1);uint(p,8,length)
        out.update(segments=u(1,3),fam=u(4,3),nameBytes=length,textBytes=n-8-length)
    elif code == 'S' and n==7: out.update(id=u(1),session=u(3,4))
    elif code == 'k' and n==6: out.update(id=u(1),kills=u(3,3))
    elif code == 'z' and n==4: out['radius']=u(1,3)
    elif code == 'v' and n==2: out['status']=p[1]
    elif code == 'p' and n==1: pass
    elif code in 'MVUuL':
        if code=='M':
            out.update(side=u(1),encoding='reverse-seven-bit-runs',dataBytes=n-3)
        elif code=='V': out.update(encoding='xor-reverse-seven-bit-runs',dataBytes=n-1)
        else: raise ValueError('unresolved alternate minimap')
        at=3 if code=='M' else 1;cells=ones=runs=0
        while at<n:
            value=p[at];at+=1
            if value==255:
                if at==n:raise ValueError('truncated minimap escape')
                cells+=126*p[at];at+=1;runs+=1
            elif value>=128:cells+=value-128;runs+=1
            else:cells+=7;ones+=value.bit_count()
        out.update(encodedCells=cells,setBits=ones,zeroRuns=runs)
    elif code == 'y':
        out['id']=u(1)
        if n in (3,5):
            out['removed']=True
            if n==5: out['eater']=u(3)
        elif n==20:
            out.update(color=p[3],position=[u(4,3)/5,u(7,3)/5],size=p[10],
                       direction=p[11],angleA=u(12,3),angleB=u(15,3),speed=u(18))
        else: raise ValueError('prey appearance length')
    elif code == 'j':
        if n not in (7,9,10,11,12,13,14,16): raise ValueError('prey motion length')
        out.update(id=u(1),position=[1+3*u(3),1+3*u(5)])
        if n in (11,13,14,16):out['direction']=p[7]
        if n in (10,12):out['heading']=u(7,3)
        if n in (11,13):out['wanted']=u(8,3)
        if n in (14,16):out.update(heading=u(8,3),wanted=u(11,3))
        if n in (9,12,13,16):out['speed']=u(n-2)
    elif code == '6': out['encodedBytes']=n-1
    else: raise ValueError('unresolved code/length '+code+'/'+str(n))
    return out

def scrub(p):
    """Return same-length derived bytes and explicit text/identity scrub ranges."""
    q=bytearray(p); ranges=[]
    def clear(at,n,kind,fill=0):
        uint(q,at,n)
        if n: q[at:at+n]=bytes([fill])*n;ranges.append([at,at+n,kind])
    code=chr(p[0])
    if code=='a':
        if len(p)>=27: clear(25,2,'server-number')
        if len(p)>30: clear(30,len(p)-30,'opaque-setup-extension')
    elif code=='s' and len(p)>7: clear(23,p[22],'name',ord('x'))
    elif code=='l':
        at=6
        while at<len(p):
            n=uint(p,at+6,1);clear(at+7,n,'name',ord('x'));at+=7+n
    elif code=='m':
        n=uint(p,7,1);clear(8,n,'name',ord('x'));clear(8+n,len(p)-8-n,'record-text',ord('x'))
    elif code=='S': clear(1,len(p)-1,'session-identity')
    return bytes(q),ranges

def compact(decoded):
    """Small numeric assertions for selected frames; never return display text."""
    result=dict(decoded)
    # Preserve wire-unit assertions; scaled angles/speeds are documented
    # interpretations rather than an independent floating-point oracle.
    if result['code']=='s':
        for field in ('heading','wanted','speed','direction','cosmetic'):
            result.pop(field,None)
    if result['code']=='S':
        result.pop('id');result.pop('session')
    for field in ('body','foods','rows'):
        if field in result:
            values=result.pop(field);result[field+'Count']=len(values)
            if values:result[field+'First']=values[0]
    return result

