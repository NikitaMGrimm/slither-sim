# Minimal comment changes

Keep Comparison, Performance, Regression and fidelity, Details, and all existing table columns. Keep numeric Base/PR formatting. The performance unit line becomes:

> Operation wall time, ms/op. Negative change means less time.

Result strings:

```
🟢 Faster
🔴 Slower
⚪ Within ±5%
🟡 Undecided
⚠️ Unavailable
```

Keep the Change column, with an estimate and asymmetric interval, all in percent:

```
−8.30% [−10.10%, −6.40%]
+1.20% [−1.10%, +3.50%]
+1.90% [−3.80%, +7.90%]
```

These three formatting-only examples correspond respectively to Faster, Within ±5%, and Undecided. An Unavailable row uses em dashes; a degenerate Undecided row shows its point estimate plus `[interval unavailable]`. Two decimal places for changes/endpoints; decisions always use the unrounded log bounds. Do not label the bracket a per-row 95% interval: its individual coverage is 99.8333%, chosen for at least 95% simultaneous coverage across all rows and looks under the model.

Replace the old three performance-method bullets in Details with these **two exact bullets**:

```markdown
- Paired operation wall time after warmup, with balanced order and checks at 8/12/26 pairs. Base/PR are geometric means; Change shows estimate [interval] against a −5% to +5% band.
- Order-adjusted t intervals provide ≥95% simultaneous coverage across 10 workloads and 3 looks under independent normal log errors. Resolved rows stop; time-capped rows retain their last completed look. Labels use unrounded bounds.
```

Keep all non-performance Details about fixture scope, behavior traces, source changes, provenance, raw data and errors. Replace the old per-row raw min/max change range line with counts/stopping information, for example:

```markdown
- `observe`: 8 pairs used; stopped when resolved.
- `step-food`: 12 pairs used; time cap (14 complete pairs collected).
- `protocol`: 8 pairs used; interval unavailable (degenerate residuals); time cap.
- `step-body`: unavailable; candidate failed at pair 5 (see raw logs).
```

Before hosted calibration is completed, add one temporary Details sentence:

```markdown
- Hosted-runner calibration is pending; performance labels are advisory and conditional on the timing model.
```

Remove that pending statement only when calibration has actually been run and its results reviewed; keep the model qualification and advisory status. No table redesign, badges, plots or extra columns are required.
