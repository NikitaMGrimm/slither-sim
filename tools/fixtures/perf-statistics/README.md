# PR comparison statistics reference

New runs use `protocol-v2-reference.json`, `protocol-v2-critical-values.csv` and
`generate_v2_reference.py`/lock. The 14 covered looks and exact allocation follow
[protocol v2](../../../docs/maintainers/performance-statistics-v2.md).
The copied CSV SHA-256 is
`de2f7e7c0c5a0c00faa09a5c764dcbec9b4fe0192cdaa16bc032f6ffbf4c5303`.
The synthetic v2 oracle uses SciPy general least squares and checked t tails,
not hosted calibration. Reproduce with
`uv run --locked --script generate_v2_reference.py`.
Files below retain their historical v1 meaning.

`statistics-reference.json` is a synthetic oracle for `tools/perf_stats.py`, not
a measurement. The contract's statistician generated it on 2026-10-24 with
`generate_statistics_reference.py` (Python
3.12.9, NumPy 2.5.4, SciPy 1.18.1, pinned by the uv script lock beside it). Fits
use SciPy's general least squares (`scipy.linalg.lstsq`), not the balanced-design
shortcut in the module; critical values use `scipy.stats.t.isf`.

It holds three t constants, 15 regression datasets, 8 interval-boundary
cases, 14 invalid-input mutations, 8 stopping histories and 3 block-aggregation
vectors. `tools/test_perf_stats.py` feeds them to the module, compares numbers
with relative tolerance 2e-10 and absolute tolerance 5e-12, and checks constants
with a standard-library incomplete-beta tail. Neither module nor tests need SciPy.

Regenerate offline, in this directory, only with a new contract version:

```sh
uv run --locked --script generate_statistics_reference.py
```

SHA-256 of the copied file: `f21b1548a5c5726bdcc23a53397c820a8155c219460ed72f7f63c4fde3ab401e`.
