# Attempts and limits

- Initial current-main profile: 500 seats, the retained #242 harness, 1,000
  warmup and 200 counted decisions. The old physics hook no longer matched
  current main; that overlay failed before compilation. Restricting reuse to
  bot hooks then exposed the added broadcast `false` argument. Both failures
  remain in raw output; the adapted bot-only overlay builds.
- Finer head prediction and food/snake brackets preserve the uninstrumented
  world's endpoint. An exploratory head-prefix-only candidate passes the exact
  replay oracle and preserves that endpoint, but saves only about 0.8% of the
  four bot stages. It does not qualify alone. These pilots select the change;
  their diagnostic costs are not substituted for the frozen paired comparison.
- All 39 protocol tests pass with the food snapshot candidate. Removing its
  insertion invalidation is rejected by the warmed/fresh replay oracle.
  Perturbing the current interval duration is rejected by the bitwise motion
  oracle. Both tests pass after exact source restoration.
- The first full Rust group passes formatting and every Clippy configuration,
  then fails compiling existing transport tests: `CARGO_BIN_EXE_slither-sim`
  is missing. A focused repeat fails the same way. The guest's `rustc` is
  launched through a POSIX shell wrapper, which strips environment keys with
  hyphens. A task-local `RUSTC` pointing directly to the installed Rust binary
  makes all 15 transport tests pass; `RUSTDOC` is selected the same way for the
  repeated full group. No wrapper, system configuration or repository test was
  changed. Retain both failed logs. On installations with wrappers, export
  `RUSTC=/path/to/toolchain/bin/rustc` and `RUSTDOC=/path/to/toolchain/bin/rustdoc`
  before `verify.py`.
- Persistent storage was nearly full. Disposable release targets are removed
  after copying and hashing binaries, and this task's stopped test target was
  relocated to temporary storage. Raw evidence and other workers' files are
  preserved. Traces stream directly into gzip to avoid uncompressed copies.
- Main advanced during work with #242's study and #243's radius-limit error
  clarification. Measurements keep their frozen task-start baseline and
  candidate pins. Neither added commit changes the client-frame implementation
  or successful official-500/2,000 configuration.
- The first food snapshot candidate still used element-wise `Food::clone`.
  Its fresh stage sum saved only about 0.5%, below the threshold. It is not
  the selected measurement arm. Declaring the already value-only `Food` as
  `Copy` enables contiguous vector copies; a separate pilot reduces the sum
  by about 3.5%. Clippy requires mechanical clone-to-copy edits in callers
  and fixtures; no policy arithmetic or spawn-admission predicate changes.
  The final measured runtime includes that cleanup at `64490e2e`.
- A trace-only stdout adapter forgot to flush its buffered terminal record
  before the donor's summary `println!`. Both raw streams were equal, but
  the summary interrupted the final JSON line. Those invalid streams were
  copied off the guest and rehashed before freeing their guest copies. They
  are rejected evidence, not accepted JSONL. New trace-only binaries flush
  before the summary; counting binaries are separate. The intermediate
  candidate trace and in-progress pairs were stopped after the nonqualifying
  candidate was replaced; partial output is retained. No interrupted pair
  enters the selected five-pair comparison.
- The complete Rust group passes for the pre-`Copy` prototype using the direct
  compiler. Final qualification repeats the group after the copy cleanup.
- Final runtime `64490e2e` passes the complete Rust group (all Clippy feature
  configurations, default/core/bot/networking all-target tests, doctests, C ABI,
  Python crate check and basic example), both restored oracles, both mutation
  controls and all seven documentation tests. The complete test source was
  checked against the candidate archive before this qualification. The final
  temporary compiler target was deleted after the group completed.
- The first documentation check during report preparation correctly rejects
  the new index/changelog links before `REPORT.md` exists. After adding the
  report and staging the study, all seven checks pass. The result selector
  rejects missing/incomplete paired evidence, and the reproduction driver
  refuses Python `-O` so assertion-based gates cannot disappear.
- Final selection accepts all five alternating pairs plus the untallied warmup:
  total median 229.253k -> 222.956k instructions per background decision,
  reductions 2.7466–2.7468%. All endpoints match and all counted brackets have
  100% PMU scheduling. The selector rechecks every copied binary, all 23
  retained decompressed matrix traces against their partners and hashes, and
  full catalog coverage. The validated bot trace retains 597,804 actions.
- Before delivery, main adds #244's spawn bounds and the v0.7.1 release. Merge
  `4e68c804` into the PR branch, preserving both. Only the changelog conflicts;
  the client implementation, `Food`, shared policy and new regressions stay
  byte-identical to measured `64490e2e`. Existing performance numbers retain
  their original pins; no combined-runtime speed claim is substituted.
