# Attempts

- The maintaining agent requested a protocol-19 input-to-explicit-echo study and
  added investigation of #281's accumulated frame excess and short stalls.
  Source inspection found zero default player latency, midpoint steering decode,
  immediate responsive wanted-angle reports, early socket delivery, and a Linux
  delivery-channel wake path rather than an unconditional 1-ms output delay.
  Use the unchanged host CLI stats and generator-side packet capture; no runtime
  hook is proposed or added. Freeze all rows/quotas/definitions before timing.
- The first remote probe build failed because sha2 0.11's digest array does not
  implement LowerHex. Replaced formatting with explicit hexadecimal digits and
  removed an unused import. No counting had started; retain the failed build log.
- The first non-primary smoke connection raced the host's asynchronous bind and
  was refused. The host log proved it began listening just afterwards. Added an
  external controller readiness check for the existing "Listening" CLI line;
  preserve the failed smoke directory. No server source or counted window changed.
- The second smoke exposed a stale-file race in that readiness check: a previous
  smoke's Listening line was visible before the new log reader truncated it.
  Preserve prior logs under distinct names and create a fresh log before launch.
  Both refused-connection attempts remain non-primary and are retained.
- The third smoke reached gameplay and exposed a probe clock-order error: the
  game-ping check reused a loop timestamp older than the just-sent probe. Refresh
  that timestamp before checking/sending the game ping. ClientState correctly
  rejected the backward clock; retain the log and capture. No primary ran.
- The fourth smoke passed: 32 sends/echoes, control/game pongs and all 799
  recorded binary frames matched the capture with no overlapping payloads.
  The collection SSH dispatch subsequently timed out after the remote process
  had started; read-only inspection confirmed the solo window was counting.
  Do not redispatch or duplicate the collector. This is a controller timeout,
  not a failed or replaced measurement window.
- Retrieved a closed/member-verified solo checkpoint before final teardown;
  packing ran on generator CPU 9 during early 80-seat counting, followed by a
  small verified download. Retain that controller activity in interpreting maxima.
- An affinity audit found the external Python controller unbound, although the
  load coordinator, workers, probe and captures were pinned as planned. Applied
  CPU 7 to its two threads about 132 seconds into the 80-seat primary and added
  that startup pin to the reusable driver. Preserve the dispatched driver and
  timed correction receipt. Solo and the first part of 80 retain this disclosed
  deviation; no interval is discarded, shifted or replaced. The probe's own
  CPU-6 pin and measured kernel-to-read delay remain available throughout.
- The generator Spot instance was reclaimed after the four windows ended and
  during final reduction, before the final raw archive could be retrieved.
  The first three independently verified checkpoints remain safe; the original
  320 raw data is unavailable and is not a verified result. The first reduction
  had also stopped on a capture/frame-count mismatch, so its packet diagnostics
  remain unverified. Replay the retained captures without assuming complete tails.
  One replacement Spot generator fits the original combined cap (conservative
  full-timer bound below $2.50). Retain the original solo/80/160 primaries and
  repeat the loaded staged prehistory solely to replace the lost 320 artifact;
  repeated lower-load windows are calibration, never primary replacements.
- Before replacement timing, checkpoint completeness review found the original
  generator-wide qualification and ICMP logs were not included. Supersede that
  recovery selection prospectively: retain the three checkpoints as supplementary
  and unqualified, and repeat all four primaries on a fresh Spot pair. Adding a
  fresh target keeps the combined full-timer bound at $2.352 and restores the
  complete export safety margin; no timer is extended. Retrieve the old target's
  closed logs before retiring it. No original latency values informed selection.
- The second pair completed and secured all four row archives, then the load
  process failed its final exit check. Its log shows worker 3 read an empty
  target-count file during the 160-to-320 transition and exited before the 320
  primary. That row is unqualified, regardless of its latency values. Replace
  controller truncation writes with atomic temporary-file replacement, including
  RESET; keep native binaries unchanged. Repeat only 320 after the same staged
  loaded prehistory; repeated lower rows are calibration, not replacements for
  the retained lower primaries. Complete cleanup even when a worker exit check
  fails. Record the original failed row and all control-source versions.
