# Battledome food balance: frozen local plan

Design experiment, not official or community-server evidence. Compare current
main (421e384c3738e7c4d08e1eb12275733266d7a629) with the two requested rules.
Use seeds 1, 2, 3, populations 32 and 64, 600 simulated seconds, default 7 ms
physics and six-tick decisions. Sample every 10 seconds, including startup.
Keep every seed and any failed attempt. No fitting or held-out server claim.

Use the host's built-in seeker through SimulatorBackend: the same received-food
policy and background reconnect path, with the controlled slot reproducing that
small policy. This avoids introducing a new bot or external server. No boosting
is requested, so boost food should remain zero. Record all food counts and raw
nutrition by origin, living continuous mass (segments + fam), displayed score
mean/median/max, deaths, and lives that grow by at least one displayed point.
Track new lives after 300 seconds separately to check continued access to food.

Run sequentially with two-CPU process affinity, two Cargo jobs, and a 570-second
wall timeout per child. Build outside the checkout. Keep full sampled series;
report endpoint seed ranges and means, plus late growth and death rates. If
growth still accelerates or newborns stop growing, report it and propose another
experiment; do not add a third rule.

## Amendment before the usable baseline

The seeker-backend attempt was stopped after 85 wall seconds without reaching
the first 10-second sample (startup retained). The first build also failed on
the harness's `DeathEvent.snake` typo; corrected to `victim`. Both attempts remain
local under the study scratch directory. Use the other requested option:
`examples/cautious_bot.rs` unchanged, via direct Rust Simulator observations,
six-tick decisions and all slots controlled. Synthetic respawns use the host's
64-candidate, 600-unit preferred clearance and positive-margin fallback with a
separate seeded LCG. Initial lives use Simulator::add's deterministic ring.
Run the pristine baseline from a detached origin/main checkout; the baseline
results must complete before collecting the candidate. Other criteria above
remain. This measures the policy/mode combination, not a live host's load.

The first direct attempt also remained too slow: exact best-margin admission
does all body-edge comparisons for every rejected candidate. Retain that attempt
and a one-second diagnostic probe. For the usable comparison, accept the first
contact-clear ring body in 64 draws, retrying a failed slot after one simulated
second. No 600-unit preference or best-margin fallback. Keep this identical on
both sides and report living population alongside requested slots. This is a
synthetic population controller, not a host-spawning validation.
