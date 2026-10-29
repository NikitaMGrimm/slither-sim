# Sampling-rate correction

Recorded after the first complete counter/profile series, before any new
profile data. The diagnostic's two-hour session deadline remains unchanged.

All three counter-only repetitions completed for both protocols. The cycle and
dTLB IP profiles have no throttle or lost records. The L1d and generic-cache
profiles have no lost records but contain repeated throttle/unthrottle records:
stock L1d 1,374/1,373, stock generic cache 851/850; legacy L1d 1,472/1,471,
legacy generic cache 900/900. An enabled/running fraction of one does not rule
out overflow throttling. These runs remain recorded, without using their
delivered-IP shares as the primary miss attribution.

Repeat only the L1d and generic-cache profiles at period 10,007 rather than
1,009, retaining 1,080 ticks, the same current-main runtime, portable compilation,
no affinity, both protocols and the same mature screen. Warm a fresh backend
per protocol and fork each window as before; the original frozen parents have
already exited. Preserve original results and binaries. Keep cycle/dTLB
profiles and all counter-only results unchanged. If throttling remains, report
the limitation rather than starting another sampling attempt.

The initial reducer also looked for `name.samples` while the collector correctly
wrote `name.json.samples`. Correcting that filename consumes the existing raw
bytes; it changes no measured value. The failure is retained in local-only logs.
