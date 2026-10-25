"""Seeks the nearest food through `slither-sim client`. Build first: cargo build --release --locked"""
import argparse
import json
import math
import os
from pathlib import Path
import subprocess
import time


class FoodBot:
    def __init__(self):
        self.target = None

    def decide(self, frame):
        view = frame["observation"]
        head = view["self"]["head"]
        x, y = head["x"], head["y"]
        center = view["arena"]["center"]
        points = [(food["x"], food["y"]) for food in view["food"]]
        if not points or math.hypot(x - center["x"], y - center["y"]) >= view["arena"]["radius"] * 0.8:
            self.target = None
        elif self.target not in points:
            self.target = min(points, key=lambda p: math.hypot(p[0] - x, p[1] - y))
        target = self.target or (center["x"], center["y"])
        return {"heading": math.atan2(target[1] - y, target[0] - x), "boost": False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    binary = "slither-sim.exe" if os.name == "nt" else "slither-sim"
    parser.add_argument("--binary", type=Path, default=Path(__file__).resolve().parents[1] / "target/release" / binary)
    for flag in ("url", "origin", "nickname"):
        parser.add_argument(f"--{flag}")
    parser.add_argument("--seed", type=int)
    parser.add_argument("--steps", type=int, default=500)
    parser.add_argument("--lives", type=int, default=1)
    args = parser.parse_args()
    if args.steps < 1 or args.lives < 1:
        parser.error("--steps and --lives must be positive")
    if args.url is not None and args.seed is not None:
        parser.error("--seed applies to the simulator only")
    if args.url is None and (args.origin is not None or args.nickname is not None):
        parser.error("--origin and --nickname require --url")
    process = subprocess.Popen([str(args.binary.resolve()), "client"], stdin=subprocess.PIPE,
                               stdout=subprocess.PIPE, text=True, encoding="utf-8")

    def request(message):
        process.stdin.write(json.dumps(message, allow_nan=False) + "\n")
        process.stdin.flush()
        reply = json.loads(process.stdout.readline())
        if reply.get("protocol") != "slither-client-json-v1":
            raise RuntimeError("incompatible client protocol")
        if "error" in reply:
            raise RuntimeError(reply["error"])
        return reply["result"]

    try:
        if args.url is None:
            start = {"op": "simulation", "seed": 1 if args.seed is None else args.seed}
        else:
            config = {k: getattr(args, k) for k in ("url", "origin", "nickname") if getattr(args, k) is not None}
            start = {"op": "network", "config": config}
        frame = request(start)["frame"]
        bot, identity, ended = FoodBot(), None, 0
        for _ in range(args.steps):
            if frame["contract"] != "slither-client-v2":
                raise RuntimeError("incompatible observation contract")
            key = (frame["received"]["connectionGeneration"], frame["life"])
            if key != identity or frame["status"] != "playing":
                bot = FoodBot()
            identity = key
            if frame["status"] in ("dead", "disconnected"):
                ended += 1
                if ended >= args.lives:
                    break
                frame = request({"op": "reconnect"})["frame"]
                continue
            message = {"op": "step"}
            if frame["status"] == "playing" and frame["observation"] is not None:
                started = time.perf_counter()
                message["action"] = bot.decide(frame)
                message["processingSeconds"] = time.perf_counter() - started
            frame = request(message)["frame"]
        print(f"ended={ended} status={frame['status']} time={frame['time']:.3f}s")
    finally:
        process.stdin.close()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
        process.stdout.close()
    if process.returncode:
        raise RuntimeError(f"client exited with code {process.returncode}")


if __name__ == "__main__":
    main()
