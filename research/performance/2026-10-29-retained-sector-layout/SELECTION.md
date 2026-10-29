# Candidate selection after fresh attribution

Choose #269 candidate 2: sorted contiguous retained-sector keys with parallel
exact `f64` timestamps. Remove both tree-backed timestamp storage and duplicate
membership keys; expose ascending membership to the existing food consumers.
Keep the oldest witness and its update/skip logic, strict subtraction-based
expiry, stencil geometry, cleanup epochs and revision identity semantics.

The fresh runtime is merged main `9ad002c1`. Three cycle profiles per protocol
contain 718 stock and 803 protocol-13 maintenance samples. Retained-sector
advancement accounts for **11.70% / 8.72%**, and **10.31% / 9.54%** of qualified
maintenance dTLB samples. The `last_seen.retain` source site has 36 cycle samples
in each protocol, with tree iteration/extraction among the named leaves.
Subscription-union work is another **5.99% / 5.85%** of cycle samples, including
iteration over retained membership. These are containing scopes, not a promise
that every sample can be removed by the layout.

The food source group is larger overall (**34.82% / 33.25%** of maintenance
cycles; **28.87% / 28.80%** of dTLB samples), but its leading work is held-listing
refresh, hint lookup, Arc bookkeeping and subscription union. Listing build/sort
is only **1.53% / 1.37%**, and sector diff plus listing lookup **2.79% / 2.49%**
of maintenance cycle samples. Those containing scopes include non-layout work.
They do not establish a clearly larger removable budget for food record packing
than the retained-sector representation. Held-listing refresh/ownership remains
a strong follow-up candidate, independently of record packing.

On ordinary ticks, snake visibility/history/bounds still leads attributed
project sources at **21.08% / 19.41%** of cycle samples; food is **10.57% / 9.96%**.
This round targets the aligned maintenance burst rather than rewriting those
separate snake paths. Broad groups and sampled IPs do not identify exact load
addresses or imply corresponding speedups.

All cycle/dTLB samples are lossless and unthrottled, with fully scheduled
counters. L1d counters are valid, but L1d IP sampling throttled in both protocols
even at the predeclared 10,007 period; retain it as diagnostic-only and do not
repeat/tune it. Counter-only results remain separate from profiler CPU.

The start footprint has 30,199 retained sectors/timestamps across 500 recipients.
The old timestamp-map key/value payload is at least 301,990 bytes and duplicate
membership keys another 60,398 bytes, before tree nodes/allocator overhead.
Two sorted vectors remove that duplicate key payload and node traversal; shifts
and vector spare capacity remain costs to measure. No node-byte saving or CPU
win is assumed in advance.

Reuse the literal full-sweep BTree reference owner in the existing directed and
48,000-step random-walk oracle, independently of the new vector storage. Compare
ordered keys, every timestamp bit, touch/center/epoch, minimum-witness validity
and revision-change behavior. Keep the three semantic mutations (outgoing time,
re-entry witness refresh, added deadline) and source restoration. Freeze this
design before the primary source/build pins and five-pair results; no later
layout tuning or gate change is permitted.
