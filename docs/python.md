# Python package: slither_sim

The `slither-sim` Python package runs [`serve` sessions](bot-api.md) inside the
Python process. Steps take typed arrays and frames arrive as typed columns, so
the decision loop never encodes or parses JSON. It wraps the Rust
[`transport::Session`](bot-api.md#in-process-sessions) with the same worlds,
requests and frame values as `serve`.

## Install

```sh
pip install slither-sim
```

`uv add slither-sim` works the same way. Wheels use the stable ABI, so one wheel
per platform serves CPython 3.11 and later. They cover Linux (x86-64 and ARM64,
glibc 2.17+), macOS (Apple silicon and Intel) and Windows (x86-64), and have no
runtime dependencies. Elsewhere, pip builds the source distribution, which needs
Rust 1.86+. Each crate release publishes the same version to PyPI.

## Install from a checkout

Into a uv project or virtual environment, which builds a release wheel with maturin:

```sh
uv pip install ./python
python python/examples/bot.py --steps 200
```

`pip install ./python` works the same way. To keep the wheel, build it with the
repository's locked maturin; it lands in `target/wheels`:

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
| `reset(seed=1, worlds=1, agents=4, decision_ticks=6, config=None)` | Replaces the worlds like a JSON `reset`; `config` is a dict of camelCase [world settings](configuration.md#world-config). |
| `step(headings, boosts)` | Steps every world like a JSON `step`. |
| `frame(world)` | A copy of world `world`'s current frame. |
| `request(message)` | Answers one JSON request, given as a dict, `str` or `bytes`, and returns the parsed reply dict with `protocol` and `result` or `error`. Use it for `info` and `snapshot`. |
| `worlds`, `agents` | Session shape; 0 before the first reset. |

`step` takes `worlds × agents` entries per argument, world-major: slot `s` of
world `w` is entry `w × agents + s`. Headings are radians; NaN means no action,
which only dead slots may give. Pass any buffer of float64 headings and of bool
or uint8 boosts, such as `array.array('d')`, `bytes`, `bytearray` or numpy
arrays, in any byte order and alignment; multidimensional buffers are read in C
order. Lists and other iterables
also work, with `None` for no action and any truthy value to boost.

Rejected resets and steps raise `ValueError` with the protocol's error text and
change nothing. Buffers of other item types raise `TypeError`. `frame` raises
`ValueError` before the first reset and `IndexError` for any index outside the
session; negative indices do not count from the end. `request` returns error
replies instead of raising. Requests and `config` must be plain dicts; other
mappings raise `TypeError`.

The module also exports `PROTOCOL` (`slither-sim-json-v2`), `NONE`, `NO_ORIGIN`,
`ORIGINS` and `REASONS`, with the meanings of the [Rust
constants](bot-api.md#in-process-sessions). Type stubs ship with the package.

## Frames

A frame has the scalars `time`, `ticks`, `living`, `radius` and `vision`, and one
attribute per column, named like the [C ABI](c-api.md#frames) members:

| Table | Columns |
| --- | --- |
| Snakes | `snake_slot`, `snake_x`, `snake_y`, `snake_heading`, `snake_speed`, `snake_radius`, `snake_length`, `snake_segments`, `snake_fam`, `snake_score`, `snake_kills`, `snake_body_start`, `snake_body_x`, `snake_body_y` |
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

`step`, `reset` and `request` release the GIL while the simulator runs, so
separate sessions in separate Python threads run concurrently. A session with
several worlds also steps them in parallel on the [rayon](https://docs.rs/rayon)
pool; `RAYON_NUM_THREADS` sizes it. Use one session from one thread at a time:
a call that overlaps another call on the same session raises `RuntimeError`.

## Example and timing

[`python/examples/bot.py`](https://github.com/NikitaMGrimm/slither-sim/blob/main/python/examples/bot.py)
makes the same decisions as the [JSON controller](../examples/bot.py) and prints
the same summary.
[`python/examples/timing.py`](https://github.com/NikitaMGrimm/slither-sim/blob/main/python/examples/timing.py)
times a 128-agent loop through typed steps against JSON requests. With a release
build on CPython 3.13, Windows 11 and a Ryzen 7 9800X3D, one world of 128 agents
in a 20,000-unit arena took these medians of 7 runs of 300 steps, in
milliseconds per step:

| Loop | Step | Frame | JSON encode | Decide | Total |
| --- | --- | --- | --- | --- | --- |
| Typed `step` and `frame` | 0.19 | 0.006 | — | 0.07 | 0.27 |
| `request`, with `json.loads` in its step time | 0.44 | — | 0.05 | 0.06 | 0.56 |

"Decide" is a pure-Python nearest-visible-food choice for every slot. Indexing
an `array.array` creates a Python float each time, so code that reads a column
repeatedly can call `tolist()` once. Two sessions stepping in two threads took
the same wall time as one.

## Compatibility

The package version is the `slither-sim` crate's, inherited from the workspace. Requests and replies follow
`slither-sim-json-v2`; check `PROTOCOL`. The Python API is experimental and may
change between `0.x` versions. As with `serve`, record the commit for
reproducible results.
