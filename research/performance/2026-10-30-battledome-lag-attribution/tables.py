"""Regenerate bounded Markdown tables from selected-results.json, remotely."""
from pathlib import Path
import json
import sys


def q(value,keys=("p50","p99","max")):
    return " / ".join(f"{value[k]:.3f}" for k in keys) if value.get("n") else "Not observed"


def table(headers,rows):
    return "| "+" | ".join(headers)+" |\n| "+" | ".join("---" for _ in headers)+" |\n"+"".join(
        "| "+" | ".join(str(v) for v in row)+" |\n" for row in rows)


def render(data):
    series=data["series"]
    primary=[r for s in series for r in s["rows"] if r["kind"] in ["primary","followup-diagnostic"]]
    blocks=[]
    blocks.append(table(["Window / mode","Ticks","Start lag p50 / p99 / max ms",
        "Completion progress p99 / max ms","Comparable completion p99 / max ms","Episodes >20 ms / deferred"],
        [[r["label"]+" / "+r["mode"],r["ticks"],q(r["startLagMs"]),
          q(r["completionProgressLagMs"],("p99","max")),q(r["censusComparableCompletionRelativeMs"],("p99","max")),
          f"{r['completeEpisodes']} / {r['observedDeferredIncrement']}"] for r in primary]))
    blocks.append(table(["Window","World CPU p50 / p99 / max ms","Aligned sim CPU p50 / p99 / max ms",
        "Aligned socket CPU p50 / p99 / max ms","Aligned total CPU p50 / p99 / max ms"],
        [[r["label"],q(r["worldCPUMs"]),q(r["alignedPeriodSimulationCPUMs"]),
          q(r["alignedPeriodSocketCPUMs"]),q(r["alignedPeriodTotalHostCPUMs"])] for r in primary]))
    blocks.append(table(["Window","World / total periods >7-ms CPU","Sim runnable p99 / max ms",
        "Socket runnable p99 / max ms","No-pacing periods / longest streak","IRQ / softIRQ / steal jiffies"],
        [[r["label"],f"{r['worldCPUOverBudgetTickCount']} / {r['alignedTotalCPUOverBudgetPeriodCount']}",
          q(r["alignedPeriodSimulationRunnableWaitMs"],("p99","max")),
          q(r["alignedPeriodSocketRunnableWaitMs"],("p99","max")),
          f"{r['noPacingWaitPeriods']} / {r['longestNoPacingWaitStreak']}",
          f"{r['guestIRQJiffies']} / {r['guestSoftIRQJiffies']} / {r['guestStealJiffies']}"] for r in primary]))
    blocks.append(table(["Window","Probe residual jitter p99 / max ms","Excess p99 / max ms",
        "Residual stalls >20 / >100 / >250 ms","Bunched <1 ms / gaps","Input p99 / max ms (echoes)"],
        [[r["label"],q(r["probe"]["frameMetricsMs"]["gapResidual"],("p99","max")),
          q(r["probe"]["frameMetricsMs"]["excess"],("p99","max")),
          " / ".join(str(r["probe"]["stalls"]["gapResidual"][str(t)]) for t in [20,100,250]),
          f"{sum(v['bunchedUnder1Ms'] for v in r['clientByEpisodePhase'].values())} / {sum(v['gapMs']['n'] for v in r['clientByEpisodePhase'].values())}",
          q(r["probe"]["inputToEffectMs"],("p99","max"))+f" ({r['probe']['confirmed']})"] for r in primary]))
    blocks.append(table(["Window","Pacing overshoot p50 / p99 / max ms",
        "Wake-ready delay p50 / p99 / max ms","Wake-dispatch p50 / p99 / max ms"],
        [[r["label"],q(r["pacingWakeMs"]["overshootMs"]),q(r["pacingWakeMs"]["readyDelayMs"]),
          q(r["pacingWakeMs"]["dispatchDelayMs"])] for r in primary]))
    blocks.append(table(["Calibration series","Window","Host CPU/tick ms"],
        [[s["name"],label,f"{value:.6f}"] for s in series
         for label,value in s["calibration"]["hostCPUPerTickMs"].items()]))
    blocks.append(table(["Traced window","Socket / observer / other-host / other-guest / idle / IRQ runnable-wait ms"],
        [[r["label"]," / ".join("Not traced" if k=="irq" and r["mode"]=="sched" else
                              f"{r['traceWindowRunnableWaitPartsMs'].get(k,0):.3f}" for k in
          ["socket","observerOrController","otherHostThread","otherGuest","idleOrSteal","irq"])]
         for r in primary if r["trace"]["usable"]]))
    return "\n".join(blocks)


if __name__=="__main__":print(render(json.loads(Path(sys.argv[1]).read_text())))
