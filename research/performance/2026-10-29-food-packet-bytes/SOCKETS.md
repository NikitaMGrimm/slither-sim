# Socket writes: counts and exact-reduction limits

Analysis only. No socket production code changes here. The earlier
[500-client diagnostic](../2026-10-29-food-diffs/REPORT.md#remaining-measured-work)
assigns 2.760 CPU ms per maintenance interval to the **whole socket task**;
that is not an isolated measurement of empty flushes or kernel writes. Its
mean body length 65.192–66.776 fails the mature-class gate, so it remains an
explanatory diagnostic. Loopback sends can charge receive-side processing to
the sender; none of these costs establishes a real-NIC budget.

## Earlier 500-client counts

Values are calls/messages per recipient per simulation-boundary interval,
dividing totals by 500. Socket work can cross those boundaries.

| Intervals | Count | Binary WebSocket messages | TCP `Write` calls | `Open::flush` calls |
| --- | ---: | ---: | ---: | ---: |
| Maintenance | 25 | 0.973440 | 0.881280 | 6.544240 |
| Other | 1,775 | 0.717643 | 0.718921 | 3.689075 |

The totals are 12,168/11,016/81,803 on maintenance intervals and
636,908/638,042/3,274,054 otherwise. The complete diagnostic has 649,076
WebSocket messages and 649,058 TCP writes: approximately one write per message.
The different conditional ratios alone cannot identify coalescing because work
crosses boundaries. These were instrumented Rust `Write` calls, not a historical
kernel syscall trace. The source chain is `Open::send` → tungstenite buffered
framing → `ReadBudget<TcpStream>::write` → `TcpStream::write`.

## Direct kernel witness

A separate bounded own-child `ptrace` diagnostic runs eight real stock-protocol
loopback clients with the retained received-frame A4 client. It counts kernel
entries, brackets every TCP `Write` call, and marks simulation boundaries and
the same message/flush call sites. No attach, system/security setting or package
change is used. Markers and tracing distort scheduling: this is a **short-world
count witness**, with no CPU, tail or capacity result.

[Selected counts and build identities](socket-results.json),
[counter](syscall_counts.c), [build overlay](build_socket_counts.py),
[bounded runner](run_socket_counts.py), [inventory wrapper](inventory.py).

| Intervals | Count | WebSocket messages/recipient | Kernel `sendto`/recipient | `Open::flush`/recipient |
| --- | ---: | ---: | ---: | ---: |
| Maintenance | 9 | 1.000000 | 1.000000 | 5.888889 |
| Other | 691 | 0.700434 | 0.700434 | 5.921852 |

These are means per recipient **per interval**, not totals per recipient.
Across ticks 301–1,000 there are 3,944 TCP `Write` calls and exactly 3,944 kernel
`sendto` entries. Every completed bracket contains one `sendto`; transport
`write`, `writev`, `sendmsg` and sends outside the brackets are zero. The nine
maintenance intervals contain 72 messages, 72 `sendto`s and 424 flush calls;
the other 691 contain 3,872, 3,872 and 32,736. All eight clients remain playing,
with no refusal, client failure or host panic.

Thus `sendto` is the observed kernel write operation on this build. Applying the
one-to-one mapping to the earlier 500-client Rust-call counts is an **inference**;
it does not turn that historical run into a direct syscall trace. The two
workloads and their flush frequencies must not be treated as matched samples.

## What could be reduced exactly

The current host already uses a 16 KiB tungstenite write buffer. `send_due`
queues due messages in order; every socket service pass ends with one
`Open::flush` call per open connection. Threshold-triggered writes, partial
writes, automatic control replies and `WouldBlock` retries can also write.
Many explicit flush calls therefore reach an empty buffer and issue no syscall.

An exact TCP batching change would combine **already-due, complete WebSocket
frames for the same recipient at the existing release/flush opportunity**,
retaining every WebSocket header, payload, message boundary, order and elapsed
header. The code already performs basic buffering. These counts do not establish
that an additional batching opportunity is large: first identify recipients
with multiple actual writes within one such opportunity. One due message for
one recipient still needs transport progress; batching across later service
opportunities would change release timing.

Stock frame grouping and protocol-13 individual-command messages must stay
unchanged. Merging legacy commands into one WebSocket message would violate
the [retained protocol-13 framing](../../fidelity/2026-10-26-protocol13-catalog/REPORT.md).
TCP buffering can carry several unchanged WebSocket frames in one write; changing
the WebSocket message itself is a different behavior.

Skipping proven-empty `Open::flush` calls is a separate possible saving in
function/service work, not a demonstrated syscall saving. An exact pending-output
hint would have to cover queued frames, automatic pong/close replies from reads,
threshold writes and partial/`WouldBlock` retries, and clear only when drained.
No such implementation or CPU-saving claim is made here.

## Reproduction and limits

Runtime is main after #260, `23e987f3`, independent of both round-2 production
changes. Reconstruct the earlier [checkpoint client](../2026-10-28-capacity-checkpoint/REPRODUCE.md)
and the [redacted inventory helper](../2026-10-28-broadcast-distance/reproduce.py).
Bind the new scripts' historical temporary paths to that external harness
layout, put the runtime archive at `base.tar`, and name the inventory wrapper
`inventory.py`. Run `python3 build_socket_counts.py`, then
`python3 run_socket_counts.py`. The latter owns only its child host/counter and
eight clients, uses a new loopback listener, and has a 45-second collection cap.
Both use the round's sole Cargo target; finish them before timing anything else.
The builder's `tarfile` extraction filter needs Python 3.11.4 or later.

The x86-64 Linux counter uses `PTRACE_TRACEME`, follows only its own child's
threads, counts syscall entries and validates complete write-call brackets.
Research-only zero-byte invalid-descriptor `write` markers carry event/tick
arguments without emitting bytes. Their overhead is not subtracted or timed.
Full raw CSV, client/host logs, command inventories, sources and copied binaries
remain local-only. Selected values and script/source/binary hashes identify the
retained witness; they do not establish production scheduling or capacity.
