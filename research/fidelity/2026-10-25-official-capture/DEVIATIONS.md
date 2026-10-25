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
- The diagnostic opened and received 1,130 frames with a verified protocol-19
  setup and receipt. The original coordinator was stopped after `low-1` drained,
  before reserving another batch. Its five remaining no-Origin batches were
  superseded by the separately committed continuation plan; none was silently
  counted as completed. One replacement low-RTT batch restores the three-batch
  target. No other replacement or endpoint substitution occurred.
- The original coordinator launch and a strict-CPU-gate process were deliberately
  stopped between attempts, with their lock files retained under distinct names.
  These process exits are not interrupted captures. All acquisition streams
  drained to terminal receipts.
- A literal Windows-glob audit invocation and a malformed empty ad-hoc summary
  failed offline. The glob audit was rerun with Python path expansion. These
  command failures changed no capture source or raw bytes.
- The final held-out timing results were inspected only after all four
  development analyses completed. Both held-out streams passed integrity;
  high CPU and near-empty collector-clean exposure block confirmatory adoption.
