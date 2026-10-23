# Client presentation

Pellet colors are assigned in the client backend, outside physics and bot
observations. The earlier simulator bridge already had this layer; the standalone
host initially omitted it and encoded every pellet as color 0.

The retained source is `slither-arena-rs`'s `tools/local_slither_presentation.py`
and October 3 client-presentation report, including pooled ambient weights from
commit `a0587bc87fb010d29e7dfdde00e5ea83587078e2`. The anonymous
[fixture](../tests/fixtures/client-presentation.json) retains stock recipes,
verified received prefixes, archive hashes, custom remaps and timing receipts.
Original recordings are not distributed and were not recounted for this port.

- Ambient births draw from the pooled counts for colors 0..8:
  `12536,38648,11965,16363,13075,10999,5350,21929,11243` (142,108 births).
  Individual archives differ substantially; these are a pooled presentation
  model, not a recovered official random generator or universal distribution.
- Primary skins 0..8 produce the matching boost/death food color. Source slots
  come from admitted birth telemetry, never proximity to a snake.
- Stock death recipes retain their received order, including skin 15's rainbow
  and skin 62's repeated-zero quartet. Custom run lengths survive remapping.
  Starting phase, rejected-birth advancement and fancy/custom boost equivalence
  remain unverified. Custom white chooses one of the observed 0/1/2 outcomes per
  life; unknown stock skins use an explicitly approximate modulo-9 fallback.
- A separate seeded presentation RNG assigns each birth once. Ground colors
  survive source death, respawn, reconnection and sector re-entry. Physics RNG,
  `Food`, policy observations and the bot contract do not gain color fields.
  Co-located pellets retain their insertion-ordered lifetimes; the wire exposes
  the first. Pickup consumes them in physics order before ambient replenishment.
- Sector listings carry explicit colors. Incremental ambient births use `f`;
  boost/death births use `b`. Real pickup telemetry supplies protocol-19 `<`
  removals with the eater's slot; unexplained disappearance carries no invented
  eater. Birth and pickup buffers are consumed before each physics step resets
  them, including births between broadcasts.

The fixture's 1,170-birth ambient sample predates the larger pooled weights;
they describe different cohorts. Neither the retained table nor this port
establishes an official prey-color distribution.

Post-death streaming and browser timing are described in [hosting](raw-server.md#reports-and-game-over).
