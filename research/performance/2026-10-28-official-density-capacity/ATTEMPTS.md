# Retained attempts

1. Preparation attempt 1 compiled the pristine CLI and portable client, then
   failed the census build because `RawSimulator.backend` was private. No run
   or measurement. The read-only archived-copy visibility overlay now includes
   that field. Compiler output and copied pristine binary remain local-only.
2. Preparation attempt 2 reused only the bounded dependency target and rebuilt
   the root package from the pinned archive. The profile-overlay sleep anchor
   matched both simulation and bot loops; it was narrowed to its complete line.
   No measurement used a partially instrumented binary.
3. The eight-bot smoke run completed with working user PMU counts, loopback
   reconnect-capable A4 client and verified affinity. Its forced short warmup
   is not a length-screened capacity result.
4. During the primary warmup, `/proc/TID/stat` returned the process-wide
   user/system CPU totals for every thread. Direct reads of
   `/proc/PID/task/TID/stat` returned distinct thread totals. The collector now
   uses the explicit task path. The old collector was terminated and the fixed
   collector attached to the same running host/client PIDs; neither game process
   was stopped or restarted. No hold had begun; affected early thread CPU rows
   are excluded from attribution, while their world census remains retained.
   Reproduction: compare fields 14/15 from those two proc paths for each TID
   of the same multithreaded host. This is an observed path discrepancy on this
   guest, not an identified kernel cause.
5. An ad hoc telemetry expression had a mismatched closing bracket and failed
   with `SyntaxError` before reading counters. It changed no workload or data;
   subsequent inspection used a saved script instead of nested shell quoting.
6. The primary warmup initially used main `4e68c8047e2ff61c985f449506ff7bca5273c3ff`,
   while #245 was open. It reached more than 11,000 completed ticks, with
   growing lengths and roughly 15–18 ticks/s. When #245 merged, the collector
   and both game processes were stopped before any hold. This exploratory
   trajectory and all raw rows remain retained; it supplies no post-#245
   capacity or paired speed claim. Primary preparation was restarted at main
   `d127acd1f0e9db2100c8a1dd5c2fa43e2deff506`, including #244, #245 and #247.
7. The agent turn was interrupted shortly after the post-#245 run started.
   On resumption the guest had exactly the intended study coordinator,
   collector, one 2,280-bot host and one loopback client. The primary collector
   was still sampling, with no hold started; no stale preparatory, pre-#245,
   smoke or compiler process remained. No stray process needed stopping.
   The bounded target directory was absent. Full process command lines and
   the UTC check time are retained in the local-only resume receipt. The
   intact primary warmup continued without restarting either game process.
8. During primary warmup, new SSH sessions began returning
   `Permission denied (publickey)` on repeated attempts. The already established
   study/coordinator connection remained alive and continued returning block
   census output. A read-only local SSH configuration/certificate inspection
   found a certificate with a past validity endpoint; this does not establish
   why earlier new connections had succeeded. The maintaining agent was asked to
   restore SSH access. That diagnosis was premature: the maintaining agent verified fresh connections
   with three SSH clients and identified the first alias block in the explicit
   local configuration as authoritative. Supplying that configuration explicitly
   restored command access without changing any credential or setting.
   The established collection connections had meanwhile closed. The host
   collector failed with a broken output pipe at the 25,000-tick block and
   stopped its host/client through its cleanup; its data are retained as an
   interrupted warmup, with no timed hold. The remaining task-owned instruction
   sampler was stopped before restarting. No prohibited management CLI,
   credential change or system setting change was attempted. The study does
   not claim access was revoked or that an expired certificate caused the
   failure. No configured SSH login name is included in public files.
9. The same pinned binaries restarted in fresh output directories after
   preserving the interrupted 25,000-tick attempt. Coordinator and instruction
   sampler output now goes to task-owned files, with stdin detached; SSH
   transport loss can no longer break their progress-output pipes. Runtime,
   seed, affinity, warmup screen and holds are unchanged. No compilation.
10. Before any hold, the maintaining agent, acting on the owner's behalf,
    extended warmup to three hours per case and
    authorized multi-core warming. An idempotent `taskset -a -p -c` operation
    plus explicit readback passed on all five existing host threads. Only the
    collector/sampler were replaced; the same host/client processes and live
    world continued. The new collector permits CPUs 0, 2 and 3 for warmup,
    records the pin and full command-line process list, then re-pins every
    host thread to CPU 0 and discards 60 seconds before holds. CPU 1 remains
    for the external client/collector. The original length rule and minimum
    tick count remain unchanged. No hold data had been collected or inspected
    before the amendment; the unchanged runtime is built for multithreaded
    hosting and no simulation or transport clock is rewritten by affinity.
11. Optional user-instruction IP sampling was piloted only during excluded
    warmup. The first two pilots rejected non-user overflow locations; the
    retained header identifies CPU mode KERNEL despite the user-only event
    configuration. The final sampler keeps these locations unattributed and
    makes no kernel-instruction claim. A third pilot's symbolizer incorrectly
    equated executable file offsets with virtual addresses; readelf showed
    different PT_LOAD offsets in the PIE. Those symbols are rejected. An
    independent synthetic-ELF regression now verifies the translation, including
    a deliberately different text virtual offset. The fourth pilot also exposed
    misleading nearest exported names in stripped shared libraries. The final
    sampler symbolizes only the measured executable and retains library weights
    by module, without assigning them to a caller or function. All failed and
    superseded pilot artifacts remain local-only; none enters a capacity hold
    or the final profile. The validated final pilot precedes any primary hold.

All preparation, raw host/client logs, process lists, generated source and
binary identities are retained outside Git. Public selection removes private
machine names, account/home paths, remote addresses and kernel build suffixes.
