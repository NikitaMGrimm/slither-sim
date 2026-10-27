"""Linux x86-64 per-thread user-instruction counter, no wall/CPU estimate."""
import ctypes,os,struct
class Counter:
    def __init__(self,tid):
        library=ctypes.CDLL(None,use_errno=True);library.syscall.restype=ctypes.c_long
        attr=(ctypes.c_uint64*16)();attr[0]=128<<32;attr[1]=1;attr[4]=3;attr[5]=(1<<5)|(1<<6)
        self.fd=library.syscall(298,ctypes.byref(attr),ctypes.c_int(tid),ctypes.c_int(-1),ctypes.c_int(-1),ctypes.c_ulong(0))
        if self.fd<0:raise OSError(ctypes.get_errno(),os.strerror(ctypes.get_errno()))
    def read(self):return struct.unpack('QQQ',os.read(self.fd,24))
    def close(self):os.close(self.fd)
