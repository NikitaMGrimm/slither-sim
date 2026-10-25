# Deviations and failed attempts

- Before acquisition: `python -B run.py` exited with `OSError: cannot pin
  coordinator/children to CPUs 4-5`. No socket or attempt was opened. Python's
  default ctypes return type truncated the Windows process handle. `bootstrap.py`
  declares the handle and affinity ABI, then runs the frozen coordinator without
  changing its bytes. The affinity requirement and acquisition plan are unchanged.
- Before acquisition: the first offline server-list decoder import lacked a
  `file:///` URL and failed with `ERR_UNSUPPORTED_ESM_URL_SCHEME`. Corrected import
  decoded the retained HTTP response. No game connection was made.
