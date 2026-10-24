"""Checks perf_stats against the SciPy-generated reference vectors, without SciPy."""

import copy
import json
import math
from pathlib import Path
import unittest

import perf_stats


REFERENCE = json.loads((Path(__file__).parent / "fixtures/perf-statistics/statistics-reference.json")
                       .read_text(encoding="utf-8"))


def beta_continued_fraction(a, b, x):
    """Lentz's continued fraction for the regularized incomplete beta function."""
    tiny = 1e-300
    c, d = 1.0, 1.0 - (a + b) * x / (a + 1.0)
    d = 1.0 / (d if abs(d) >= tiny else tiny)
    h = d
    for m in range(1, 10001):
        for aa in (m * (b - m) * x / ((a + 2 * m - 1) * (a + 2 * m)),
                   -(a + m) * (a + b + m) * x / ((a + 2 * m) * (a + 2 * m + 1))):
            d = 1.0 + aa * d
            d = 1.0 / (d if abs(d) >= tiny else tiny)
            c = 1.0 + aa / c
            c = c if abs(c) >= tiny else tiny
            h *= d * c
        if abs(d * c - 1.0) <= 4 * math.ulp(1.0):
            return h
    raise ArithmeticError("incomplete beta did not converge")


def student_upper_tail(critical, df):
    """P(T > critical) = I_x(df/2, 1/2) / 2 with x = df/(df+t^2), computed directly
    rather than as one minus a CDF near one."""
    a, b, x = df / 2, 0.5, df / (df + critical * critical)
    factor = math.exp(math.lgamma(a + b) - math.lgamma(a) - math.lgamma(b) + a * math.log(x) + b * math.log1p(-x))
    if x < (a + 1) / (a + b + 2):
        return 0.5 * factor * beta_continued_fraction(a, b, x) / a
    return 0.5 * (1 - factor * beta_continued_fraction(b, a, 1 - x) / b)


class Statistics(unittest.TestCase):
    tolerance = REFERENCE["numeric_comparison"]

    def assertMatches(self, actual, expected, path=""):
        if isinstance(expected, dict):
            for key, value in expected.items():
                self.assertMatches(actual[key], value, f"{path}/{key}")
        elif isinstance(expected, list):
            self.assertEqual(len(actual), len(expected), path)
            for index, (a, e) in enumerate(zip(actual, expected)):
                self.assertMatches(a, e, f"{path}/{index}")
        elif type(expected) in (int, float):
            self.assertTrue(math.isclose(actual, expected, rel_tol=self.tolerance["relative_tolerance"],
                                         abs_tol=self.tolerance["absolute_tolerance"]), (path, actual, expected))
        else:
            self.assertEqual(actual, expected, path)

    def test_constants_match_the_contract_and_the_incomplete_beta_tail(self):
        constants = REFERENCE["constants"]
        self.assertEqual(perf_stats.LOOKS, tuple(constants["looks"]))
        self.assertEqual(perf_stats.ORDER, tuple(constants["order_base_first_plus_one"]))
        self.assertEqual(perf_stats.WORKLOAD_SLOTS * len(perf_stats.LOOKS), constants["eta_denominator"] // 20)
        self.assertEqual((perf_stats.LOWER, perf_stats.UPPER),
                         (constants["lower_log_boundary"], constants["upper_log_boundary"]))
        self.assertEqual(perf_stats.DEGENERATE_ULP_MULTIPLIER, constants["degenerate_ulp_multiplier"])
        self.assertEqual({str(df): value for df, value in perf_stats.T_CRITICAL.items()}, constants["critical_by_df"])
        self.assertEqual(sorted(perf_stats.T_CRITICAL), [n - 2 for n in perf_stats.LOOKS])
        for df, critical in perf_stats.T_CRITICAL.items():
            with self.subTest(df=df):
                self.assertTrue(math.isclose(student_upper_tail(critical, df), 1 / 1200, rel_tol=2e-10, abs_tol=1e-15))
        for n in perf_stats.LOOKS:
            self.assertEqual(sum(perf_stats.ORDER[:n]), 0)
        for start in range(0, len(perf_stats.ORDER), 2):
            self.assertEqual(sum(perf_stats.ORDER[start:start + 2]), 0)

    def test_regression_datasets_match_the_scipy_oracle(self):
        for item in REFERENCE["datasets"]:
            with self.subTest(item["id"]):
                self.assertTrue(perf_stats.valid_pairs(item["pairs"]))
                self.assertMatches(perf_stats.analyze(item["pairs"]), item["expected"], item["id"])

    def test_boundaries_are_strict_for_claims_and_inclusive_for_within(self):
        for item in REFERENCE["classification_vectors"]:
            with self.subTest(item["id"]):
                self.assertEqual(perf_stats.classify(*item["interval_log"]), item["label"])

    def test_invalid_inputs_are_rejected(self):
        cases = REFERENCE["invalid_mutations"]
        template = next(x["pairs"] for x in REFERENCE["datasets"] if x["id"] == cases["base_dataset"])
        self.assertTrue(perf_stats.valid_pairs(template, operations=1000, blocks=1))
        for item in cases["cases"]:
            with self.subTest(item["id"]):
                pairs = copy.deepcopy(template)
                if "delete_pair_index" in item:
                    del pairs[item["delete_pair_index"] - 1]
                elif "delete_field" in item:
                    del pairs[item["pair_index"] - 1][item["delete_field"]]
                else:
                    pairs[item["pair_index"] - 1][item["field"]] = (
                        float(item["python_float"]) if "python_float" in item else item["value"])
                self.assertFalse(perf_stats.valid_pairs(pairs))
                with self.assertRaises(ValueError):
                    perf_stats.analyze(pairs)
        self.assertFalse(perf_stats.valid_pairs(template, operations=999))
        self.assertFalse(perf_stats.valid_pairs(template, blocks=2))
        with self.assertRaises(ValueError):
            perf_stats.analyze(template[:7])

    def test_block_aggregation_is_geometric(self):
        for item in REFERENCE["block_aggregation_vectors"]:
            with self.subTest(item["id"]):
                baseline = perf_stats.geometric_block_time(item["baseline_blocks_seconds"])
                candidate = perf_stats.geometric_block_time(item["candidate_blocks_seconds"])
                self.assertMatches({"baseline_wall_seconds": baseline, "candidate_wall_seconds": candidate,
                                    "pair_log_ratio": math.log(candidate) - math.log(baseline)},
                                   item["expected"], item["id"])

    def test_stopping_histories_and_final_rows(self):
        for item in REFERENCE["state_vectors"]:
            with self.subTest(item["id"]):
                pairs, history = item["pairs"], []
                for n in perf_stats.LOOKS:
                    if n > len(pairs):
                        break
                    history.append(perf_stats.analyze(pairs[:n]))
                    if history[-1]["label"] in perf_stats.RESOLVED:
                        self.assertEqual(n, len(pairs), "sampling continued after resolution")
                        break
                row = perf_stats.final(history, len(pairs), item["termination"])
                self.assertMatches(row, item["expected"], item["id"])

    def test_numeric_failure_withholds_the_claim(self):
        pairs = copy.deepcopy(REFERENCE["datasets"][0]["pairs"])
        for pair in pairs:
            pair["baseline_wall_seconds"] = 1e-300
            pair["candidate_wall_seconds"] = 1e300
        result = perf_stats.analyze(pairs)
        self.assertEqual((result["label"], result["reason"]), ("Unavailable", "numeric_failure"))
        row = perf_stats.final([result], 8, "resolved")
        self.assertEqual((row["label"], row["reason"], row["n_used"], row["termination"]),
                         ("Unavailable", "numeric_failure", 0, "measurement_failure"))
        self.assertIsNone(row["result"])


if __name__ == "__main__":
    unittest.main()
