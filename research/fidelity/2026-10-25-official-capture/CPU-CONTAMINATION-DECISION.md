# Load decision before continuation acquisition

After more than five minutes, every ten-second total-CPU sample was above
30% (roughly 48-69%). The parent explicitly allowed recording load and marking
batches contaminated instead of waiting for a quiet machine. `cpu_record.py`
therefore retains the host-process gate, samples total CPU before and during
every batch, and records `cpuContaminated` for samples at or above 30%.
Any such sample contaminates the entire batch for confirmatory model adoption.
Full and collector-clean results remain descriptive; no clean mask removes the
machine-level contamination label. This decision precedes all continuation
connections. The strict gate and its unsuccessful waits remain retained.
