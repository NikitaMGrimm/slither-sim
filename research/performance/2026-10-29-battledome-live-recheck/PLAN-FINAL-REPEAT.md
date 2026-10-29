# Generator qualification failure and final full repeat

The c7i replacement's candidate-320 archive was retrieved and verified, but
input-lateness p99 reached 59.197346 ms, exceeding the unchanged 42 ms gate.
Worker CPU p99 remained below 34%, and socket errors/queue gates passed.
Retain the complete invalid row; it is not qualified live-tail evidence. The
collector stopped before base-320 and cleaned up. All enabled regions then
verified zero nonterminated instances, open/active Spot requests, volumes and
Elastic IPs, with task key/group removed. The CLI session stayed valid.

Repeat all six rows on one fresh Frankfurt same-AZ c7a.medium/c7a.2xlarge Spot
pair. Require the generator's default eight vCPUs to be eight physical cores
with one thread each, matching the original generator's core topology; c7i's
four physical cores/eight threads did not qualify. Preserve the same portable
native binaries, six workers, decoder, A4 steering, boost override, gates,
order, fresh ladders and marked windows. No relaxed gate, thinner client or
selective row pooling. All earlier complete/invalid/interrupted rows remain
separate supplementary or failed-attempt evidence. The source/pin identities
must match before the final sequence starts.

Previous paid compute is bounded by $0.65 at bid caps using completed cleanup
and the observed old-generator termination bound. The final pair's full
140-minute timer ceiling remains $0.84; the $1.40 noncompute reserve covers
all attempts together. Combined ceiling **$2.89**, below $3. Recheck the
existing session and prior cleanup before launch. Both new guests retain
140-minute boot poweroff timers (110 planned +30), terminate-on-shutdown and
DeleteOnTermination. No play hold, bench guest, manual renewal or production
changes. If this final attempt fails, retain its outcome and reassess without
silently exceeding the cap.
