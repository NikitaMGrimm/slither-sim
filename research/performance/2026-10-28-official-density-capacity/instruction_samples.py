"""Linux x86-64 user-instruction IP samples; diagnostic windows only.

instruction_samples.py pilot PID NEW_OUTPUT
instruction_samples.py watch RUN_DIRECTORY [RUN_DIRECTORY ...]
Reuses the study's perf_event_open setup. No runtime patch, kernel access or
system setting change. Inline locations are not complete dynamic call stacks.
"""
from collections import Counter
import ctypes
import json
import mmap
import os
from pathlib import Path
import struct
import subprocess
import sys
import time

def elf_relative(binary,mapping_offset,within_mapping):
    with Path(binary).open('rb') as stream:
        header=stream.read(64)
        assert header[:6]==b'\x7fELF\x02\x01', 'ELF64 little-endian required'
        phoff=struct.unpack_from('Q',header,32)[0]
        size,count=struct.unpack_from('HH',header,54)
        for index in range(count):
            stream.seek(phoff+index*size)
            kind,flags,offset,virtual,physical,filesize,memorysize,alignment=struct.unpack('IIQQQQQQ',stream.read(56))
            lo=offset//mmap.PAGESIZE*mmap.PAGESIZE
            hi=(offset+filesize+mmap.PAGESIZE-1)//mmap.PAGESIZE*mmap.PAGESIZE
            if kind==1 and flags&1 and lo<=mapping_offset<hi:
                return within_mapping+mapping_offset+virtual-offset
    raise ValueError('No executable PT_LOAD for map offset')

def sample(pid,out,seconds):
    out.mkdir(exist_ok=False)
    libc=ctypes.CDLL(None,use_errno=True);page=mmap.PAGESIZE
    maps=[]
    raw=Path(f'/proc/{pid}/maps').read_text();(out/'maps.txt').write_text(raw)
    for line in raw.splitlines():
        columns=line.split(maxsplit=5)
        start,end=[int(x,16) for x in columns[0].split('-')]
        maps.append((start,end,int(columns[2],16),columns[5] if len(columns)>5 else '[anonymous]',columns[1]))
    events=[];lost=0;counts=Counter();unattributed=Counter();modes=Counter();started=time.time()
    def read(fd):return struct.unpack('QQQ',os.read(fd,24))
    try:
        for task in Path(f'/proc/{pid}/task').iterdir():
            attr=(ctypes.c_uint64*16)()
            attr[0]=128<<32;attr[1]=1;attr[2]=1000000
            attr[3]=1|2|4|256  # IP, TID, TIME, PERIOD
            attr[4]=3;attr[5]=(1<<5)|(1<<6)
            fd=libc.syscall(298,ctypes.byref(attr),int(task.name),-1,-1,0)
            if fd<0:raise OSError(ctypes.get_errno(),'perf_event_open sampling')
            ring=mmap.mmap(fd,page*17,flags=mmap.MAP_SHARED,prot=mmap.PROT_READ|mmap.PROT_WRITE)
            events.append({'fd':fd,'ring':ring,'tail':0,'role':(task/'comm').read_text().strip(),'before':read(fd)})
        end=time.monotonic()+seconds
        while time.monotonic()<end:
            for event in events:
                ring=event['ring'];head=struct.unpack_from('Q',ring,1024)[0]
                offset,size=struct.unpack_from('QQ',ring,1040)
                assert offset==page and size==page*16 and head-event['tail']<=size, 'ring overrun'
                def take(at,n):
                    at%=size
                    data=ring[offset+at:offset+min(size,at+n)]
                    return data+ring[offset:offset+n-len(data)] if len(data)<n else data
                while event['tail']<head:
                    kind,misc,length=struct.unpack('IHH',take(event['tail'],8))
                    assert 8<=length<=size
                    data=take(event['tail'],length)
                    if kind==9:
                        ip,process,thread,clock,period=struct.unpack_from('QIIQQ',data,8)
                        mode=misc&7;modes[event['role'],str(mode)]+=period
                        if mode==2:counts[event['role'],ip]+=period
                        else:unattributed[event['role'],f'[no user IP: sampled CPU mode {mode}]']+=period
                    elif kind==2:lost+=struct.unpack_from('Q',data,16)[0]
                    event['tail']+=length
                struct.pack_into('Q',ring,1032,event['tail'])
            time.sleep(.01)
        coverage=[]
        for event in events:
            delta=[b-a for a,b in zip(event['before'],read(event['fd']))]
            assert delta[1]>0 and delta[2]/delta[1]>=.99
            coverage.append({'role':event['role'],'instructions':delta[0],'enabledNanos':delta[1],'runningNanos':delta[2]})
        assert lost==0, 'lost samples'
    finally:
        for event in events:event['ring'].close();os.close(event['fd'])
    capture_end=time.time()
    grouped={};other=unattributed.copy()
    for (role,ip),weight in counts.items():
        region=next((r for r in maps if r[0]<=ip<r[1]),None)
        if region and 'x' in region[4] and region[3].endswith('/slither-sim'):
            relative=elf_relative(region[3],region[2],ip-region[0])
            grouped.setdefault(region[3],[]).append((role,relative,weight))
        else:other[role,Path(region[3]).name if region else '[unmapped]']+=weight
    functions=Counter();inclusive=Counter()
    for binary,rows in grouped.items():
        addresses=sorted({address for _,address,_ in rows})
        command=['addr2line','-a','-f','-C','-i','-e',binary]
        result=subprocess.run(command,input=''.join(f'{address:x}\n' for address in addresses),capture_output=True,text=True,check=True,timeout=120)
        (out/f'addr2line-{Path(binary).name}.txt').write_text(result.stdout)
        stacks={};current=None;lines=result.stdout.splitlines();i=0
        while i<len(lines):
            if lines[i].startswith('0x'):
                current=int(lines[i],16);stacks[current]=[];i+=1
            else:
                assert current is not None and i+1<len(lines)
                stacks[current].append(lines[i]);i+=2
        for role,address,weight in rows:
            stack=stacks[address] or ['[unresolved]']
            functions[role,stack[0]]+=weight
            for function in set(stack):inclusive[role,function]+=weight
    def listing(counter):
        return [{'role':role,'function':function,'instructionWeight':weight}
                for (role,function),weight in counter.most_common()]
    result={'startUnix':started,'endUnix':capture_end,'symbolizationEndUnix':time.time(),'sampleSecondsRequested':seconds,
            'periodInstructions':1000000,'lostSamples':lost,'coverage':coverage,
            'sampledCpuModes':listing(modes),
            'self':listing(functions),'inlineInclusive':listing(inclusive),'otherModules':listing(other),
            'limits':'User retired-instruction event; only USER locations in the measured executable are symbolized, with PT_LOAD virtual offsets. Shared libraries remain module-only because stripped symbols can give misleading nearest-symbol names. Non-user locations remain unattributed, not kernel instruction counts. Inline frames overlap; no dynamic stack. Sample weights are estimates, not exact phase counters. Symbolization follows capture; maps/raw locations remain local-only.'}
    (out/'selected.json').write_text(json.dumps(result,indent=2)+'\n')
    print('INSTRUCTION_SAMPLES_COMPLETE',out.name,flush=True)

if __name__=='__main__':
    if not __debug__:raise SystemExit('Run without Python optimization')
    os.sched_setaffinity(0,{1})
    if sys.argv[1]=='pilot':sample(int(sys.argv[2]),Path(sys.argv[3]),5)
    else:
        assert sys.argv[1]=='watch'
        for arg in sys.argv[2:]:
            root=Path(arg);deadline=time.monotonic()+14400
            while not (root/'PROFILE').exists():
                if time.monotonic()>deadline:raise TimeoutError(root.name)
                time.sleep(.5)
            for attempt in range(100):
                try:meta=json.loads((root/'summary.json').read_text());break
                except json.JSONDecodeError:time.sleep(.01)
            else:raise RuntimeError('Invalid profile metadata')
            sample(meta['serverPid'],root/'ip-samples',15)
