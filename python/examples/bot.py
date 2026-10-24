"""Run examples/bot.py's food-seeking controller in-process with the slither_sim package.

Install the package first (see docs/python.md). Decisions match examples/bot.py,
so both print the same summary.
"""

import argparse
import math

import slither_sim


def heading(frame, slot):
    """Steer to the nearest visible food, or back toward the center near the rim."""
    me = frame.view_self[slot]
    x, y = frame.snake_x[me], frame.snake_y[me]
    start, end = frame.view_food_start[slot], frame.view_food_start[slot + 1]
    if end > start and math.hypot(x, y) < frame.radius * 0.8:
        food_x, food_y = frame.food_x, frame.food_y
        target = min(frame.view_food[start:end], key=lambda i: (food_x[i] - x) ** 2 + (food_y[i] - y) ** 2)
        return math.atan2(food_y[target] - y, food_x[target] - x)
    return math.atan2(-y, -x)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--steps", type=int, default=200)
    args = parser.parse_args()
    if args.steps < 1:
        parser.error("--steps must be positive")

    session = slither_sim.Session()
    # One world; pass worlds=N and give N * agents actions per step for more.
    session.reset(seed=7, agents=4)
    frame = session.frame(0)
    completed = 0
    while completed < args.steps and frame.living:
        headings = [math.nan if me == slither_sim.NONE else heading(frame, slot)
                    for slot, me in enumerate(frame.view_self)]
        session.step(headings, [False] * len(headings))
        frame = session.frame(0)
        completed += 1
    print(f"decisions={completed} time={frame.time:.3f}s living={frame.living}")


if __name__ == "__main__":
    main()
