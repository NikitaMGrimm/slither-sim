# Prequalification harness recovery

No 500-client hold ran in the first instance pair. Native traces matched and
49 unit /63 loopback checks passed after supplying #201's required tick-output
path to the library tests. The first socket smoke then failed reduction because
the new `load-*.inputs.json` trace was included in the legacy `load-*.json`
shard census. The smoke/raw data and failed logs remain retained.

Before replacement collection, rename sampled traces to `inputs-*.inputs.json`
in the worker after the clients exit. This changes output packaging only; reuse
the identical already-built server/load binaries and tested runtime sources.
Validate reduction against the retained smoke first. No performance-based repeat
or exclusion. One replacement pair is allowed, and all attempts count toward $3.

Replacement hard timers are 90 minutes (planned <=60 minutes plus 30), verified
terminate-on-shutdown with delete-on-terminate roots. Worst remaining compute at
launch caps is (.65+.06+.08+.15)*1.5 = $1.41, plus first-attempt cost and the
$1.10 reserve. Stop before $2.80 conservative projection. Do not recover again.
