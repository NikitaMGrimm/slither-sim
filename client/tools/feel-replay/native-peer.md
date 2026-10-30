# Offline native and desktop CPU peers (A3)

The `feel-replay` Rust example drives production LocalGame with synthetic
transport, unsplit packets, real input policy, effects and prepared pictures.
No socket, window, winit loop, GPU or game server is created. The v3 flat ABI
matches the browser recorder. Pictures keep immutable preparation observations
through issuance/retry; recorder identities bind actual acknowledgements.
Recoverable memory skips stay prepared, unsubmitted and uncommitted.

Run from `client/` with Rust 1.99, Node 24 and wasm-bindgen 0.2.129. Stage
intended changes, then build sequentially with `CARGO_BUILD_JOBS=2`:

```powershell
$env:CARGO_BUILD_JOBS='2'
$env:WASM_BINDGEN='<absolute wasm-bindgen 0.2.129 executable>'
node tools/build-feel-native.mjs
node tools/build-feel-replay.mjs
node tools/feel-browser-replay.mjs --record tools/feel-replay/browser-synthetic.json --out <fresh-wasm-directory> --browser 'C:/Program Files/Google/Chrome/Application/chrome.exe' --commands 7 --appearance-at 3
node tools/feel-native-replay.mjs --record tools/feel-replay/browser-synthetic.json --wasm-run <fresh-wasm-directory> --out <fresh-native-directory> --commands 7 --appearance-at 3
node tools/feel-desktop-replay.mjs --record tools/feel-replay/browser-synthetic.json --wasm-run <fresh-wasm-directory> --out <fresh-desktop-directory> --commands 7 --appearance-at 3 --hz 60 --ack-ms 0
```

The native executable writes ordinary JSON once. Node adds the small per-run
manifest and invokes the same perceptual comparator as the browser. No Rust
assembly round trip, custom codec, exact report or mandatory repeat is needed.
The executable/build receipt is checked against this run's own staged source;
a baseline can have another source revision/binary. Any valid completed Canvas
trace with the same tape/frozen schedule/geometry is admitted; no summary-specific
trace hash is required. Preserve failed runs in fresh directories outside Git.

The native mapper reuses browser CSS normalization, mouse compatibility expansion
and production target admission. CSS MouseEvent floors its double initializer;
PointerEvent retains fractions. Do not apply native physical/f32/DPR conversion a
second time. This does not establish physical OS/DOM input parity. Negotiation
bytes are discarded; safe command bytes/times and actual effect receipts remain.

## Desktop ordering

The default equal-schedule lane eagerly acknowledges prepared pictures. The
desktop lane instead uses production `service_ingress`, `begin_redraw`,
`take_frame` and `presented`, issuing at most one picture per redraw.
Superseded Setup candidates retain core/camera observations but receive no
successful-picture acknowledgement. Production quality credit for completed
Setup work is distinct from replay's acknowledged-picture counter.

`--hz` freezes cadence. `--ack-ms frame` and numeric blocking intervals are
authored CPU blocking experiments: ingress waits, redraws coalesce and the
lifecycle sidecar retains receipt/service/preparation/ack clocks. The comparator
still checks the original frozen schedule; changed service/count histories fail
rather than being aligned, trimmed or promoted by a separate exact diagnostic.
Before desktop execution the runner freezes an authored zero-delay receipt
expectation from the Canvas baseline: packet Setup preparations are superseded,
all other scheduled pictures are acknowledged. It retains every numeric row and
preparation boundary, adjusts only expected receipt flags/clocks and cumulative
counts in a separately labelled expected trace, and never rewrites the baseline.
The same comparator gates this expected desktop history. The original cross-host
comparison remains visible in `cross-host-comparison.json`: Canvas acknowledged
Setup pictures and desktop superseded them. A delayed experiment still needs an
independently authored expected service history to pass a changed schedule. CPU acknowledgements are not display or GPU completion.

## Checks

```sh
node --test tools/feel-native-replay.test.mjs tools/feel-desktop-replay.test.mjs
cargo test --offline --locked --features feel-trace --manifest-path apps/wgpu-slice/Cargo.toml --example feel-replay
cargo test --offline --locked --features feel-trace --test feel_trace
cargo test --offline --locked --features feel-trace --manifest-path apps/wgpu-slice/Cargo.toml --test feel_trace
```

These synthetic CPU checks cover staged copies, effect acknowledgements,
Setup supersession, cumulative successful-picture counts, retries without
resampling, decimal cadence edges and the unchanged perceptual budgets.
They establish neither rasterization nor physical latency/perceptual acceptance.
