# slither-sim

In-process [SlitherSim](https://github.com/NikitaMGrimm/slither-sim) sessions for
Python bots. SlitherSim is a headless Slither.io simulator written in Rust. This
package runs its `serve` sessions inside the Python process: steps take typed
arrays, frames arrive as typed columns, and no JSON is encoded or parsed in the
decision loop.

```sh
pip install slither-sim
```

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

Wheels use the stable ABI for CPython 3.11 and later. They cover Linux
(x86-64 and ARM64, glibc 2.17+), macOS (Apple silicon and Intel) and Windows
(x86-64). Elsewhere, pip builds the source distribution, which needs Rust 1.99+.

- [Python package guide](https://github.com/NikitaMGrimm/slither-sim/blob/main/docs/python.md):
  API, frame columns, threads and timing
- [Bot protocol](https://github.com/NikitaMGrimm/slither-sim/blob/main/docs/bot-api.md):
  requests, worlds and frames shared with `serve`
- [Changelog](https://github.com/NikitaMGrimm/slither-sim/blob/main/CHANGELOG.md)

The package version is the `slither-sim` crate's. The Python API is experimental
and may change between `0.x` versions. MIT licensed.
