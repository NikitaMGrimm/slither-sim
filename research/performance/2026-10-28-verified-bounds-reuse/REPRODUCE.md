# Reproduce verified bounds reuse

Use the existing [full-host construction and fork method](../2026-10-28-mature-covered-bounds/REPRODUCE.md)
and [circle-study workflow](../2026-10-28-circle-body-coverage/REPRODUCE.md).
All workloads are synthetic and offline. Heavy builds/profiling/counts run on the
authorized guest using existing tools and locked cached dependencies. No packages,
limits, security settings or outside game servers are involved.

Runtime baseline is merged #253 `634b54d835e56fd054d220f7aab13389f93564ff`;
measured cache runtime is `b12f70b9c13ff0984648579415deb9131c656b59`.
The later guide-only merge changes no Rust/build input. Create base.tar and
candidate.tar with git archive at those pins, in a fresh external output directory.
Use one bounded target directory and preserve all failed runs and copied binaries.

Reuse these scripts by copying them into that *external output directory* for
execution; they are linked here rather than duplicated in the study:

- [build.py](../2026-10-28-circle-body-coverage/build.py)
- [check.py](../2026-10-28-circle-body-coverage/check.py), with [check.patch](check.patch)
- [fixture.rs](../2026-10-28-mature-covered-bounds/fixture.rs)
- [serve.py](../2026-10-28-mature-covered-bounds/serve.py), [measure.py](../2026-10-28-mature-covered-bounds/measure.py)
- [queue249.py](../2026-10-28-mature-covered-bounds/queue249.py), [accept249.py](../2026-10-28-mature-covered-bounds/accept249.py)
- [compare.py](../2026-10-28-mature-covered-bounds/compare.py), [verify-parity.py](../2026-10-28-mature-covered-bounds/verify-parity.py)
- [short-pairs.py](../2026-10-28-mature-covered-bounds/short-pairs.py)
- [shared/reproduce.py](../2026-10-28-broadcast-distance/reproduce.py), [shared/counter.rs](../2026-10-28-broadcast-distance/counter.rs)
- [reconstruct.py](../2026-10-28-circle-body-coverage/reconstruct.py)

Place the last two shared files under output/shared. From the output directory,
apply check.patch using git apply with its absolute path; it reconstructs the
private executed check script exactly. The patch is harness code, not production
code. The production change is recorded in its commit and has no patch file.

```sh
python3 build.py base
python3 build.py candidate
python3 build.py base --profile
python3 build.py candidate --profile
python3 build.py mutation
python3 check.py
python3 verify-parity.py
python3 short-pairs.py
python3 measure.py stock base candidate
python3 measure.py legacy base candidate --legacy 1
python3 measure.py short base candidate --warm 0 --ticks 1080
python3 measure.py phases base-profile candidate-profile --diagnostic
python3 measure.py mutation-stock mutation --diagnostic
python3 measure.py mutation-legacy mutation --diagnostic --legacy 1
```

Run the two warm-stream comparer controls in fresh sessions too. Both commands
must fail with offset zero in mismatch.json; serve.py cleans up their children.

```sh
python3 serve.py stream-control-0 base candidate --warm 120 --ticks 1 --perturb-stream
python3 serve.py stream-control-1 base candidate --legacy 1 --warm 120 --ticks 1 --perturb-stream
```

Each mature parent runs the full backend from tick zero to 193,000 with all
recipients/broadcasts and natural food history. Keep the existing length screen;
never restore a checkpoint. The fixed measured window is 360 advances. Pair mode
compares every warm byte, two repeated children per arm and direct parent
continuations; one discarded AB warmup precedes five AB/BA/AB/BA/AB pairs.
Keep other active work parked or completed before any counted window. Retain
redacted full process command lines at boundaries.

The check delta adds full-body edit oracles in debug/release and expected failing
length-only, X-only and numeric-equality cache-key controls. The original coverage,
narrowed-block and narrowed-circle controls remain. Restored source must pass the
complete offline Rust group. For both mature output mutations, compare differing
complete trace bytes but identical tag-0 canonical physical states. All phase
windows must match the plain references completely. The earlier bit-flip comparer
controls remain required for stock/protocol 13.

Run the linked reconstruction script with OUTPUT_ROOT and the canonical fixture
path. All five generated Rust payloads must match their measured hashes; the
builder explicitly preserves the frozen terminal CRLF while committed sources
use LF. Check source and copied-binary immutability, then verify no task workers
remain and remove only the resolved bounded target, retaining sources/binaries.

[select.py](select.py) is the new aggregate projection for this study. It reads
those earlier scripts' accepted records and private pins, checks the gates, and
keeps table inputs (medians/ranges/counts), phase totals, compact source identities
and cited validation/CPU receipts. Full samples and per-file receipts remain
local-only. It compares phase traces to their plain references and checks mature
mutation trace differences and both physical states directly. Run it after
reconstruction and target-cleanup receipts are present:

```sh
python3 select.py OUTPUT_ROOT results.json
```
