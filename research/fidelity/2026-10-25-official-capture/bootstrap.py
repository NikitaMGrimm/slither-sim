"""Windows ABI correction for the unchanged frozen coordinator."""
import ctypes
from pathlib import Path
import runpy

ctypes.windll.kernel32.GetCurrentProcess.restype = ctypes.c_void_p
ctypes.windll.kernel32.SetProcessAffinityMask.argtypes = (ctypes.c_void_p, ctypes.c_size_t)
runpy.run_path(str(Path(__file__).with_name('run.py')), run_name='__main__')
