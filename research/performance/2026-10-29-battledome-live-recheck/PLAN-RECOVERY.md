# Second generator interruption amendment

The repeated pair completed base-160, candidate-160 and candidate-320. Spot
then reclaimed its c8a generator during the base-320 ladder, before the 320
primary. The c7a.medium target remained running and the CLI session remained
valid. Recover and verify all surviving fourth-row host evidence before
releasing that process/control port; do not reconstruct missing client receipts.

Retain this same target server for all six primary rows. Replace only the
generator, using an eight-vCPU c7a.2xlarge Spot in the same AZ to avoid the
repeatedly interrupted c8a pool. Keep portable flags, source, shared client
binary, six fully decoding workers and all qualification gates unchanged.
Require the newly built host/client hashes to match the previous builds.

Repeat **both 320 arms**, candidate then base, on the replacement generator;
then run base-480 and candidate-480. The already completed candidate-320 is
supplementary, not pooled or substituted. Primary order is still base-160,
candidate-160, candidate-320, base-320, base-480, candidate-480. Every within-seat
pair uses one generator class and the same physical target core. Disclose the
generator change between 160 and 320; cross-seat client timing is not a controlled
generator-hardware comparison. All other fresh-ladder history stays unchanged.

Reassessed full-timer budget: $0.27 for the earlier interrupted pair; at most
60 minutes of the now-terminated generator at its $0.30/h cap ($0.30); target
server at most 160 minutes at $0.06/h ($0.16); replacement generator at most
120 minutes at $0.30/h ($0.60); $1.40 noncompute reserve across all attempts.
Combined ceiling **$2.73**, below $3. No new key/group is needed. Preserve the
already captured old generator request ID and cancel it if still active.

Plan 130 minutes total for the retained target and 90 for the replacement
generator. Before resuming, install and verify a target poweroff deadline at
160 minutes from its original launch, maintaining the original timer until
the replacement timer is verified. Verify the replacement generator's
120-minute boot poweroff timer, terminate behavior and DeleteOnTermination.
No owner hold or bench guest. Stop on actual CLI expiry. Keep every interrupted
row/cleanup failure and report all paid resources, including the extra generator.
