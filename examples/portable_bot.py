"""Run one food-seeking bot against the simulator or an explicit WebSocket server.

Launches `slither-sim client` and speaks slither-client-json-v1 over its stdio.
The same bot drives either backend. Lost lives are reconnected only explicitly,
up to --lives lives in total.
"""

import argparse
import json
import math
import os
from pathlib import Path
import subprocess
import sys
import time

PROTOCOL = "slither-client-json-v1"
CONTRACT = "slither-client-v2"
ENDED = ("dead", "disconnected")


class FoodBot:
    """Chase one visible pellet until it disappears; turn back near the boundary."""

    def __init__(self):
        self.reset()

    def reset(self):
        self.target = None

    def decide(self, frame):
        observation = frame["observation"]
        if observation is None:
            return None
        head = observation["self"]["head"]
        x, y = head["x"], head["y"]
        center = observation["arena"]["center"]
        food = observation["food"]
        if not food or math.hypot(x - center["x"], y - center["y"]) >= observation["arena"]["radius"] * 0.8:
            self.target = None
            target = (center["x"], center["y"])
        else:
            points = [(item["x"], item["y"]) for item in food]
            if self.target not in points:
                self.target = min(points, key=lambda point: math.hypot(point[0] - x, point[1] - y))
            target = self.target
        return {"heading": math.atan2(target[1] - y, target[0] - x), "boost": False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    root = Path(__file__).resolve().parents[1]
    filename = "slither-sim.exe" if os.name == "nt" else "slither-sim"
    parser.add_argument("--binary", type=Path, default=root / "target" / "release" / filename)
    parser.add_argument("--url", help="explicit ws:// or wss:// server URL; omit for the simulator")
    parser.add_argument("--origin", help="WebSocket Origin header for --url")
    parser.add_argument("--nickname", help="nickname for --url")
    parser.add_argument("--seed", type=int, help="simulator seed")
    parser.add_argument("--steps", type=int, default=500)
    parser.add_argument("--lives", type=int, default=1)
    args = parser.parse_args()
    if args.steps < 1 or args.lives < 1:
        parser.error("--steps and --lives must be positive")
    if args.url is not None:
        if not args.url.startswith(("ws://", "wss://")):
            parser.error("--url must be an explicit ws:// or wss:// URL")
        if args.seed is not None:
            parser.error("--seed applies to the simulator only")
    elif args.origin is not None or args.nickname is not None:
        parser.error("--origin and --nickname require --url")
    if not args.binary.is_file():
        parser.error("build the binary with cargo build --release --locked, or pass --binary")

    process = subprocess.Popen(
        [str(args.binary.resolve()), "client"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        text=True,
        encoding="utf-8",
    )

    def request(message):
        process.stdin.write(json.dumps(message, allow_nan=False) + "\n")
        process.stdin.flush()
        line = process.stdout.readline()
        if not line:
            raise RuntimeError("client process closed its output")
        reply = json.loads(line)
        if reply.get("protocol") != PROTOCOL or ("result" in reply) == ("error" in reply):
            raise RuntimeError("incompatible client protocol reply")
        if "error" in reply:
            raise RuntimeError(reply["error"])
        return reply["result"]

    def frame_of(result):
        frame = result["frame"]
        if frame.get("contract") != CONTRACT:
            raise RuntimeError("incompatible bot observation contract")
        return frame

    try:
        if request({"op": "info"}).get("contract") != CONTRACT:
            raise RuntimeError("incompatible bot observation contract")
        if args.url is None:
            start = {"op": "simulation", "seed": 1 if args.seed is None else args.seed}
        else:
            config = {"url": args.url}
            if args.origin is not None:
                config["origin"] = args.origin
            if args.nickname is not None:
                config["nickname"] = args.nickname
            start = {"op": "network", "config": config}
        frame = frame_of(request(start))

        bot = FoodBot()
        identity = status = last_ended = None
        lives = decisions = 0
        for _ in range(args.steps):
            # Mirror BotRunner: new connection, new life or leaving play resets state.
            key = (frame["received"]["connectionGeneration"], frame["life"])
            if key != identity or (status == "playing" and frame["status"] != "playing"):
                bot.reset()
            identity, status = key, frame["status"]
            if status in ENDED:
                if key != last_ended:
                    lives += 1
                    last_ended = key
                if lives >= args.lives:
                    break
                print(f"life ended ({status}); reconnecting", file=sys.stderr)
                frame = frame_of(request({"op": "reconnect"}))
                continue
            message = {"op": "step"}
            if status == "playing":
                started = time.perf_counter()
                action = bot.decide(frame)
                elapsed = time.perf_counter() - started
                message["processingSeconds"] = elapsed
                if action is not None:
                    message["action"] = action
                    decisions += 1
            frame = frame_of(request(message))
        print(f"decisions={decisions} lives={lives} status={frame['status']} time={frame['time']:.3f}s")
    except (OSError, RuntimeError, KeyError, TypeError, ValueError) as error:
        print(error, file=sys.stderr)
        return 1
    finally:
        process.stdin.close()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
        process.stdout.close()
    return 1 if process.returncode else 0


if __name__ == "__main__":
    raise SystemExit(main())
