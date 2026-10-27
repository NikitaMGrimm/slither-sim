# Infrastructure recovery, frozen before replacement collection

The first c7a.4xlarge generator was terminated by AWS Spot interruption during
the official-375 run/end-of-hold collection. Its ephemeral root and unretrieved
client receipts were lost. The completed official-500 summary and server-side
evidence remain retained, but cannot be independently re-reduced in full.
Do not silently exclude its miss or treat it as a complete qualification row.

Allow one replacement pair in the same bounded study after the first pair's
cleanup. Restart the requested four-hold schedule unchanged, retaining both
attempts and distinguishing infrastructure recovery from performance repeats.
Use the already-built, downloaded and hashed binary, without rebuilding.
Download each hold's raw load receipts before reduction to narrow the evidence
loss window. Final cost includes both attempts; the total $3 cap still applies.
No further recovery attempt if Spot interrupts again. Preserve original PLAN.md.
