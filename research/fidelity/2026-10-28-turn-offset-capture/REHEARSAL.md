# Final-attempt offline rehearsal

Passed before any attempt-4 official request. This rehearsal was authorized on
the owner's behalf and used a release-built local simulator, with the same image,
limits, bridge namespace, source/archive mounts and guard as the final capture.
No official server was contacted. [Machine-readable proof](rehearsal-proof.json)
binds the successful runs to the exact source fingerprint.

| Check | Observed result |
| --- | --- |
| Functioning baseline | 316.923 s; both protocol-19 setups and paired control |
| Two positively attributed connections | 315.671 s span; 307 samples, all also matched optional inode/fd owners |
| Connection bound | Maximum two in the capture namespace; no baseline host conflict |
| Deliberate third local connection from host namespace | Detected; permanent stop, both capture sockets closed |
| Sealing | Both recordings in each leg passed exact-prefix integrity |
| Windows conflict branch | Passed in the 10-test guard suite; local server was not exposed to the controller |
| Cleanup | No rehearsal containers remained |

`rehearse.ps1` is the reproduction entry point. It builds offline from vendored
dependencies in a capped container. A preparatory command-quoting failure was
corrected before either rehearsal leg; its log is retained privately.

These are authored local instrumentation tests, excluded from official candidate
scoring. The unchanged inner driver's legacy origin label does not override the
explicit outer local-rehearsal marker. Full recordings and identities stay outside
Git. Rehearsal stop/ledger state is archived and cleared only in rehearsal
directories; the official archive starts fresh after this proof is committed.

Official launch validates this proof's payload hash and source fingerprint. The
census boundary is the capture namespace, host namespace and controller, relying
on the maintaining agent's assurance that other collectors remain idle.
