"""Causal cache/view reconstruction for the opt-in transport-view-v1 gate.

Arithmetic and eligibility follow research/fidelity/2026-10-26-sparse-view-cadence/
analyze.py (including fixed_windows) and the cadence-residual activity replay.
These are decoded workload proxies, not geometric visibility or boost commands.
"""

from collections import Counter
import math


CONTRACT = "transport-view-v1"
BINS = ("0", "1-2", "3-5", "6-10", "11-20", "21+")


def binof(peers):
    return next(label for upper, label in zip((0, 2, 5, 10, 20, math.inf), BINS) if peers <= upper)


def state(world):
    result = Counter(peers=len(world.snakes) - int(world.self_id in world.snakes))
    for sid, snake in world.snakes.items():
        group = "own" if sid == world.self_id else "peer"
        result[group + "Count"] += 1
        result[group + "Speed"] += snake.sp
        result[group + "Fast"] += snake.sp > 9
        result[group + "Boost"] += snake.sp >= 13.9
        angle = abs((snake.wang - snake.ang + math.pi) % (2 * math.pi) - math.pi)
        result[group + "Turning"] += angle > 2 * math.pi / 256
        result[group + "Segments"] += snake.sct
    return result


class ViewSamples:
    def __init__(self):
        self.bins = {label: dict(exposure=0, frames=0, state=Counter(), events=Counter(),
                                 server=[], receipt=[], windows=[]) for label in BINS}

    def merge(self, other):
        for label, incoming in other.bins.items():
            target = self.bins[label]
            for key in ("exposure", "frames"):
                target[key] += incoming[key]
            for key in ("state", "events"):
                target[key].update(incoming[key])
            for key in ("server", "receipt", "windows"):
                target[key].extend(incoming[key])

    def summary(self, quantiles):
        bins = {}
        for label, data in self.bins.items():
            seconds = data["exposure"] / 1000
            weighted, events = data["state"], data["events"]
            activity = {"meanPeers": weighted["peers"] / seconds if seconds else None}
            for group in ("own", "peer"):
                denominator = weighted[group + "Count"]
                activity[group + "SnakeSeconds"] = denominator
                for key in ("Speed", "Fast", "Boost", "Turning", "Segments"):
                    activity[group + key] = weighted[group + key] / denominator if denominator else None
                for key in ("Anchors", "Mass", "TurnChanges"):
                    activity[group + key + "PerSnakeSecond"] = events[group + key] / denominator if denominator else None
            for key in ("food", "prey", "ping"):
                activity[key + "PerSecond"] = events[key] / seconds if seconds else None
            windows = data["windows"]
            bins[label] = dict(seconds=seconds, frames=data["frames"],
                               frameGapsMs={clock: quantiles(data[clock]) for clock in ("server", "receipt")},
                               le9Fraction=sum(g <= 9 for g in data["server"]) / data["frames"] if data["frames"] else None,
                               wholeWindows=dict(seconds=len(windows), frames=sum(windows),
                                                 framesPerSecond=sum(windows) / len(windows) if windows else None,
                                                 distribution=quantiles(windows)),
                               activity=activity, eventCounts=dict(events))
        return dict(contract=CONTRACT, bins=bins)


class ViewEpoch:
    def __init__(self):
        self.active = False
        self.previous = None
        self.rows = []

    def packet(self, packet, world, events, counts):
        command = chr(packet[0])
        counts["food"] += command in "FbfwcC<"
        counts["prey"] += command in "yj"
        counts["ping"] += command == "p"  # incoming pongs, not outbound ping commands
        for event in events:
            kind = event[0]
            if kind == "game_over" or (kind == "remove" and event[2] == world.self_id):
                self.active = False
            if kind == "spawn" and event[2] == world.self_id:
                self.active = True
            if kind in ("anchor", "fam", "tail", "rot"):
                group = "own" if event[2] == world.self_id else "peer"
                counts[group + "Anchors"] += kind == "anchor"
                counts[group + "Mass"] += kind in ("fam", "tail") or (kind == "anchor" and event[5])
                if kind == "rot":
                    # Wanted-angle changes, as decoded by World; direction is unavailable.
                    counts[group + "TurnChanges"] += event[3][1] != event[4][1]

    def frame(self, frame, world, was_active, counts):
        current = dict(server=frame.server, receipt=frame.t, state=state(world))
        if self.previous and was_active and self.active:
            row = dict(self.previous, end=frame.server, endBin=binof(current["state"]["peers"]),
                       receiptGap=frame.t-self.previous["receipt"], events=counts)
            if row["end"] < row["server"]:
                raise ValueError("view server clock regressed")
            self.rows.append(row)
        self.previous = current

    def finish(self, eligible):
        result = ViewSamples()
        windows = {}
        for row in self.rows:
            if not eligible(row["receipt"], row["receipt"] + row["receiptGap"]):
                continue
            left, right = row["server"], row["end"]
            label = binof(row["state"]["peers"])
            gap = right-left
            data = result.bins[label]
            data["exposure"] += gap
            data["frames"] += 1
            data["server"].append(gap)
            data["receipt"].append(row["receiptGap"])
            data["state"].update({key: value * gap / 1000 for key, value in row["state"].items()})
            data["events"].update(row["events"])
            # Whole active seconds, including fully silent ones. Endpoint emissions
            # belong to [k*1000, (k+1)*1000), and bin transitions censor the window.
            for k in range(left // 1000, right // 1000 + 1):
                span = min(right, (k+1)*1000) - max(left, k*1000)
                if span > 0:
                    window = windows.setdefault(k, dict(covered=0, bins=set(), frames=0))
                    window["covered"] += span
                    window["bins"].add(label)
            window = windows.setdefault(right // 1000, dict(covered=0, bins=set(), frames=0))
            window["frames"] += 1
            window["bins"].add(row["endBin"])
        for window in windows.values():
            if window["covered"] == 1000 and len(window["bins"]) == 1:
                result.bins[next(iter(window["bins"]))]["windows"].append(window["frames"])
        return result
