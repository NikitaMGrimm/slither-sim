"""Count all-thread user instructions in separate, bounded profile windows.

thread_instructions.py RUN_DIRECTORY [RUN_DIRECTORY ...]
Waits for each collector's PROFILE marker, then counts 45 s while that run's
60 s phase profile is active. Uses no kernel access or system setting change.
"""
import ctypes
import json
import os
from pathlib import Path
import struct
import sys
import time

def main():
    if not __debug__:raise SystemExit('Run without Python optimization')
    os.sched_setaffinity(0,{1})
    libc=ctypes.CDLL(None,use_errno=True)
    for arg in sys.argv[1:]:
        root=Path(arg);deadline=time.monotonic()+14400
        while not (root/'PROFILE').exists():
            if time.monotonic()>deadline:raise TimeoutError(root.name)
            time.sleep(.5)
        for attempt in range(100):
            try:meta=json.loads((root/'summary.json').read_text());break
            except json.JSONDecodeError:time.sleep(.01)
        else:raise RuntimeError('Invalid profile metadata')
        pid=meta['serverPid']
        counters=[]
        for task in Path(f'/proc/{pid}/task').iterdir():
            attr=(ctypes.c_uint64*16)()
            attr[0]=128<<32;attr[1]=1;attr[4]=3;attr[5]=(1<<5)|(1<<6)
            fd=libc.syscall(298,ctypes.byref(attr),int(task.name),-1,-1,0)
            if fd<0:raise OSError(ctypes.get_errno(),'perf_event_open')
            counters.append((fd,(task/'comm').read_text().strip()))
        def read():return [struct.unpack('QQQ',os.read(fd,24)) for fd,_ in counters]
        first=read();start=time.time()
        time.sleep(45)
        last=read();end=time.time();rows=[]
        for (fd,name),a,b in zip(counters,first,last):
            os.close(fd);delta=[y-x for x,y in zip(a,b)]
            assert delta[1]>0 and delta[2]/delta[1]>=.99
            rows.append({'role':name,'userInstructions':delta[0],'enabledNanos':delta[1],
                         'runningNanos':delta[2],'scheduledFraction':delta[2]/delta[1]})
        (root/'thread-instructions.json').write_text(json.dumps({'startUnix':start,'endUnix':end,'threads':rows},indent=2)+'\n')
        print('THREAD_COUNTS_COMPLETE',root.name,flush=True)

if __name__=='__main__':main()
