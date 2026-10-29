# Attempts and limits

Full logs, source copies and binaries are retained local-only. No public machine,
provider, account, personal-path or gateway identifiers accompany these notes.

1. The first client-harness compile failed on a malformed range expression.
   Corrected before executing it; no timing result came from that build.
2. The first eight-client smoke checked task membership before worker creation
   finished. The controller now permits a bounded startup interval. No capacity
   window was accepted from that attempt.
3. The next eight-client smoke completed, including CPU-clock versus process
   accounting reconciliation.
4. The first 500-socket pilot lost a client worker during forced warmup rejoins.
   It is excluded. No host capacity number is taken from it.
5. A second 500-socket pilot used readiness polling and awaited the close
   handshake. It still encountered admission refusals: `Gate::free_slot` waits
   for the simulation to consume pending disconnect commands, beyond releasing
   the socket slot. This failed before acceptance and is retained.
6. The accepted driver delays reconnection asynchronously by 28 ms after a
   completed close, and retries only unadmitted full-server responses at the
   42 ms client cadence. Established-session errors still fail the run. It keeps
   other sockets draining during that wait. Its 500-socket pilot passed without
   admission refusals or send backpressure; it is a short preflight, not mature
   capacity evidence.
7. Synthetic negative controls doubled recorded CPU work, saturated client CPU,
   and made send queues grow. All were rejected, and the original preflight
   files remained unchanged. These are metrology controls, not observed stalls.

8. The first long mature hold qualified at tick 193,000, but after settling its
   first 300-second window moved from mean 59.064 to 86.478. Continued holds were
   stopped with normal host/client cleanup. This is retained as a drift control,
   excluded from the requested mean-about-55 comparison. The plan amendment uses
   independent re-warmups and short fixed windows with class checks throughout.

9. The reporting turn was interrupted by a T3 restart at 21:30 UTC, rather than
   a request to stop collection. On resumption, the detached coordinator, second
   base cohort, host, three client workers and core sampler were still running.
   The active cohort was retained, subject to the unchanged validity gates;
   neither binaries nor collection windows were restarted or changed.

10. That retained second base attempt completed, but failed the unchanged
    quiet-process screen: 0.320 CPU seconds from another user process in the
    29.4049-second complete-block window, above its 0.294049-second threshold.
    No capacity number is accepted from it. The coordinator stopped before
    starting the next cohort; the waiting diagnostic controller also stopped.
    After inspecting the failure, allow one fresh repeat of this cohort for
    interference, keeping the same quiet threshold and all original gates.
    This amendment does not use its timing outcome. Previously accepted cohorts
    are kept. Previous completed host/client CPU plus conservative termination
    and controller allowances remain charged against the six-CPU-hour budget.

11. A source-identity audit found two retained helper refinements relative to
    the executed copies: the common-client builder permits an absent earlier
    cleanup receipt on a fresh reproduction, and the core sampler docstring
    describes a separate accounting view. Neither changes the compiled client,
    host overlays, sampled counters or collection behavior. Other helper source
    bytes match after LF normalization. Original executed copies remain retained.

12. All six primary capacity cohorts completed and passed. The base diagnostic
    qualified late, at tick 202,500, and then grew outside the primary length
    class. Keep its actual per-window distributions explicit. Its first
    diagnostic reduction stopped because the final fine-window census edge
    lacked a later `/proc` sample. Restrict diagnostic validation and analysis
    to complete blocks bracketed by telemetry, and add a post-window snapshot
    to the next diagnostic runner. No primary selection rule or timing gate
    changes; retain the original failure and charge its CPU before continuing.

13. The replacement base diagnostic qualified at tick 193,000 and stayed in
    the body class, but its PMU collection failed client cadence. Worker reports
    in its first second have input-lateness p99 108.688 and 97.075 ms, above the
    unchanged 42-ms gate, with CPU headroom and no send backpressure. Its host
    timing and PMU counts are excluded. The later coarse/fine windows have
    bounded cadence, but are unnecessary for the current-main tail question.
    No timing-selected rerun or threshold relaxation is made. The retained
    client-valid first-base PMU window is explicitly a drifted diagnostic,
    not a matched primary-body comparison.
14. Diagnostic CPU totals 7,006.74 seconds (1.95 hours), including both base
    attempts and builds, below the 7,200-second ceiling. No further warmup is
    started. All six capacity cohorts and the current-main diagnostic windows
    pass their gates. A strictly matched-body, client-valid base PMU repeat
    remains unqualified; this limit does not change the accepted capacity data.

No production optimization or outside-server experiment is performed.
