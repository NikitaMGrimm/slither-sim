"""Render descriptive tables from validated selected data; no simulation.

python3 report_tables.py results.json > tables.md
"""
import json
from pathlib import Path
import sys

data = json.loads(Path(sys.argv[1]).read_text())
cases = data["cases"]
names = {"500-common": "500", "1000-common": "1,000", "2000-common": "2,000",
         "5000-capped": "5,000 cap", "5000-uncapped": "5,000 heuristic", "10000-capped": "10,000 cap"}


def number(value, digits=3):
    return f"{value:,.{digits}f}"


def million(value, digits=3):
    return number(value / 1e6, digits)


def median(case, field):
    return cases[case][field]["median"]


def table(title, headers, rows):
    print(f"\n### {title}\n")
    print("| " + " | ".join(headers) + " |")
    print("| " + " | ".join(["---"] * len(headers)) + " |")
    for row in rows:
        print("| " + " | ".join(map(str, row)) + " |")


table("Scaling", ["Requested / arena", "Radius", "Density x", "Step M/tick, n=5", "Original admission M/tick [n]",
                  "Original combined M/tick", "Step k/snake"], [
    [name, number(cases[key]["warmup"]["end"]["radius"], 2), number(cases[key]["densityRelativeToHeuristic"]),
     million(median(key, "stepInstructionsPerTick")),
     f'{million(median(key, "spawnInstructionsPerTick"))} [{cases[key]["originalAdmissionReferenceCount"]}]',
     million(median(key, "totalInstructionsPerTick")), number(cases[key]["stepInstructionsPerSimulatedSnake"]/1000)]
    for key, name in names.items()])
print("| 10,000 heuristic | 67,000.00 | 1.000 | Unsettled | Unsettled | Unsettled | Unsettled |")

states = []
for key, name in names.items():
    c = cases[key]
    r = c["runsIncludingWarmup"][1]
    end = r["stats"]["end"]
    states.append([name, f'{number(c["simulatedMean"])} ({c["simulatedMin"]})',
        f'{number(c["aliveMean"])} ({c["aliveMin"]})', number(c["deathsPerWorldSecond"]), r["refusals"],
        number(end["meanSegments"], 2), "/".join(str(end[k]) for k in ["p50","p90","p99","max"]),
        f'{end["food"]:,} / {end["prey"]}'])
table("Population and endpoint lengths", ["Case", "Step entry mean (min)", "End tick mean (min)", "Deaths/s",
      "Refusals", "Mean segments", "p50/p90/p99/max", "Food / prey"], states)

table("Warmup", ["Case", "Ticks", "Length screen", "Mean ratio", "p90 ratio", "Food-stock ratio"], [
    [name, f'{cases[key]["warmup"]["ticks"]:,}', "pass" if cases[key]["warmup"]["settled"] else "UNSETTLED",
     *[number(cases[key]["lastThreeWarmupBlockRatios"][field])
       for field in ["meanSegments", "p90Mean", "foodCountAtBlockEnd"]]] for key,name in names.items()])

phases = [("Movement, preparation, boost", "movementAndBoost"), ("Collision broad-phase build", "collisionBuild"),
    ("Collision queries / bounds / exact loop", "collisionQueryAndTests"), ("Death resolution / food", "deathFood"),
    ("Pickup / food cache", "pickupAndFoodCache"), ("Prey", "prey"),
    ("Ambient refill / pellet admission", "ambientRefillAndAdmission"), ("Rest of step", "rest")]
rows = [[label, *[million(cases[key]["profileInstructionsPerTick"][field]) for key in names]] for label,field in phases]
rows.append(["World step, diagnostic total", *[million(cases[key]["profileOuterInstructionsPerTick"]) for key in names]])
rows.append(["Original admission, separate reference", *[million(median(key,"spawnInstructionsPerTick")) for key in names]])
table("Coarse phase M/tick", ["Phase", *names.values()], rows)
table("Nested sampled estimates M/tick", ["Estimate", *names.values()], [
    [label, *[million(cases[key]["profileInstructionsPerTick"][field]) for key in names]]
    for label,field in [("Collision query", "sampledCollisionQueryEstimate"),
                        ("Candidate / bounds / exact loop", "sampledCandidateTestsEstimate"),
                        ("Boost accounting / emission", "sampledBoostEmissionEstimate")]])

census = []
for key,name in names.items():
    c = cases[key]
    observed = c["continuationCandidateCensus"]
    births = c["runsIncludingWarmup"][1]["births"]
    census.append([name, number(c["admissionOracle"]["candidateCensus"]["meanCandidatesPerPose"]),
        number(observed["meanCandidatesPerAdmission"]) if observed["status"] == "complete" else "unavailable",
        observed.get("admissions", "unavailable"), million(median(key,"spawnInstructionsPerTick")*1000/births) if births else "none"])
table("Admission search", ["Case", "50-pose candidates/pose", "1,100-tick candidates/life", "1,100-tick admissions",
      "Original phase M/life in 1,000 ticks"], census)

table("Necessary instruction rates for 7 ms", ["Case", "World step G instructions/s", "Original combined G instructions/s"], [
    [names[key], number(median(key,"stepInstructionsPerTick")/7e6), number(median(key,"totalInstructionsPerTick")/7e6)]
    for key in ["2000-common","5000-capped","5000-uncapped","10000-capped"]])

table("Repeat and instrumentation checks", ["Case", "Physics step min-max M/tick", "Original admission min-max M/tick [n]",
      "Prepared/original step ratio", "Diagnostic/plain step ratio"], [
    [name, "–".join(million(cases[key]["stepInstructionsPerTick"][k],6) for k in ["min","max"]),
     "–".join(million(cases[key]["spawnInstructionsPerTick"][k],6) for k in ["min","max"])
         + f' [{cases[key]["originalAdmissionReferenceCount"]}]',
     number(cases[key]["preparedStepRelativeToOriginal"],6), number(cases[key]["profileStepRelativeToPlain"],6)]
    for key,name in names.items()])
