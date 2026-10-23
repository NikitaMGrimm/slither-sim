# Bounding a matched performance window

Per-child timeouts do not bound a comparison containing many children. An
October 23 helper experiment was interrupted after 190.6 seconds against an
agreed 120-second window when observed costs rose. Its partial rows are retained
as incomplete, and no helper performance conclusion is drawn. An earlier complete
81.2-second comparison is separate; it showed gains and an observation regression,
so the runtime candidate remains unpromoted.

`tools/perf_compare.py --max-wall-seconds N` gives metadata commands and all
measurement children one shared monotonic deadline. For example, after agreeing
a two-minute window and pinning copied release executables:

```sh
python -B tools/perf_compare.py --baseline BASELINE_EXE --candidate CANDIDATE_EXE --output NEW_OUTPUT_DIR --max-wall-seconds 105
```

Replace the three path placeholders. Leave time inside the agreed interval for
startup, file operations and cleanup. The existing cleanup kills the owned native
child, waits with a ten-second timeout and closes its pipes/error file. This is
not an OS CPU/memory cap or a guarantee against an unresponsive operating system.
The option cannot extend the existing 30-second metadata or 120-second native
child limits. No runtime threads, services or platform changes are introduced.

Expiry prevents the next child from starting or interrupts the current child.
Flushed rows and diagnostics remain available; `failure.json` records the budget
and elapsed wall time. Incomplete attempts produce no comparison summary and
must not supply selected medians or a speed claim. Rehash the retained binaries
before reusing them in a separately agreed window. All existing workload counts,
warmups, pair ordering and behavior without this optional flag are retained.

Deadline regressions use mocked children and clocks, including actual handle
closure checks; they do not run native benchmarks. Separate release-workload
measurements are not necessary to validate timeout/cleanup plumbing.
