"""Offline synthetic controls for the new ladder's arithmetic and stop gates."""
from pathlib import Path
import argparse
import json
import sys
import tempfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("harness", type=Path)
    args = parser.parse_args()
    sys.path.insert(0, str(args.harness.resolve()))
    from window import reduce_window

    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)

        def fixture(*, queue_growth=0, late=1_000_000, human=0, deferred=False, scale=1):
            lines = []
            for tick in range(0, 4501, 100):
                census = dict(tick=tick, unix=100 + tick * .007, deferred=int(deferred and tick >= 1000), living=350, occupied=350 + human, meanSegments=55., p90Segments=176, p99Segments=250, food=25000, prey=30, radius=12445)
                lines.append("CENSUS " + json.dumps(census))
                if tick:
                    # One phase spikes on even ticks, the other on odd ticks.
                    rows = [[8_000_000 if t % 2 == 0 else 2_000_000, 1_000_000 if t % 2 == 0 else 7_000_000, 1] for t in range(tick - 99, tick + 1)]
                    rows = [[int(s * scale), int(i * scale), w] for s, i, w in rows]
                    lines.append("CAPACITY " + json.dumps(dict(tick=tick, rows=rows, stages=[0] * 16)))
            clients = []
            stats = []
            for second in range(99, 134):
                lines.append("SOCKET " + json.dumps(dict(unix=second, queueBytes=(second - 99) * queue_growth, kernelSendBytes=0, players=350 + human, externalPlayers=human, writeBlocks=0, bufferFull=0, rejects=0)))
                stats.append(json.dumps(dict(unix=second, players=350 + human)))
                for worker in range(1, 7):
                    clients.append("CLIENT " + json.dumps(dict(unix=second, worker=worker, cpuFraction=.1, lateP99Nanos=late)))
            (root / "host-stream.log").write_text("\n".join(lines) + "\n")
            (root / "stats.jsonl").write_text("\n".join(stats) + "\n")
            (root / "client.log").write_text("\n".join(clients) + "\n")
            return reduce_window(root, dict(label="synthetic", bots=350, start={"unix": 100}, end={"unix": 130}))

        baseline = fixture()
        assert baseline["clientValid"] and not baseline["passes"]
        assert baseline["ticks"] == 4200
        assert baseline["worldCpuMsPerTick"]["p50"] == 9
        assert baseline["worldCpuMsPerTick"]["p99"] == 9
        assert baseline["simulationCpuMsPerTick"]["p99"] == 8
        assert baseline["socketCpuMsPerTick"]["p99"] == 7
        assert not fixture(queue_growth=100000)["clientValid"]
        assert not fixture(late=42_000_000)["clientValid"]
        assert fixture(human=1)["human"]["affected"]
        assert fixture(scale=.5)["passes"]
        deferred = fixture(scale=.5, deferred=True)
        assert deferred["deferredTicks"] == 1 and not deferred["passes"]
        print("PASS: aligned CPU sums, complete blocks, queue and cadence rejection, human occupancy, deferrals")


if __name__ == "__main__":
    main()
