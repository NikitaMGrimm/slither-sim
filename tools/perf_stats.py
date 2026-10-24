"""Error-controlled paired timing analysis for PR comparisons; standard library only.

Implements the v2 schedule in docs/maintainers/performance-statistics-v2.md.
The model/calculations retain the frozen v1 contract's section numbering.
docs/maintainers/pr-reports.md explains
the method. SciPy-generated reference vectors live in tools/fixtures/perf-statistics/.
"""

import math


PLAN = "slither-perf-sequential-v2"
LOOKS = (8, 12, 26) + tuple(26 * 2 ** k for k in range(1, 12))
# §2 literal order sequence: +1 runs the baseline first. Every adjacent two-pair
# block is balanced, so every scheduled prefix is too.
ORDER26 = (1, -1, -1, 1, 1, -1, 1, -1, -1, 1, 1, -1, -1, 1,
           1, -1, -1, 1, -1, 1, 1, -1, 1, -1, -1, 1)
ORDER = ORDER26 * (LOOKS[-1] // 26)
WORKLOAD_SLOTS = 10
# Allocate every possible look before sampling; stopped/missing rows do not lend
# their allocation to other rows. The finite family bound is 0.0499951171875.
ETA_DENOMINATORS = (750,) * 3 + tuple(1000 * 2 ** k for k in range(1, 12))
# SciPy 1.18.1 t.isf(1/(2*eta_denominator), n-2), independently checked against
# the stdlib incomplete-beta tail. No production quantile dependency.
T_CRITICAL = {6: 5.6381610664568065, 10: 4.4008345563591851, 24: 3.6302454361287109,
              50: 3.7231413492837411, 102: 3.7958204274577012, 206: 3.91053112436507,
              414: 4.0447022702968463, 830: 4.1872769246391446, 1662: 4.3324956015448794,
              3326: 4.4773191275288378, 6654: 4.6201814720747434, 13310: 4.7603285541138822,
              26622: 4.8974519327571864, 53246: 5.0314828408503693}
# §4 practical band: log(0.95) and log(1.05) as fixed binary64 literals, equal to the
# reference vectors' boundaries. math.log is not correctly rounded everywhere: the
# Windows runtime returns log(1.05) one ulp below the correctly rounded
# 0.04879016416943205 that glibc and macOS return, so computing it would make
# boundary labels platform-dependent.
LOWER = -0.05129329438755058
UPPER = 0.04879016416943204
# §5 numerical policy, in units of ulp(max(1, max |d_i|)).
DEGENERATE_ULP_MULTIPLIER = 64
RESOLVED = ("Faster", "Slower", "Within")
FIELDS = ("pair_index", "order", "baseline_wall_seconds", "candidate_wall_seconds",
          "operations_per_block", "blocks")


def classify(low, high):
    """§4 labels from unrounded log endpoints."""
    if high < LOWER:
        return "Faster"
    if low > UPPER:
        return "Slower"
    if low >= LOWER and high <= UPPER:
        return "Within"
    return "Undecided"


def valid_pairs(pairs, operations=None, blocks=None):
    """§1/§6 input checks: contiguous 1-based indices, the literal order, equal positive
    K and m on every pair (and equal to the plan when given), finite positive times."""
    def integer(value):
        return type(value) is int

    if not pairs or len(pairs) > len(ORDER):
        return False
    first = pairs[0]
    if not isinstance(first, dict):
        return False
    operations = first.get("operations_per_block") if operations is None else operations
    blocks = first.get("blocks") if blocks is None else blocks
    if not (integer(operations) and operations > 0 and integer(blocks) and blocks > 0):
        return False
    for index, pair in enumerate(pairs):
        if not isinstance(pair, dict) or not set(FIELDS) <= pair.keys():
            return False
        if not (integer(pair["pair_index"]) and pair["pair_index"] == index + 1
                and integer(pair["order"]) and pair["order"] == ORDER[index]
                and integer(pair["operations_per_block"]) and pair["operations_per_block"] == operations
                and integer(pair["blocks"]) and pair["blocks"] == blocks):
            return False
        for key in ("baseline_wall_seconds", "candidate_wall_seconds"):
            value = pair[key]
            if type(value) not in (int, float) or not math.isfinite(value) or value <= 0:
                return False
    return True


def geometric_block_time(seconds):
    """§1 m > 1 aggregation: the geometric mean block time, not a sum or arithmetic mean."""
    return math.exp(math.fsum(map(math.log, seconds)) / len(seconds))


def analyze(pairs):
    """One scheduled look (§3-§5) over a complete, validated prefix.

    Returns the §7 look record. A nonfinite or overflowing derived value yields
    label Unavailable with reason numeric_failure instead of a claim.
    """
    n = len(pairs)
    if n not in LOOKS or not valid_pairs(pairs) or sum(p["order"] for p in pairs) != 0:
        raise ValueError(f"analysis needs a complete, balanced, valid prefix of {', '.join(map(str, LOOKS))} pairs")
    operations = pairs[0]["operations_per_block"]
    order = [p["order"] for p in pairs]
    # Subtract logs rather than forming a ratio that could overflow.
    d = [math.log(p["candidate_wall_seconds"]) - math.log(p["baseline_wall_seconds"]) for p in pairs]
    # Balanced +/-1 design: these are the least-squares intercept and order effect.
    mu = math.fsum(d) / n
    gamma = math.fsum(o * x for o, x in zip(order, d)) / n
    sse = math.fsum((x - mu - gamma * o) ** 2 for x, o in zip(d, order))
    df = n - 2
    s = math.sqrt(sse / df)  # residual SD after the order term, not the SD of d
    se = s / math.sqrt(n)
    critical = T_CRITICAL[df]
    floor = DEGENERATE_ULP_MULTIPLIER * math.ulp(max(1.0, max(map(abs, d))))
    result = {"n": n, "df": df, "mu_hat": mu, "gamma_hat": gamma, "sse": sse, "s": s, "se": se,
              "critical": critical, "residual_floor": floor}
    try:
        # §4: geometric means over the same prefix; their ratio is exp(mu_hat).
        for label in ("baseline", "candidate"):
            mean_log = math.fsum(math.log(p[f"{label}_wall_seconds"]) for p in pairs) / n
            result[f"{label}_ms_per_operation"] = math.exp(math.log(1000) + mean_log - math.log(operations))
        result["change_percent"] = 100 * math.expm1(mu)
        if s <= floor:
            # §5: withhold the interval and any claim; never a zero-width interval.
            result.update(interval_log=None, interval_percent=None, label="Undecided",
                          reason="degenerate_residuals")
        else:
            low, high = mu - critical * se, mu + critical * se
            result.update(interval_log=[low, high],
                          interval_percent=[100 * math.expm1(low), 100 * math.expm1(high)],
                          label=classify(low, high), reason="valid_interval")
    except OverflowError:
        result["label"], result["reason"] = "Unavailable", "numeric_failure"
        return result
    if not all(math.isfinite(value) for value in _numbers(result)):
        result["label"], result["reason"] = "Unavailable", "numeric_failure"
    return result


def _numbers(value):
    if isinstance(value, dict):
        for item in value.values():
            yield from _numbers(item)
    elif isinstance(value, list):
        for item in value:
            yield from _numbers(item)
    elif isinstance(value, float):
        yield value


def final(history, n_complete, termination, failure=None):
    """§6/§7 final row from the looks actually evaluated.

    A measurement failure suppresses any earlier look (it stays in history only).
    Otherwise the last completed look is the result; with none, the row is
    Unavailable for insufficient pairs.
    """
    last = history[-1] if history else None
    row = {"termination": termination, "history": history, "n_complete": n_complete,
           "last_look_n": last["n"] if last else None}
    if termination == "measurement_failure" or (last and last["label"] == "Unavailable"):
        reason = last["reason"] if last and last["label"] == "Unavailable" else "measurement_failure"
        return {**row, "termination": "measurement_failure", "n_used": 0, "label": "Unavailable",
                "reason": reason, "failure": failure, "result": None}
    if last is None:
        return {**row, "n_used": 0, "label": "Unavailable", "reason": "insufficient_pairs",
                "failure": failure, "result": None}
    return {**row, "n_used": last["n"], "label": last["label"], "reason": last["reason"],
            "failure": failure, "result": last}
