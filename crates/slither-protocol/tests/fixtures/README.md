# Protocol-13 catalog snapshot

`protocol-catalog-13.json` is a byte-identical packaging copy of the existing
repository fixture at `tests/fixtures/protocol-catalog-13.json`. Its provenance,
redactions, selections, source hashes and reference-decoder expectations are
unchanged. It contains derived official catalog witnesses, not community-server
challenges. The copy lets the published crate run its tests independently of
the simulator source tree; the checkout test also checks the two copies agree.

See the [catalog report](https://github.com/NikitaMGrimm/slither-sim/blob/main/research/fidelity/2026-10-26-protocol13-catalog/REPORT.md).
Synthetic cases in the adjacent Rust tests complement these 33 records; no
server encoder generates their expectations.
