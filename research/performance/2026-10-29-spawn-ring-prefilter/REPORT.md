# Reject blocked spawn heads before constructing rings

The candidate preserves all 31 selected complete byte traces and catches both
focused and protocol-13/19 wire mutations. The required instruction pairs and
slow-tick measurements have **not run**: the SSH bench remains reserved by
another worker. This report makes no speed, latency or capacity claim yet.

Baseline `23eddca8`; implementation `bd8915c8`. The selected `edab3287` archive
adds only harness qualification, with the same production source. This branch
is independent of the pending minimap cache and marker reuse candidates.
[Plan](PLAN.md), [attempts](ATTEMPTS.md), [reproduction](REPRODUCE.md),
[selected correctness](correctness.json).

The high-boost [Battledome study](../2026-10-29-battledome-boost/REPORT.md) found
admission dominating the slowest ticks at 160 and 320 seats. Every rejected
pose still built a complete circular body before testing its head. After the
first original body initializes the strict query, this candidate skips that
construction only when the head already fails, ring geometry is valid and
directional fallback is ineligible. All three random draws still occur in
their original order. The first 256 fallback-eligible poses keep the full
original path, including exact scores/ties. Invalid geometry keeps the original
construction/error path. No candidate budget, admission outcome, refusal,
retry delay, floor, clearance, population or delivery rule changes.

## Correctness and stress qualification

The frozen pre-change full-search oracle compares bitwise admitted poses and
headings, errors, subsequent RNG and unchanged world state for three seeds,
open/blocked scenes, floor/no-floor and request ages below/at/above two seconds.
The focused fallback-gate mutation fails that oracle; restored source passes.
The ring-head unit control also rejects hiding invalid/nonfinite ring geometry.

All 23 deterministic catalog rows, four historical host fixtures and two
supplementary Battledome protocol traces compare complete bytes with terminal
records. Two more traces run the actual host queue against held synthetic
barriers, then remove the barriers halfway through to check successful
admissions and bytes. Their checkpoint/RNG, queue state, frames and delivery
times agree completely between baseline and candidate.

| Protocol | Blocked-phase pending seats throughout | Refused searches | Candidate poses tested | Pending after release | Mutation pending after release |
| --- | ---: | ---: | ---: | ---: | ---: |
| 19 | 72 | 90 | 368640 | 54 | 57 |
| 13 | 72 | 90 | 368640 | 54 | 57 |

These values come from `fixed[*].base.summary` in the selected JSON; the
candidate's entire corresponding receipt is identical. The uncompressed bytes
are also compared directly. Each refused search tests exactly 4,096 poses.
Both wire mutations change encoded frames, not merely a summary or hash field.
The wrong fallback gate admits 15 seats after release, versus the original 18.

The stress row guarantees the overload work pattern with the real host's
waiting list, request-age/fallback clock and refusal retry methods. Its seven
held replay barriers and one observer are explicitly synthetic; dynamic radius
growth is disabled for that row. This is not an 80-live-player, naturally grown
or capacity workload. The counted primary version holds all 72 joins waiting
throughout, while the uncounted correctness version includes the release phase.

Additional no-backlog 80-viewer and shrinking-pit controls are prepared to
measure the minimap and marker paths independently. Their held bodies are also
synthetic. No local timing or PMU value is used as performance evidence.
Full raw sources, copied binaries, traces and logs are local-only, with selected
hashes identifying them but no public download.

## Pending measurement

Local focused tests, the full 15-command Rust group and seven documentation
tests pass. The final audited fixed-work examples compile separately for all
nine study/arm stages, with distinct executables and all six protocol wire
mutations detected. The 80-viewer minimap row also compares complete bytes in
both protocols with stock decorations and nonzero routing generations. The marker control has five
actual marker-change ticks in each protocol; both no-backlog controls retain
80 live viewers. Synthetic reducer checks validate group arithmetic and exclude
the discarded warmup. See [verification](verification.json). Then, only
after the maintaining agent releases the SSH guest, run native full-byte gates,
repeated-fork/direct-parent proofs and five alternating instructions:u pairs
for both protocols, with search/refusal and slow-tick groups separate. The
staged source manifests and runner are in the reproduction instructions.
Keep every pair, failure and process inventory; report ranges and limitations
before opening this change's PR. The minimap candidate is measured first,
then this candidate; marker reuse follows. No guest use or added cloud cost
has occurred during this preparation.
