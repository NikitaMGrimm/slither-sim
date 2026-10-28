# Frozen offline wall-sliding comparison

Commit this plan before new candidate scores or inspection of raw recording
positions. Prior exposure: #233's formula map/ranking, the current constraint and
boundary tests, all three selected browser-fixture groups (rows were displayed
during provenance inspection), and the historical boundary-evidence note and
headless analyzer. Their published projection and terminal outcomes are known.
This is retrospective research, not a blind replication.

## Pins, inventory and split

- Simulator base: `d66ebc08ecc557187748280e2b8745aaabd92f2c`.
- Client reference: clean read-only external clone at
  `8d0772c015d6b9b73b80253db8c7349aa600b201`. Read its received-radius and anchor
  semantics; do not read official JS/assets or modify/build the owner's checkout.
  The existing Python protocol decoder suffices for received coordinates; pin
  its hash and check the relevant layouts against this client.
- Canonical inherited implementation: Arena initial import
  `895e6f6b5b8c4d54aefbcfae4a16c8e6b17a18ac`, `src/boundary.rs` and
  `tests/fixtures/boundary-contact.json`. Retained explanation/analyzers:
  `c2fb7285febb4273034e30699dc880715a334423`,
  `docs/research/boundary-evidence.md`, `tools/fidelity/analyze-boundary.py` and
  `tools/fidelity/analyze-headless-boundary.py`.
- Inventory every archive in the existing 91-entry
  `research/fidelity/2026-10-26-arena-radius/source-inventory.json`, verifying
  compressed hashes and complete gzip/JSON parsing before admission. Only that
  historical official headless root is searched, never community captures.
  Missing/mismatched/incomplete/non-19 sources remain in the exclusion ledger.
  Deduplicate compressed hashes. Do not add archives after seeing scores.
- Development: the 593-row browser fixture and
  `rim-angle-discovery-20260927/bot0.jsonl.gz` (SHA-256
  `77cce972026da4cab77c5868c391fc046a1bb455ddf4e1ea86d6f875248e663f`).
- Directory-level validation: every other inventory entry. Separately report
  the known historical validation sources `rim-angle-validation-20260927`
  (`61938fd7217126c22f95eff9e63ad49667759eab1a917756fe53a05bc04cef7f`)
  and `rim-boost-threshold-20260927`
  (`493fc1eb7eece8906e8c40a8355a0f79d302590f25b5c5273fa168e5cc55c481`).
  Their outcomes are already published: this source split is held out from any
  new fitting, but is not fresh confirmatory evidence. Report other-directory
  exposure separately. Do not turn within-source tails into independent trials.
- Original browser raw captures are reported unavailable by the canonical note.
  Check the named retained roots for them; do not treat the fixture's hashes as
  newly verified raw-file integrity or invent missing packet clocks.

## Exact candidate rules

Use world coordinates centered on the received setup center and the latest
received physical target radius R from setup/z, never the eased drawn radius.
Scale is clamp(1+(n-2)/106,1,6). Heading is h, u=(cos h,sin h), proposed movement
is p'=p+v*dt*u. Never rotate the heading as a consequence of contact.

1. **P16 (current):** call the exact current `boundary::constrain` on p', h,
   scale and R. It checks center outside R as lethal, then forward contact point
   t=p'+16*scale*u. Contact below R does nothing. Contact is lethal when
   dot(t,u)/|t| > sin(.7); otherwise p_next=R*t/|t|-16*scale*u.
2. **P15:** identical with lead 15*scale. No fitting of the lead.
3. **P0:** identical center projection with lead zero (lethal outward check
   still applies). Suppress the already-outside-center kill only in this
   explicit head-projection alternative, so shallow contact can project.
4. **H (no sliding):** free proposed movement, lethal at |p'|>=R. Also retain
   unconstrained proposed endpoints as a labelled free-motion diagnostic;
   survival after H's predicted death is a contradiction, not a zero residual.
5. **T16:** on P16's contact condition, remove the positive outward velocity
   component using the unit normal at p+16*scale*u, then advance from p by that
   projected displacement and apply P16 for radius correction/lethality.
   Before contact use normal free movement. This distinguishes velocity
   projection from the current finite-step position projection.
6. **D90:** same as T16, multiplying its tangential contact displacement by .9.
   This is a fixed 10% damping alternative, not a fitted coefficient.

Primary integration step is 7 ms, with 3.5 and 14 ms sensitivities. Apply received
heading/speed/count/radius changes in original packet order at their server-header
times, splitting an integration step at each event. No fitted lag, per-interval
speed, heading, radius, offset, death cutoff or initial state. Check the Python
P16 mirror against the actual Rust function on deterministic authored inputs.

Separately tabulate forward-tip radial residual for leads 0/15/16 on all fixed
browser fixture rows. These position-only data cannot establish motion damping,
clock phasing, or the lethal cutoff. Also describe terminal observations without
substituting requested phase angles for an untransmitted collision pose.

## Eligibility and metrics

Inventory received own-anchor events within 250 units of R (including slightly
outside), recording absolute versus directed/reconstructed anchors, protocol,
life, physical target-radius changes, count and actual reported speed. Never
use predicted/extrapolated/rendered heads as authoritative samples. Preserve
directed-angle accumulation since the last absolute/body origin.

Score every consecutive own-anchor interval in one incarnation with server gap
in [50,700] ms, both center gaps in [-5,40] units, positive outward angle
asin(dot(radial_unit,heading_unit)) in [.35,.68] radians, matching tangent
direction at both ends, no count change inside the
interval, actual reported speed <=14.1 wire units and >0, and nearest visible
other body anchor >200 world units at both ends (empty cache is only absence of
known interference). Split on setup/spawn/removal/death. No model-error filter.
Initial positions are reset only at each interval's first received anchor.

Primary stratum additionally requires unchanged received R and heading age <=150
ms at both endpoints; report all eligible intervals as a broader diagnostic.
Report a <=42-ms age sensitivity. Separate cruise (<7 wire units), boost (>13.5),
intermediate speed, directions, sources and lives. Zero exposure stays null.

Primary residual is Euclidean predicted-minus-received endpoint distance, with
signed radial/tangential components; report median, p95, max and RMSE. Count
model-lethal intervals separately, retaining their last simulated pose and
unconstrained diagnostic. Do not rank a model after silently deleting its kills.
Report observed/free and observed/predicted displacement ratios diagnostically.
Adjacent intervals are correlated; source/life is the replication unit.

## Uncertainty and decision

Wire heading quantum is 2*pi/256, directed-anchor angle quantum 2*pi/65536,
speed quantum 1/18 per .032 s, absolute anchor coordinate cell one world unit,
appearance/body-origin cell .2 and intermediate body-delta cell .5 per axis.
Use full cells, not an assumed encoder rounding policy. Bound accumulated
origin error by sqrt(2)*(.2+.5*body_delta_count), plus one full directed-angle
chord error for every encoded 42-unit step since that origin. Absolute resets
replace this bound by sqrt(2). These are conservative conditional bounds.

For each interval duration T and maximum reported world speed V, compute
Epos=Eanchor_start+Eanchor_end, Eheading=V*T*(2*pi/256),
Espeed=T/(18*.032), Eclock=V*(.001+2*j/1000), for j=0,7,14,42 ms.
Add received-radius variation over the interval expanded by j+7 ms, plus two
full radius units. Sum the terms. For projection include the same radial-origin
budget; this is a sensitivity screen, not a proof for discontinuous death gates.
Also report the observed held-heading-staleness allowance
V*T*min(2, omega_max*(max_heading_age+j)/1000), omega_max=.033/.008, separately.
Held received headings do not prove hidden headings remained fixed.

Compare candidate endpoint separation with twice the same budget, not just the
smaller RMSE. Repeat primary scores with all received events shifted together
by +/-j ms, preserving order, as timing diagnostics only. Retain uncertain kills
and terminal reports explicitly: bounded endpoint error is not a lethal-threshold
confidence interval. Header clocks and packet receipts do not establish a bound
on event-to-send age or on clock-rate error.

Call current P16 **compatible/descriptively preferred** only with visible
denominators and limits. Reject an alternative conditionally only if at j=42 its
endpoint error exceeds the budget on >=5 primary intervals in each of >=2 source
directories while P16 is within budget there and does not kill those intervals.
Report retrospective validation directories separately from development. A lower
RMSE alone is insufficient. With no fresh held-out capture and hidden-state
uncertainty, do not claim an identified exact wall law or mechanics correction.
If evidence nonetheless supports a correction, stop and report; do not implement.

## Power and full-selector dry run

For unresolved pairs, compute displacement/radial-gap separation at counts
2,7,37,100,174,343,500, scales capped at 6; outward angles .35,.5,.6,.65;
R=6000,10000,14000; cruise and boost; durations .3,1,3,10,30,120,1200 seconds.
Include full-cell heading/speed, anchor-origin/radius and j=42 timing budgets.
Show which systematic terms grow with duration; repetitions cannot average them
away. At most 4*1200 seconds and two concurrent connections are available to a
future design. No unobserved phase, large-size attainment or boost-fuel plateau
is assumed feasible merely because ideal arithmetic separates a pair.

Use a capped, one-CPU local simulator with the existing headless boundary probe
to dry-run the full frozen eligibility selector. Require actual selected
intervals/cells, not just successful sockets or guard behavior. Retain failed
acquisition and logs; do not tune the official selector to make the rehearsal
pass. Report outcomes by speed/direction and the attainable contact rate.

Propose a future capture only for an unresolved pair that passes both the
uncertainty power gate and that full-selector local dry run. Freeze cells,
durations, interval counts, exclusions, development/held-out allocation and
stopping rules in the report, explicitly **not approved and not collected**.
If any required gate fails, propose no capture and explain the limiting term.
No outside server, discovery, endpoint probe, browser or remote machine is used.

## Delivery and resources

One research PR only: report, analysis/focused tests, bounded selected JSON,
plain index row and one short changelog bullet. Full raw/derived rows stay
outside Git. No simulator/fixture/client/workspace/contract change. No official
JS/assets, credentials, personal paths, endpoint or machine details in Git.
One local logical CPU for the entire task; builds -j 1, test/Rayon threads 1,
serial builds/analysis, modest RAM, disposable external targets cleaned afterward.
