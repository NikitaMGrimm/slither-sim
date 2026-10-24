"""Time a 128-agent decision loop in-process: typed steps and frames against JSON requests.

Every living slot drives the same outward-drifting headings either way, so both loops
simulate the same worlds. Each step also runs a pure-Python nearest-visible-food
decision for every living slot, as a stand-in for bot logic. Prints the median
over runs of each part's mean milliseconds per step. Build the package in release
mode for meaningful numbers.
"""

import argparse
import json
import math
import statistics
import threading
import time

import slither_sim

AGENTS = 128
CONFIG = {"radius": 20000, "dynamicRadius": False}


def drift(step, living, missing):
    """Headings for living slots, and `missing` for dead ones."""
    return [2 * math.pi * i / AGENTS + 0.3 * math.sin(step / 20 + i) if alive else missing
            for i, alive in enumerate(living)]


def nearest_typed(frame):
    targets = []
    food_x, food_y, view_food, food_start = frame.food_x, frame.food_y, frame.view_food, frame.view_food_start
    for slot, me in enumerate(frame.view_self):
        if me == slither_sim.NONE:
            continue
        x, y = frame.snake_x[me], frame.snake_y[me]
        start, end = food_start[slot], food_start[slot + 1]
        if end > start:
            target = min(view_food[start:end], key=lambda i: (food_x[i] - x) ** 2 + (food_y[i] - y) ** 2)
            targets.append(math.atan2(food_y[target] - y, food_x[target] - x))
    return targets


def nearest_json(frame):
    targets = []
    snakes, food = frame["snakes"], frame["food"]
    food_x, food_y = food["x"], food["y"]
    for view in frame["views"]:
        if view is None:
            continue
        x, y = snakes["x"][view["self"]], snakes["y"][view["self"]]
        if view["food"]:
            target = min(view["food"], key=lambda i: (food_x[i] - x) ** 2 + (food_y[i] - y) ** 2)
            targets.append(math.atan2(food_y[target] - y, food_x[target] - x))
    return targets


def typed_run(seed, steps):
    session = slither_sim.Session()
    session.reset(seed=seed, agents=AGENTS, config=CONFIG)
    parts = {"step": 0.0, "frame": 0.0, "decide": 0.0}
    frame = session.frame(0)
    for step in range(steps):
        t0 = time.perf_counter()
        nearest_typed(frame)
        headings = drift(step, [me != slither_sim.NONE for me in frame.view_self], math.nan)
        t1 = time.perf_counter()
        session.step(headings, bytes(AGENTS))
        t2 = time.perf_counter()
        frame = session.frame(0)
        t3 = time.perf_counter()
        parts["decide"] += t1 - t0
        parts["step"] += t2 - t1
        parts["frame"] += t3 - t2
    return parts, (frame.time, frame.living)


def json_run(seed, steps):
    session = slither_sim.Session()
    reply = session.request({"op": "reset", "seed": seed, "agents": AGENTS, "config": CONFIG})
    frame = reply["result"]["worlds"][0]
    parts = {"encode": 0.0, "request": 0.0, "decide": 0.0}
    for step in range(steps):
        t0 = time.perf_counter()
        nearest_json(frame)
        headings = drift(step, [view is not None for view in frame["views"]], None)
        t1 = time.perf_counter()
        line = json.dumps({"op": "step", "actions": [{"heading": headings, "boost": [False] * AGENTS}]})
        t2 = time.perf_counter()
        # request() steps, encodes the reply in Rust and parses it with json.loads.
        frame = session.request(line)["result"]["worlds"][0]
        t3 = time.perf_counter()
        parts["decide"] += t1 - t0
        parts["encode"] += t2 - t1
        parts["request"] += t3 - t2
    return parts, (frame["time"], frame["living"])


def threads_run(seed, steps, threads):
    """Wall seconds for `threads` sessions stepping at once, each in its own thread."""
    def run(index):
        session = slither_sim.Session()
        session.reset(seed=seed + index, agents=AGENTS, config=CONFIG)
        for step in range(steps):
            session.step([2 * math.pi * i / AGENTS for i in range(AGENTS)], bytes(AGENTS))

    workers = [threading.Thread(target=run, args=(index,)) for index in range(threads)]
    start = time.perf_counter()
    for worker in workers:
        worker.start()
    for worker in workers:
        worker.join()
    return time.perf_counter() - start


def report(label, runs, steps):
    medians = {part: statistics.median(run[part] for run in runs) * 1000 / steps for part in runs[0]}
    parts = "  ".join(f"{part} {value:.3f}" for part, value in medians.items())
    print(f"{label:6} {parts}  total {sum(medians.values()):.3f} ms/step")


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--seed", type=int, default=7)
    parser.add_argument("--steps", type=int, default=300)
    parser.add_argument("--runs", type=int, default=7)
    args = parser.parse_args()
    typed, encoded = [], []
    for _ in range(args.runs):
        parts, typed_end = typed_run(args.seed, args.steps)
        typed.append(parts)
        parts, json_end = json_run(args.seed, args.steps)
        encoded.append(parts)
        if typed_end != json_end:
            raise SystemExit(f"loops diverged: typed {typed_end}, JSON {json_end}")
    print(f"{AGENTS} agents, seed {args.seed}, {args.steps} steps, median of {args.runs} runs; "
          f"final time {typed_end[0]:.3f}s, living {typed_end[1]}")
    report("typed", typed, args.steps)
    report("json", encoded, args.steps)
    one = statistics.median(threads_run(args.seed, args.steps, 1) for _ in range(args.runs))
    two = statistics.median(threads_run(args.seed, args.steps, 2) for _ in range(args.runs))
    print(f"typed steps only: one session {one:.3f}s, two sessions in two threads {two:.3f}s wall")


if __name__ == "__main__":
    main()
