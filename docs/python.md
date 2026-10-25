# Python package: slither_sim

`slither-sim` wraps Rust [`transport::Session`](bot-api.md#in-process-sessions)
in Python, with the worlds, requests and frame values of [`serve`](bot-api.md).
Typed-array steps and frame columns avoid JSON in the decision loop.

## Install

```sh
pip install slither-sim
```

Or use `uv add slither-sim`. Stable-ABI wheels serve CPython 3.11+ on Linux
(x86-64 and ARM64, glibc 2.17+), macOS (Apple silicon and Intel) and Windows
(x86-64), without runtime dependencies. Elsewhere, pip builds the source
distribution with Rust 1.99+. Each crate release publishes the same version to PyPI.

## Install from a checkout

Build a release wheel with maturin into a uv project or virtual environment:

```sh
uv pip install ./python
python python/examples/bot.py --steps 200
```

Or use `pip install ./python`. To keep the wheel in `target/wheels`, use locked maturin:

```sh
uv run --locked --only-group tools python -m maturin build --release --locked -m python/Cargo.toml
```

## API

```python
import math
import slither_sim

session = slither_sim.Session()
session.reset(seed=7, worlds=2, agents=4, config={"radius": 1000, "dynamicRadius": False})
headings = [math.nan if me == slither_sim.NONE else 0.0
            for world in range(session.worlds) for me in session.frame(world).view_self]
session.step(headings, bytes(len(headings)))
print(session.frame(1).time)
```

| Call | Purpose |
| --- | --- |
| `Session()` | An empty session; reset it before stepping. |
| `reset(seed=1, worlds=1, agents=4, decision_ticks=6, config=None)` | Replaces the worlds like a JSON `reset`; `config` is a dict of camelCase [world settings](configuration.md#world-config), optionally with a [`preset`](configuration.md#presets). Each reply warning about valid but unmeasured settings is issued as a `UserWarning`. |
| `step(headings, boosts)` | Steps every world like a JSON `step`. |
| `frame(world)` | A copy of world `world`'s current frame. |
| `request(message)` | Answers one JSON request, given as a dict, `str` or `bytes`, and returns the parsed reply dict with `protocol` and `result` or `error`. Use it for `info` and `snapshot`. |
| `worlds`, `agents` | Session shape; 0 before the first reset. |

`step` takes `worlds × agents` entries per argument, world-major: slot `s` of
world `w` is entry `w × agents + s`. Headings are radians; only dead slots may
use NaN for no action. Pass float64 heading and bool/uint8 boost buffers
(`array.array('d')`, `bytes`, `bytearray`, numpy), any byte order/alignment;
multidimensional buffers use C order. Other iterables, including lists, accept
`None` for no action and truthy values to boost.

Rejected resets and steps raise `ValueError` with the protocol's error text and
change nothing. Buffers of other item types raise `TypeError`. `frame` raises
`ValueError` before the first reset and `IndexError` for any index outside the
session; negative indices do not count from the end. `request` returns error
replies instead of raising. Requests and `config` must be plain dicts; other
mappings raise `TypeError`.

Advancement or frame-collection failure makes `step` raise `SessionEndedError`
(a `RuntimeError` subclass) and clear every world: `worlds` and `agents` become 0;
`frame` raises `ValueError` ("Reset first"). Reset to continue. `request` instead
returns an error reply with `sessionEnded: true` (false for unchanged-state errors).

The module also exports `PROTOCOL` (`slither-sim-json-v3`), `NONE`, `NO_ORIGIN`,
`ORIGINS` and `REASONS`, with the meanings of the [Rust
constants](bot-api.md#in-process-sessions). Type stubs ship with the package.

## Frames

A frame has the scalars `time`, `ticks`, `living`, `radius` and `vision`, and one
attribute per column, named like the [C ABI](c-api.md#frames) members:

| Table | Columns |
| --- | --- |
| Snakes | `snake_slot`, `snake_x`, `snake_y`, `snake_heading`, `snake_speed`, `snake_radius`, `snake_length`, `snake_segments`, `snake_fam`, `snake_score`, `snake_kills`, `snake_kill_score`, `snake_body_start`, `snake_body_x`, `snake_body_y` |
| Food | `food_x`, `food_y`, `food_value`, `food_size`, `food_origin` |
| Prey | `prey_id`, `prey_x`, `prey_y`, `prey_heading`, `prey_target_heading`, `prey_speed`, `prey_size` |
| Views | `view_self`, `view_snakes_start`, `view_snakes`, `view_food_start`, `view_food`, `view_prey_start`, `view_prey` |
| Deaths | `death_time`, `death_victim`, `death_killer`, `death_reason`, `death_food` |

Columns are `array.array` copies with fixed typecodes: `'d'` (float64) for
measures, `'I'` (uint32) for slots, counts and row indices, `'Q'` (uint64) for
prey IDs and `'B'` (uint8) for `food_origin` and `death_reason`. Values match the
JSON frame, rounded alike. Slot `s` sees the food rows
`view_food[view_food_start[s]:view_food_start[s + 1]]`, and likewise snakes and
prey; `view_self[s]` is its own snakes row, or `NONE` when dead. A frame stays
valid after later steps.

With numpy, `np.frombuffer(frame.snake_x, dtype=np.float64)` views a column
without another copy; use `np.uint32`, `np.uint64` and `np.uint8` for the other
typecodes.

## Threads

`step`, `reset` and `request` release the GIL, allowing separate sessions in
separate Python threads to run concurrently. Worlds within a session step on
the [rayon](https://docs.rs/rayon) pool, sized by `RAYON_NUM_THREADS`.
Overlapping calls on one session raise `RuntimeError`.

## Example and timing

[`python/examples/bot.py`](https://github.com/NikitaMGrimm/slither-sim/blob/main/python/examples/bot.py)
makes the [JSON controller](../examples/bot.py)'s decisions and prints its summary.
[`python/examples/timing.py`](https://github.com/NikitaMGrimm/slither-sim/blob/main/python/examples/timing.py)
compares typed steps with JSON requests. Medians in ms/step over 7 runs of 300
steps: release build, CPython 3.13, Windows 11, Ryzen 7 9800X3D, one world of
128 agents in a 20,000-unit arena.

| Loop | Step | Frame | JSON encode | Decide | Total |
| --- | --- | --- | --- | --- | --- |
| Typed `step` and `frame` | 0.19 | 0.006 | — | 0.07 | 0.27 |
| `request`, with `json.loads` in its step time | 0.44 | — | 0.05 | 0.06 | 0.56 |

"Decide" is a pure-Python nearest-visible-food choice for every slot. Indexing
an `array.array` creates a Python float each time, so code that reads a column
repeatedly can call `tolist()` once. Two sessions stepping in two threads took
the same wall time as one.

## Compatibility

The package inherits the workspace's `slither-sim` crate version. Check
`PROTOCOL` (`slither-sim-json-v3`) and record the commit for reproducibility.
The Python API is experimental and may change between `0.x` versions.
