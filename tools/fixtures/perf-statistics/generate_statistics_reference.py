# /// script
# requires-python = ">=3.12"
# dependencies = ["numpy==2.5.4", "scipy==1.18.1"]
# ///
"""Generate offline regression oracles; never invokes simulator, GitHub, or CI."""
import copy
import json
import math
from pathlib import Path
import platform

import numpy as np
import scipy
from scipy.linalg import lstsq
from scipy.stats import t

ROOT = Path(__file__).resolve().parent
LOOKS = [8, 12, 26]
ORDER = [1, -1, -1, 1, 1, -1, 1, -1, -1, 1, 1, -1, -1, 1,
         1, -1, -1, 1, -1, 1, 1, -1, 1, -1, -1, 1]
ETA = 1 / 600
CRITICAL = {n: float(t.isf(1 / 1200, n - 2)) for n in LOOKS}
A, B = math.log(.95), math.log(1.05)


def classify(lower, upper):
    if upper < A:
        return "Faster"
    if lower > B:
        return "Slower"
    if lower >= A and upper <= B:
        return "Within"
    return "Undecided"


def dataset(name, n, mu=0., gamma=0., noise=.01, offsets=None, scale=1.):
    # Fixed nonrandom residual pattern. OLS need not recover the nominal inputs.
    pattern = [-1.1, .7, -.4, 1.4, -.8, .2, 1.0, -1.0, .5, -.3,
               1.2, -.6, -.9, .4, -.2, 1.3, -.7, .1, .9, -1.2,
               .6, -.5, 1.1, -.1, .3, -.8]
    pairs = []
    for i in range(n):
        base = scale * (1 + .03 * ((i * 7) % 11))
        d = offsets[i] if offsets is not None else mu + gamma * ORDER[i] + noise * pattern[i]
        pairs.append({"pair_index": i + 1, "order": ORDER[i],
                      "baseline_wall_seconds": base,
                      "candidate_wall_seconds": base * math.exp(d),
                      "operations_per_block": 1000, "blocks": 1})
    return {"id": name, "pairs": pairs}


def analyze(pairs):
    n = len(pairs)
    assert n in LOOKS
    d = np.array([math.log(p["candidate_wall_seconds"]) - math.log(p["baseline_wall_seconds"])
                  for p in pairs], dtype=float)
    order = np.array([p["order"] for p in pairs], dtype=float)
    assert sum(order) == 0
    design = np.column_stack([np.ones(n), order])
    # Independent oracle: general SciPy least-squares, not the balanced fsum shortcut.
    fit, _, rank, _ = lstsq(design, d, lapack_driver="gelsd")
    assert rank == 2
    mu, gamma = map(float, fit)
    errors = d - design @ fit
    sse = float(errors @ errors)
    s = math.sqrt(sse / (n - 2))
    se = s / math.sqrt(n)
    floor = 64 * math.ulp(max(1., max(abs(float(x)) for x in d)))
    critical = CRITICAL[n]
    baseline = math.exp(math.fsum(math.log(p["baseline_wall_seconds"]) for p in pairs) / n
                        - math.log(1000) + math.log(1000))
    candidate = math.exp(math.fsum(math.log(p["candidate_wall_seconds"]) for p in pairs) / n
                         - math.log(1000) + math.log(1000))
    result = {"n": n, "df": n - 2, "mu_hat": mu, "gamma_hat": gamma,
              "sse": sse, "s": s, "se": se, "critical": critical,
              "residual_floor": floor,
              "baseline_ms_per_operation": baseline,
              "candidate_ms_per_operation": candidate,
              "change_percent": 100 * math.expm1(mu)}
    if s <= floor:
        result.update(interval_log=None, interval_percent=None, label="Undecided",
                      reason="degenerate_residuals")
    else:
        low, high = mu - critical * se, mu + critical * se
        result.update(interval_log=[low, high],
                      interval_percent=[100 * math.expm1(low), 100 * math.expm1(high)],
                      label=classify(low, high), reason="valid_interval")
    return result


vectors = [
    dataset("within_at_8_order_effect", 8, gamma=.04, noise=.004),
    dataset("faster_at_8", 8, mu=math.log(.90), gamma=.02, noise=.006),
    dataset("slower_at_8", 8, mu=math.log(1.12), gamma=-.025, noise=.008),
    dataset("undecided_at_8", 8, mu=math.log(1.02), gamma=.02, noise=.04),
    dataset("within_at_12", 12, gamma=.01, noise=.02),
    dataset("slower_at_12", 12, mu=math.log(1.12), gamma=.02, noise=.025),
    dataset("within_at_26", 26, mu=.005, gamma=.03, noise=.04),
    dataset("faster_at_26", 26, mu=math.log(.9), gamma=-.01, noise=.035),
    dataset("undecided_at_26", 26, mu=math.log(1.05), gamma=.015, noise=.10),
    dataset("order_only_degenerate_at_8", 8, mu=math.log(1.12), gamma=.03, noise=0),
    dataset("all_equal_degenerate_at_26", 26, noise=0),
    dataset("tiny_residuals_degenerate_at_8", 8, mu=.10, gamma=.02, noise=1e-16),
    dataset("mixed_sign_minus_0_01_percent", 8,
            offsets=[math.log(1.02), math.log(1.02), math.log(.9999)] + [math.log(1.02)] * 5),
    dataset("mixed_sign_plus_0_01_percent", 8,
            offsets=[math.log(1.02), math.log(1.02), math.log(1.0001)] + [math.log(1.02)] * 5),
    dataset("large_absolute_clock_scale", 12, mu=.10, gamma=.01, noise=.02, scale=1e150),
]
for item in vectors:
    item["expected"] = analyze(item["pairs"])

state_vectors = []


def state(name, item, count, termination):
    pairs = copy.deepcopy(item["pairs"][:count])
    history = []
    for n in LOOKS:
        if n > count:
            break
        outcome = analyze(pairs[:n])
        history.append(outcome)
        if outcome["label"] in ("Faster", "Slower", "Within"):
            break
    if history and history[-1]["label"] in ("Faster", "Slower", "Within"):
        # A conforming runner stops immediately. No future pairs are present.
        pairs = pairs[:history[-1]["n"]]
        termination = "resolved"
    expected = {"termination": termination, "history": history,
                "last_look_n": history[-1]["n"] if history else None,
                "n_complete": len(pairs),
                "n_used": history[-1]["n"] if history else 0,
                "label": history[-1]["label"] if history else "Unavailable",
                "reason": history[-1]["reason"] if history else "insufficient_pairs"}
    if termination == "measurement_failure":
        expected.update(label="Unavailable", reason="measurement_failure", n_used=0)
    state_vectors.append({"id": name, "pairs": pairs, "termination": termination,
                          "expected": expected})


state("resolved_at_8_no_future_pairs", dataset("_", 26, gamma=.04, noise=.004), 26, "max_pairs")
state("cap_before_first_look", dataset("_", 26, mu=.04, noise=.1), 7, "time_cap")
state("cap_after_10_keep_8", dataset("_", 26, mu=.04, noise=.1), 10, "time_cap")
state("cap_after_14_keep_12", dataset("_", 26, mu=.04, noise=.1), 14, "time_cap")
state("failure_after_10_overrides_prior_look", dataset("_", 26, mu=.04, noise=.1), 10, "measurement_failure")
state("max_26_undecided", vectors[8], 26, "max_pairs")
state("degenerate_8_continue_then_resolve_12",
      dataset("_", 12, offsets=[.10 + .02 * o for o in ORDER[:8]]
              + [.10 + .02 * ORDER[i] + e for i, e in zip(range(8, 12), [-.02, .02, .015, -.015])]),
      12, "max_pairs")
state("undecided_8_then_within_12", dataset("_", 26, noise=.03), 12, "max_pairs")

classification = []
for name, low, high in [
    ("both_boundaries_inclusive", A, B),
    ("upper_equal_fast_boundary_is_not_faster", A - .01, A),
    ("lower_equal_slow_boundary_is_not_slower", B, B + .01),
    ("one_ulp_below_fast_boundary", A - .01, math.nextafter(A, -math.inf)),
    ("one_ulp_above_slow_boundary", math.nextafter(B, math.inf), B + .01),
    ("point_negative_but_not_proven_faster", -.04, -.01),
    ("positive_but_within", .01, .04),
    ("crosses_band", -.1, .1),
]:
    classification.append({"id": name, "interval_log": [low, high],
                           "label": classify(low, high)})

# These are instructions to mutate an otherwise valid n=8 input. JSON itself
# stays standards-compliant; python_float is materialized only by a test adapter.
invalid = []
for value in [0, -1, None, "0.5", True]:
    invalid.append({"id": "invalid_time_" + str(value), "pair_index": 4,
                    "field": "candidate_wall_seconds", "value": value,
                    "expected_label": "Unavailable", "reason": "invalid_pair"})
for name in ["nan", "inf", "-inf"]:
    invalid.append({"id": "nonfinite_time_" + name, "pair_index": 4,
                    "field": "baseline_wall_seconds", "python_float": name,
                    "expected_label": "Unavailable", "reason": "invalid_pair"})
for name, field, value in [("wrong_order", "order", -1),
                           ("wrong_operation_count", "operations_per_block", 999),
                           ("wrong_block_count", "blocks", 2),
                           ("duplicate_pair_index", "pair_index", 3)]:
    invalid.append({"id": name, "pair_index": 4, "field": field, "value": value,
                    "expected_label": "Unavailable", "reason": "invalid_pair"})
invalid.extend([
    {"id": "missing_candidate_time", "pair_index": 4,
     "delete_field": "candidate_wall_seconds", "expected_label": "Unavailable", "reason": "invalid_pair"},
    {"id": "gap_in_prefix", "delete_pair_index": 4,
     "expected_label": "Unavailable", "reason": "invalid_pair"},
])

block_vectors = []
for name, baseline, candidate in [
    ("geometric_not_arithmetic", [1., 4.], [2., 4.]),
    ("four_blocks_constant_factor", [1., 2., 4., 8.], [1.5, 3., 6., 12.]),
    ("one_block_identity", [.125], [.1375]),
]:
    base_log = math.fsum(map(math.log, baseline)) / len(baseline)
    candidate_log = math.fsum(map(math.log, candidate)) / len(candidate)
    block_vectors.append({"id": name, "baseline_blocks_seconds": baseline,
                          "candidate_blocks_seconds": candidate,
                          "expected": {"baseline_wall_seconds": math.exp(base_log),
                                       "candidate_wall_seconds": math.exp(candidate_log),
                                       "pair_log_ratio": candidate_log - base_log}})

payload = {
    "schema": "slither-perf-statistics-reference-v1",
    "generator": {"python": platform.python_version(), "numpy": np.__version__,
                  "scipy": scipy.__version__, "ols": "scipy.linalg.lstsq(gelsd)",
                  "critical": "scipy.stats.t.isf(1/1200, df)",
                  "scope": "offline synthetic oracle; no simulator or CI measurements"},
    "constants": {"alpha_family": .05, "workload_slots": 10, "looks": LOOKS,
                  "eta_numerator": 1, "eta_denominator": 600,
                  "upper_tail_numerator": 1, "upper_tail_denominator": 1200,
                  "lower_log_boundary": A, "upper_log_boundary": B,
                  "critical_by_df": {str(n - 2): CRITICAL[n] for n in LOOKS},
                  "order_base_first_plus_one": ORDER,
                  "degenerate_ulp_multiplier": 64},
    "numeric_comparison": {"absolute_tolerance": 5e-12, "relative_tolerance": 2e-10,
                           "note": "Compare numeric outputs with isclose; do not compare formatted percentages. Degenerate SSE/s are diagnostic only and need not match relative error."},
    "datasets": vectors, "state_vectors": state_vectors,
    "classification_vectors": classification,
    "block_aggregation_vectors": block_vectors,
    "invalid_mutations": {"base_dataset": "within_at_8_order_effect", "cases": invalid},
}
path = ROOT / "statistics-reference.json"
path.write_text(json.dumps(payload, indent=2, allow_nan=False) + "\n", encoding="utf-8", newline="\n")
print(json.dumps({"output": str(path), "critical_by_df": payload["constants"]["critical_by_df"],
                  "datasets": {x["id"]: x["expected"]["label"] for x in vectors},
                  "states": {x["id"]: {k: x["expected"][k] for k in ("label", "last_look_n", "termination")} for x in state_vectors}}, indent=2))
