# PR comparison statistics reference

`statistics-reference.json` is a synthetic oracle for `tools/perf_stats.py`, not
a measurement. The statistician who wrote the comparison's statistics contract
generated it on 2026-10-24 with `generate_statistics_reference.py` (Python
3.12.9, NumPy 2.5.4, SciPy 1.18.1, pinned by the uv script lock beside it). Fits
use SciPy's general least squares (`scipy.linalg.lstsq`), not the balanced-design
shortcut the module uses, and the critical values use `scipy.stats.t.isf`.

The file holds the three t constants, 15 regression datasets, 8 interval-boundary
cases, 14 invalid-input mutations, 8 stopping histories and 3 block-aggregation
vectors. `tools/test_perf_stats.py` feeds them to the module, compares numbers
with relative tolerance 2e-10 and absolute tolerance 5e-12, and checks the
constants with a standard-library incomplete-beta tail. Neither the module nor
its tests need SciPy.

Regenerate offline, in this directory, only with a new contract version:

```sh
uv run --locked --script generate_statistics_reference.py
```

SHA-256 of the copied file: `f21b1548a5c5726bdcc23a53397c820a8155c219460ed72f7f63c4fde3ab401e`.
