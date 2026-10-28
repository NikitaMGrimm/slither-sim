"""Record public-safe execution facts and probe PMU permissions; change no setting."""
import ctypes
import json
import os
from pathlib import Path
import platform
import subprocess
import sys

os.sched_setaffinity(0,{1})
libc=ctypes.CDLL(None,use_errno=True)
def pmu(exclude_kernel):
    attr=(ctypes.c_uint64*16)()
    attr[0]=128<<32;attr[1]=1;attr[4]=3
    attr[5]=(1<<6)|((1<<5) if exclude_kernel else 0)
    ctypes.set_errno(0)
    fd=libc.syscall(298,ctypes.byref(attr),0,-1,-1,0)
    if fd<0:return {'available':False,'errno':ctypes.get_errno()}
    os.close(fd)
    return {'available':True}

result={'description':'a 4-vCPU Zen 5 EPYC KVM guest','architecture':platform.machine(),
        'logicalCpus':os.cpu_count(),'kernelVersion':platform.release().split('-')[0],
        'python':platform.python_version(),'libc':platform.libc_ver(),
        'rustc':subprocess.check_output(['rustc','-Vv'],text=True),
        'perfEventParanoid':int(Path('/proc/sys/kernel/perf_event_paranoid').read_text()),
        'userInstructions':pmu(True),'userAndKernelInstructions':pmu(False),
        'memoryKiB':int(Path('/proc/meminfo').read_text().splitlines()[0].split()[1]),
        'timingScope':'descriptive virtual-machine elapsed time; user instruction counts separate'}
Path(sys.argv[1]).write_text(json.dumps(result,indent=2)+'\n')
