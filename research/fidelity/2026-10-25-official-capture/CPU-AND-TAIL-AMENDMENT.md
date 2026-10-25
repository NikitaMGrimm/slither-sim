# Parent steering before continuation acquisition

The parent reported two more offline compilation/simulation workers. Before
each continuation batch, `cpu_gate.py` samples total CPU for about ten seconds
and waits for a mean below 30%, as well as absence of `host_load`/`slither-sim`.
It records the same samples during acquisition; a mean at or above 30% marks
that batch contaminated. Sampling uses the language-independent WMI total-CPU
counter. The original frozen files remain unchanged.

Before inspecting held-out batches, add these offline diagnostics:

- Explicit downlink tail episodes above 10 and 20 ms: incidence, peak, observed
  duration and adjacent-receipt duration bracket. Compare the frozen full model,
  no pauses, half pause rate and half pause duration on the same schedules/seeds.
- Also calculate the parent's ±600-frame-floor metric after dropping the first
  50 frames of each setup epoch, alongside the original detrended ±10-second
  metric. They use different gauges and must not be mixed in a comparison.
- Report RTT-minus-downlink-excess and conditional RTT sensitivity to pauses in
  both independent directions versus downlink only. This cannot establish a
  physical direction split or common physical cause. It is an ablation, not a fit.

No new success threshold or model selection is introduced. The original gates
remain primary. Any numerical alternative stays exploratory and cannot change
defaults from these data. Parent-supplied loopback p99 43.6 ms and retained
official p99 8.4 ms motivate the check; they are not new official measurements.
