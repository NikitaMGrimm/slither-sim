# Evidence-led follow-ups

## Socket nice +10 (frozen before collection)

Nice +5 lowered excess p99 from 72.930 to 19.795 ms and tick busy p99 from
9.543 to 6.331 ms, with zero deferred ticks, but still missed the delivery gate.
The profiled baseline showed large simulation runnable waits and involuntary
switches; representative queue-to-send-pop p99 was only 0.157 ms. Following the
original eligibility rule, test socket nice +10 once, unchanged 120 s hold and
grade. No other thread priority changes. Keep the +5 miss.

## Socket SCHED_BATCH / nice 0 (frozen before collection)

Nice +10 still missed (20.421 ms excess p99), although tick-completion lateness
p99 improved to 4.039 ms from baseline 71.301 ms; nice +5 gave 5.318 ms. The
permanent weight reduction trades simulation backlog for delivery delay. Test
Linux SCHED_BATCH for the socket thread at nice 0: retain equal CPU weight while
avoiding its ordinary wakeup-preemption behavior. One unchanged 120 s hold;
record effective policy/nice in /proc. This is the second and last additional
candidate after +5; keep both nice misses. No send buffering or protocol change.
