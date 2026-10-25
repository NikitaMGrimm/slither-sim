# Deviations and failed attempts

- Before acquisition: `python -B run.py` exited with `OSError: cannot pin
  coordinator/children to CPUs 4-5`. No socket or attempt was opened. Python's
  default ctypes return type truncated the Windows process handle. `bootstrap.py`
  declares the handle and affinity ABI, then runs the frozen coordinator without
  changing its bytes. The affinity requirement and acquisition plan are unchanged.
- Before acquisition: the first offline server-list decoder import lacked a
  `file:///` URL and failed with `ERR_UNSUPPORTED_ESM_URL_SCHEME`. Corrected import
  decoded the retained HTTP response. No game connection was made.
- After both `low-1` clients exhausted three quick retries without WebSocket
  open: separately predeclare one ten-second, one-client handshake diagnostic
  with `Origin: http://slither.io` in `handshake-plan.json`. The original plan
  remains unchanged. This tests a routine header failure using the same official
  endpoint and retains HTTP/connect diagnostics, with no timing-model inference.
