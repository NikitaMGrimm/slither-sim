# Official-500 AWS acceptance v2

Collection has not started. [Plan](PLAN.md), [attempts](ATTEMPTS.md),
[reproduction and preflight checks](REPRODUCE.md).

This is a new prospective acceptance, following
[failed v1](https://github.com/NikitaMGrimm/slither-sim/blob/55f2921594aff00044624e47a8a1f90c4ee0b073/research/performance/2026-10-30-official-500-acceptance/REPORT.md).
V1 remains failed and is not pooled with v2. The maintaining agent corrects the
end-class upper bounds before v2 data; entry screen and all other gates remain.
Generator reconnect/close work and all output move off decision loops, including
the endpoint histogram publication that caused v1's six lateness violations.
No production runtime or root README/changelog change.
