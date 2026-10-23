"""Permutation tests and Holm correction for predeclared study summaries.

The unit of analysis is one scalar summary per independent study or capture
batch, never individual packets: packets within a capture are serially
dependent and are not exchangeable. A permutation test is valid only when the
units are independent and exchangeable under the null hypothesis, and only for
strata and comparisons declared before the data were seen. A p-value measures
surprise under that null; no statistical result here proves a physics rule or
establishes equivalence between simulator and server.
"""

import itertools
import math
import random

MAX_RESAMPLES = 65535
MAX_COHORT = 128
MAX_FAMILY = 32
# Distinct relabelings whose statistics differ by less than this many ulps of
# the largest input are counted as ties with the observed statistic. Rounding
# in the means is a few ulps; counting near-ties as extreme only raises p.
TIE_ULPS = 1024


def _real(value, name):
    if isinstance(value, bool) or not isinstance(value, (int, float)):
        raise TypeError(f"{name} must be a real number")
    try:
        number = float(value)
    except OverflowError:
        raise ValueError(f"{name} is too large for a float") from None
    if not math.isfinite(number):
        raise ValueError(f"{name} must be finite")
    return number


def _group(values, name):
    if not isinstance(values, (list, tuple)):
        raise TypeError(f"{name} must be a list or tuple")
    if not values:
        raise ValueError(f"{name} must not be empty")
    if len(values) > MAX_COHORT:
        raise ValueError(f"{name} has more than {MAX_COHORT} values")
    return [_real(value, f"{name}[{index}]") for index, value in enumerate(values)]


def _count(value, name, low, high):
    if isinstance(value, bool) or not isinstance(value, int):
        raise TypeError(f"{name} must be an integer")
    if not low <= value <= high:
        raise ValueError(f"{name} must be between {low} and {high}")
    return value


def _mean(values):
    try:
        total = math.fsum(values)
    except OverflowError:
        raise ValueError("group sum overflows") from None
    mean = total / len(values)
    if not math.isfinite(mean):
        raise ValueError("group mean is not finite")
    return mean


def _means(pooled, chosen):
    """Return (baseline mean, candidate mean) when `chosen` indexes the baseline."""
    selected = set(chosen)
    baseline = _mean([pooled[index] for index in chosen])
    candidate = _mean([value for index, value in enumerate(pooled) if index not in selected])
    if not math.isfinite(candidate - baseline):
        raise ValueError("difference of means is not finite")
    return baseline, candidate


def permutation_test(baseline, candidate, *, resamples=4095, seed=0):
    """Two-sided permutation test on the absolute difference of group means.

    `baseline` and `candidate` hold one finite scalar summary per independent,
    exchangeable study or batch unit, drawn from a predeclared stratum. Labels
    are permuted with group sizes fixed. When every relabeling fits within
    `resamples`, all of them are enumerated and the p-value is exact.
    Otherwise `resamples` uniformly random relabelings are drawn with
    replacement from `random.Random(seed)` and p = (extreme + 1) /
    (resamples + 1), which is never zero. Relabelings tying the observed
    statistic count as extreme.

    Returns the signed difference candidateMean - baselineMean, both means,
    group sizes, the p-value, the method and the number of relabelings
    evaluated. The p-value does not prove a mechanism or equivalence.
    """
    baseline = _group(baseline, "baseline")
    candidate = _group(candidate, "candidate")
    resamples = _count(resamples, "resamples", 1, MAX_RESAMPLES)
    if isinstance(seed, bool) or not isinstance(seed, int):
        raise TypeError("seed must be an integer")
    pooled = baseline + candidate
    total = len(pooled)
    if total > MAX_COHORT:
        raise ValueError(f"cohort has more than {MAX_COHORT} values")
    size = len(baseline)

    baseline_mean, candidate_mean = _means(pooled, range(size))
    observed = abs(candidate_mean - baseline_mean)
    threshold = observed - TIE_ULPS * math.ulp(max(abs(value) for value in pooled))

    def extreme(chosen):
        low, high = _means(pooled, chosen)
        return abs(high - low) >= threshold

    relabelings = math.comb(total, size)
    if relabelings <= resamples:
        method = "exact"
        permutations = relabelings
        hits = sum(extreme(chosen) for chosen in itertools.combinations(range(total), size))
        p_value = hits / relabelings
    else:
        method = "monte-carlo"
        permutations = resamples
        generator = random.Random(seed)
        hits = sum(extreme(generator.sample(range(total), size)) for _ in range(resamples))
        p_value = (hits + 1) / (resamples + 1)

    return {
        "difference": candidate_mean - baseline_mean,
        "baselineMean": baseline_mean,
        "candidateMean": candidate_mean,
        "baselineCount": size,
        "candidateCount": len(candidate),
        "pValue": p_value,
        "method": method,
        "permutations": permutations,
    }


def holm_adjust(p_values):
    """Holm step-down adjusted p-values, returned in the input order.

    The family must be the predeclared set of comparisons, at most 32 of them.
    Holm controls the family-wise error rate under any dependence, but each
    input p-value must itself come from a valid test of independent units.
    """
    if not isinstance(p_values, (list, tuple)):
        raise TypeError("p_values must be a list or tuple")
    if not p_values:
        raise ValueError("p_values must not be empty")
    if len(p_values) > MAX_FAMILY:
        raise ValueError(f"p_values has more than {MAX_FAMILY} entries")
    values = [_real(value, f"p_values[{index}]") for index, value in enumerate(p_values)]
    if any(not 0.0 <= value <= 1.0 for value in values):
        raise ValueError("p_values must lie in [0, 1]")

    count = len(values)
    adjusted = [0.0] * count
    running = 0.0
    for rank, index in enumerate(sorted(range(count), key=values.__getitem__)):
        running = max(running, min(1.0, (count - rank) * values[index]))
        adjusted[index] = running
    return adjusted
