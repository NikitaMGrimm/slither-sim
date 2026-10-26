"""Closed-loop Python reference controller for tools/perf_transport.py (standard library only).

Usage: python -B tools/perf_transport_client.py BINARY REQUESTS [SERVE_ARG ...]

Sends each pre-encoded request line from REQUESTS to `BINARY serve SERVE_ARG...`, waits for its
reply, parses it and runs a small decision over every living slot. The decision
is not sent; requests stay scripted. Prints one JSON line with the summed timings.
"""

import json
import subprocess
import sys
import time


# Protocol slither-sim-json-v4: reply decoding and the decision step. Another
# protocol replaces these two functions and leaves the timing loop unchanged.
PROTOCOL = "slither-sim-json-v4"


def decode_reply(line):
    reply = json.loads(line)
    if reply.get("protocol") != PROTOCOL:
        raise RuntimeError(f"unexpected protocol {reply.get('protocol')!r}")
    if "error" in reply:
        raise RuntimeError(f"serve error: {reply['error']}")
    return reply["result"]


def decide(result, checksum):
    """Fold every living slot's nearest visible food row, food count and snake count into a
    32-bit checksum, so no parsed view can be skipped. Must match the Node client."""
    living = food_seen = 0
    for frame in result["worlds"]:
        snake_x, snake_y = frame["snakes"]["x"], frame["snakes"]["y"]
        food_x, food_y = frame["food"]["x"], frame["food"]["y"]
        for view in frame["views"]:
            if view is None:
                continue
            x, y = snake_x[view["self"]], snake_y[view["self"]]
            food = view["food"]
            best, nearest = -1, float("inf")
            for row in food:
                dx, dy = food_x[row] - x, food_y[row] - y
                distance = dx * dx + dy * dy
                if distance < nearest:
                    best, nearest = row, distance
            living += 1
            food_seen += len(food)
            checksum = (checksum * 1000003 + best + 1 + 7 * len(food)
                        + 13 * len(view["snakes"])) & 0xFFFFFFFF
    return checksum, living, food_seen


def main(argv):
    if len(argv) < 2:
        raise SystemExit(__doc__)
    binary, requests_path, *serve_args = argv
    with open(requests_path, "rb") as file:
        requests = [line + b"\n" for line in file.read().splitlines()]
    process = subprocess.Popen([binary, "serve", *serve_args], stdin=subprocess.PIPE, stdout=subprocess.PIPE)
    clock = time.perf_counter_ns
    totals = {"steps": 0, "replyBytes": 0, "waitNs": 0, "parseNs": 0, "decideNs": 0,
              "living": 0, "foodSeen": 0}
    reset = None
    checksum = 0
    try:
        for index, request in enumerate(requests):
            start = clock()
            process.stdin.write(request)
            process.stdin.flush()
            line = process.stdout.readline()
            received = clock()
            if not line:
                raise RuntimeError("serve closed its output")
            result = decode_reply(line)
            parsed = clock()
            checksum, living, food_seen = decide(result, checksum)
            decided = clock()
            sample = {"replyBytes": len(line), "waitNs": received - start,
                      "parseNs": parsed - received, "decideNs": decided - parsed,
                      "living": living, "foodSeen": food_seen}
            if index == 0:
                reset = sample  # The reset reply is reported separately from steps.
                continue
            totals["steps"] += 1
            for key, value in sample.items():
                totals[key] += value
    finally:
        process.stdin.close()
        try:
            process.wait(timeout=30)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
        process.stdout.close()
    if process.returncode:
        raise SystemExit(f"serve exited with {process.returncode}")
    print(json.dumps({"client": "python", "runtime": sys.version.split()[0], "checksum": checksum,
                      "reset": reset, **totals}), flush=True)


if __name__ == "__main__":
    main(sys.argv[1:])
