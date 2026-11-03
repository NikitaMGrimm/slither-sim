# Canonical-parity evidence clarification — 2026-11-03

An independent review of #450 found that its timing launcher disabled `PROFILE_PARITY`, but the report treated the unchanged quad accumulator as a vertex-parity gate. That claim was wrong. #450 now labels those historical rows inactive and retains a separate explicitly enabled canonical control. No timing values or raw historical records were changed.

## Earlier-study audit

| Change | What actually ran | What remains supported |
| --- | --- | --- |
| #445 retained pellets | Separate `PROFILE_PARITY=1`, CPU-only controls for default view, whole-map view and an authored rectangle; 600 callbacks and zero skips in each arm. Original and reviewed controls have nonempty quad digests `c1504f711bbc51b0`, `e01a42560c5f64e5`, `397415fbac706ea5`, respectively. | The canonical command/resource/send and projected-vertex control is real. It reconstructs the generic stream and does not independently establish instanced GPU order. The separate pixel controls and authored regressions remain the GPU evidence. |
| #447 projection specialization | Canonical mode was disabled in timed runs. The report already says its constant accumulator is not world-state parity evidence. The public serial run table does not retain a quad-digest column. | Five matched release pairs, selected numeric/sent-byte controls and 6,000 actual image pairs remain supported. No new full canonical-stream claim is made for this later change. |
| #448 serialized upload mirror | Canonical mode was disabled in timed runs. The report and identities already say that the constant `d1fba762150c532c` is inactive. The recompute previously compared that column to the same constant in a control record. | Actual image hashes, selected numeric columns, sent-byte controls and matched timing remain supported. The equality of two inactive accumulators establishes nothing about vertices. |

The audit read the retained launchers, their explicit flag branch and final logs, not just matching digest strings. [canonical-evidence-audit.json](canonical-evidence-audit.json) records the twelve original/reviewed control identities and log/launcher hashes. It contains no recording, identifiers or layout. Access to full logs, frozen source trees and drivers is **restricted / local only**, outside Git; the bounded audit is public. No new server connection or performance measurement was made for this audit.

The retained-pellets index now distinguishes the original explicit canonical controls from the later serial-change image controls. The #445 check rejects the inactive accumulator in either of its genuinely active control sets. The #448 check labels its historical value diagnostic metadata and never treats its equality to another arm as parity. Original data and previous report text remain, with this dated clarification linked beside them.

## Driver correction

[driver-parity-output.patch](driver-parity-output.patch) retains the output-only correction to the private offline diagnostic driver. It increments a quad count only while canonical hashing is active. Disabled mode prints `quad_digest=absent`; enabled mode with no quads prints `empty`; only an enabled, nonempty gate prints a hexadecimal hash. It also prints the active flag and quad count. New source-snapshot generation uses this corrected output; sealed historical binaries and their hashes are preserved.

The embedded authored output test was compiled and passed separately with `rustc --test`. It checks disabled mode with both zero and nonzero counts, enabled-empty mode and enabled-nonempty mode. This validates the output helper, not a new gameplay or performance result. The pool study will rely on actual-image controls and any explicitly enabled canonical controls, never on the inactive timing accumulator.

Validation: each affected study's `recompute.py --check` and study tests pass. The added #445 regression mutates both arms to the inactive value and requires rejection. Research checks need no Git history, current-file hash gate, raw recording or network.
