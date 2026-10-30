# Official-500 on-demand acceptance v3

No host world or timing has started. [Plan](PLAN.md).

V3 reuses the interrupted v2 harness with corrected per-worker headroom:
decision + connector + publisher CPU. Both instances are on-demand; run 2 follows
a completed first run whether it passes or fails. Earlier series remain separate.
The plan and tested harness will be committed before tick zero. No capacity claim,
production runtime change, root README or changelog edit.
