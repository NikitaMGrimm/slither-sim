# Same-class generator recovery within the existing timer budget

The larger Zen 5 generator completed and locally verified base-160,
candidate-160 and candidate-320, then Spot reclaimed it during base-320 warmup.
The target remained running and the CLI session valid. Its terminated state was
observed before 13:00 UTC, bounding that generator to 50 minutes from launch.
Recover, close/gzip/hash and locally verify all surviving warmup host sources
before releasing their process/control port. The deleted generator's unfinished
warmup receipts remain unavailable. Keep every completed primary unchanged.

Retain the exact c7a.medium target and replace only the generator in the same AZ,
requesting the same c8a.4xlarge class and unchanged six workers, source/binaries,
decoder, A4 steering and boost treatment. Restart only the unfinished base-320
fresh ladder, then base-480 and candidate-480. Primary order stays unchanged;
no partial window is pooled and no completed row is rerun selectively. Disclose
the generator instance change, including its implications for the single-window
client timing comparison. Every row still must pass the original gates.

Extend the target plan to 120 minutes total: install and verify its poweroff
deadline at 150 minutes from original launch before cancelling its original
timer. Bound target lifetime conservatively at 152 minutes. The replacement generator gets a verified
90-minute boot timer (60 planned +30), at most 92 minutes from launch. At the
unchanged generator cap, **50 + 92 = 142 minutes**, so the existing full-timer
generator budget is unchanged. The extra target allowance adds $0.01: combined
full-timer ceiling is $2.985334, rounded $2.99, below $3. Previous paid resources remain cleaned up.
Use the same task key/group, retain all old request/resource receipts, and
cancel the old generator request if active. No play hold, bench guest, manual
credential renewal, relaxed qualification or production change. Stop on actual
CLI expiry. Reassess rather than silently exceed the cap if recovery fails.
