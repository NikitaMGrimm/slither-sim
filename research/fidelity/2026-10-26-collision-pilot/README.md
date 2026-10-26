# Collision pilot preparation

Official pilots and scouting: **not executed**. This PR builds tooling and tests
only against a loopback SlitherSim host. [Frozen method](plan.json),
[local verification and limits](REPORT.md).

After separate collection authorization, retain the decoded official list and
sequential scouting archives privately. The selection file has this shape:

```text
{
  "serverListSha256": "<digest of retained raw list response>",
  "candidates": [
    {"listedIndex": 0, "server": {"ip": "<listed endpoint>", "port": 444},
     "population": 35, "archiveSha256": "<digest of scout archive>"}
  ]
}
```

Scout the first five active decoded entries (or all if fewer), one socket at a
time, retaining failures with `population: null`. Record the first leaderboard
within 10 seconds; close and await close before the next scout. Every scout
sends `Origin: http://slither.io`: official servers closed all five Origin-less
scouts of 2026-10-26 before any frame. The preparer binds the same Origin into
the plan unless `--origin` overrides it. `listedIndex`
is the index in the full decoded list. Population ranks ascending, then list
index; require at most 100. The preparer checks this normalized selection's
ordering, bounds and digests; the collector must independently verify its
membership in the retained official list and its first-population witnesses.
No listing, decoder, scout, endpoint or other private detail is committed here.

From the repository root, the exact preparation and pilot command are:

```sh
uv run --locked --only-group tools python tools/fidelity/prepare_collision_pilot.py --selection private/collision-selection.json --output outputs/collision-pilot
uv run --locked --only-group tools python tools/fidelity/collect.py outputs/collision-pilot
```

The preparer is offline. It copies this template unchanged except for `capture`,
retains the selection and original template, and freezes the bound plan through
`evidence.py`. Run `collect.py` once for pilot one. Its audit reports the second
attempt missing. Apply the frozen feasibility/stop gates before running the
**same collection command once more** for pilot two. Never substitute a new
plan/study to replace a failed pilot. Each invocation has a 600-second deadline;
the ledger allows two invocations total. Raw archives and endpoint-bound plans
stay private and outside Git. Audit/pack using the normal evidence workflow.

The driver also defaults to a loopback-only network gate. The dispatcher supplies
`--allow-outside yes` only for the explicitly official-server plan. This task
does not authorize that later execution.
