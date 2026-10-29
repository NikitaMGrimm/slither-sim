# Physical-core generator failure and Zen 5 pool amendment

The c7a.2xlarge attempt completed and retrieved base-160, but input-lateness
p99 reached 46.604952 ms against the unchanged 42 ms gate. Worker CPU p99
was 23.34%; queues and socket errors passed. The strict selector also rejected
it. Cleanup verified zero nonterminated instances, open/active Spot requests,
volumes or Elastic IPs across all 17 enabled regions at 11:56:01 UTC, with
the task key/group deleted. Eight physical cores alone did not qualify the
generator; no CPU-topology causal claim or relaxed deadline follows.

Return to the original c8a Zen 5 family, using c8a.4xlarge Spot to try a different
capacity pool from the twice-reclaimed c8a.2xlarge pool. Keep **six workers on
CPUs 0–5 and control on CPU 7**; the extra CPUs are unused by clients. This
changes generator capacity, not clients, steering, decode work or boost policy.
Require all source/native-binary pins to match and retain every existing gate.
Run all six primary rows together on one fresh same-AZ c7a.medium/c8a.4xlarge
pair; no earlier row supplies a primary. Preserve all failed/qualified earlier
windows separately. No play hold, bench guest, local Rust build or production
changes. Stop on actual CLI expiry without manual renewal.

All previous paid compute is bounded by $0.75 at bid caps (calculated bound
$0.716). Final bid caps are $0.06/h target and $0.50/h generator. Both guests
retain 140-minute boot poweroff timers (110 planned +30), terminate behavior
and DeleteOnTermination. Allow two extra minutes for boot when bounding launch
to poweroff: final compute ceiling is $1.325334 for 142 minutes. A checked
$0.90 noncompute allowance covers all attempts' storage, IPv4, control/artifact
traffic and misc, replacing the earlier unspent $1.40 reserve. Overall ceiling
**$2.975334**, rounded **$2.98**, below $3. Verify timer deadlines occur within
142 minutes of launch before work. Existing failed attempts cannot grow after
their verified zero-resource cleanup. Keep the outcome if this attempt fails;
do not silently exceed the task cap.
