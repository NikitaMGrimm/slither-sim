import math
import unittest

from evidence_statistics import MAX_COHORT, MAX_RESAMPLES, holm_adjust, permutation_test


class PermutationTestTests(unittest.TestCase):
    def test_exact_two_by_two(self):
        # Baseline subsets of {1,2,3,4}: |diff| = 2,1,0,0,1,2, so 2 of 6 reach 2.
        result = permutation_test([1, 2], [3, 4])
        self.assertEqual(
            result,
            {
                "difference": 2.0,
                "baselineMean": 1.5,
                "candidateMean": 3.5,
                "baselineCount": 2,
                "candidateCount": 2,
                "pValue": 2 / 6,
                "method": "exact",
                "permutations": 6,
            },
        )

    def test_exact_complete_separation(self):
        # Only the observed labeling and its mirror reach |diff| = 1 among C(6,3) = 20.
        result = permutation_test([0, 0, 0], [1, 1, 1])
        self.assertEqual(result["permutations"], 20)
        self.assertEqual(result["pValue"], 2 / 20)

    def test_exact_unequal_sizes(self):
        # Baseline {0}: 1.5, {1}: 0, {2}: |0.5 - 2| = 1.5.
        result = permutation_test([0], [1, 2])
        self.assertEqual(result["pValue"], 2 / 3)
        self.assertEqual((result["baselineCount"], result["candidateCount"]), (1, 2))
        self.assertEqual(result["difference"], 1.5)

    def test_identical_cohorts_give_one(self):
        result = permutation_test([1.0, 2.0, 3.0], [1.0, 2.0, 3.0])
        self.assertEqual(result["difference"], 0.0)
        self.assertEqual(result["pValue"], 1.0)

    def test_reversed_cohorts_negate_difference_only(self):
        forward = permutation_test([0.1, 0.5, 0.2, 0.7], [0.9, 1.3, 0.8])
        backward = permutation_test([0.9, 1.3, 0.8], [0.1, 0.5, 0.2, 0.7])
        self.assertEqual(backward["difference"], -forward["difference"])
        self.assertEqual(backward["pValue"], forward["pValue"])
        self.assertEqual(backward["permutations"], forward["permutations"])

    def test_decimal_rounding_ties_are_counted(self):
        # |diff| = |2.75 - 5 S / 6| for baseline sum S, so S >= 4.2 or S <= 2.4 is extreme:
        # sums 4.2, 4.4 and the exact tie 1.1 + 0.0 + 1.3 = 2.4, which rounds about an
        # ulp below the observed 0.75 in binary. Without the tolerance p would be 2 / 10.
        self.assertEqual(permutation_test([1.5, 1.1, 1.6], [0.0, 1.3])["pValue"], 3 / 10)

    def test_shift_and_scale_preserve_p_value(self):
        baseline = [0.3, 1.1, 0.7, 0.2]
        candidate = [1.4, 0.9, 2.0, 1.6]
        reference = permutation_test(baseline, candidate)["pValue"]
        for shift, scale in ((1e6 + 0.1, 1.0), (-37.25, 3.0), (0.0, 1e-9)):
            moved = permutation_test(
                [value * scale + shift for value in baseline],
                [value * scale + shift for value in candidate],
            )
            self.assertEqual(moved["pValue"], reference, (shift, scale))

    def test_monte_carlo_is_seeded_and_never_zero(self):
        baseline = [float(value) for value in range(10)]
        candidate = [value + 100.0 for value in baseline]
        first = permutation_test(baseline, candidate, seed=7)
        self.assertEqual(first, permutation_test(baseline, candidate, seed=7))
        self.assertEqual(first["method"], "monte-carlo")
        self.assertEqual(first["permutations"], 4095)
        self.assertGreaterEqual(first["pValue"], 1 / 4096)
        hits = first["pValue"] * 4096 - 1
        self.assertAlmostEqual(hits, round(hits))

    def test_monte_carlo_minimum_is_one_over_resamples_plus_one(self):
        # C(8,4) = 70 exceeds 5 resamples; p takes values (hits + 1) / 6.
        result = permutation_test([0, 0, 0, 0], [1, 1, 1, 1], resamples=5, seed=3)
        self.assertEqual(result["method"], "monte-carlo")
        self.assertGreater(result["pValue"], 0.0)
        self.assertIn(result["pValue"], [hits / 6 for hits in range(1, 7)])

    def test_monte_carlo_seeds_differ_and_approximate_exact(self):
        baseline = [0.2, 1.4, 0.9, 0.5, 1.1, 0.3]
        candidate = [1.0, 1.7, 0.8, 1.9, 1.3, 1.2]
        exact = permutation_test(baseline, candidate)
        self.assertEqual((exact["method"], exact["permutations"]), ("exact", 924))
        estimates = [
            permutation_test(baseline, candidate, resamples=900, seed=seed) for seed in range(3)
        ]
        self.assertTrue(all(estimate["method"] == "monte-carlo" for estimate in estimates))
        self.assertGreater(len({estimate["pValue"] for estimate in estimates}), 1)
        for estimate in estimates:
            self.assertLess(abs(estimate["pValue"] - exact["pValue"]), 0.05)

    def test_exact_boundary_uses_resample_budget(self):
        self.assertEqual(permutation_test([1, 2], [3, 4], resamples=6)["method"], "exact")
        self.assertEqual(permutation_test([1, 2], [3, 4], resamples=5)["method"], "monte-carlo")

    def test_rejects_invalid_values(self):
        for bad in (True, "1", None, [1.0], complex(1, 0)):
            with self.assertRaises(TypeError):
                permutation_test([bad, 1.0], [2.0])
        for bad in (math.nan, math.inf, -math.inf, 10**400):
            with self.assertRaises(ValueError):
                permutation_test([1.0], [bad, 2.0])

    def test_rejects_invalid_groups(self):
        with self.assertRaises(ValueError):
            permutation_test([], [1.0])
        with self.assertRaises(ValueError):
            permutation_test([1.0], [])
        for bad in (iter([1.0]), {1.0}, "12", 1.0):
            with self.assertRaises(TypeError):
                permutation_test(bad, [1.0])
        with self.assertRaises(ValueError):
            permutation_test([0.0] * (MAX_COHORT + 1), [1.0])
        with self.assertRaises(ValueError):
            permutation_test([0.0] * 64, [1.0] * (MAX_COHORT - 63))
        # The largest cohort; one random relabeling misses complete separation.
        result = permutation_test([0.0] * 64, [1.0] * 64, resamples=1)
        self.assertEqual(result["pValue"], 1 / 2)

    def test_rejects_invalid_resamples_and_seed(self):
        for bad in (0, -1, MAX_RESAMPLES + 1):
            with self.assertRaises(ValueError):
                permutation_test([1.0], [2.0], resamples=bad)
        for bad in (True, 10.0, "5", None):
            with self.assertRaises(TypeError):
                permutation_test([1.0], [2.0], resamples=bad)
        for bad in (True, 1.5, "0", None):
            with self.assertRaises(TypeError):
                permutation_test([1.0], [2.0], seed=bad)

    def test_rejects_nonfinite_arithmetic(self):
        with self.assertRaises(ValueError):
            permutation_test([1e308, 1e308], [0.0])
        with self.assertRaises(ValueError):
            permutation_test([1.7e308], [-1.7e308])


class HolmTests(unittest.TestCase):
    def test_known_values_in_original_order(self):
        # Sorted 0.005, 0.01, 0.03, 0.04 -> 0.02, 0.03, max(0.03, 0.06), max(0.06, 0.04).
        adjusted = holm_adjust([0.01, 0.04, 0.03, 0.005])
        for value, expected in zip(adjusted, [0.03, 0.06, 0.06, 0.02]):
            self.assertAlmostEqual(value, expected, places=15)

    def test_caps_at_one(self):
        self.assertEqual(holm_adjust([0.5, 0.6]), [1.0, 1.0])
        self.assertEqual(holm_adjust([1, 0]), [1.0, 0.0])

    def test_single_value_and_ties(self):
        self.assertEqual(holm_adjust((0.2,)), [0.2])
        self.assertEqual(holm_adjust([0.1, 0.1, 0.1]), [0.30000000000000004] * 3)

    def test_monotone_and_not_below_raw(self):
        raw = [0.012, 0.3, 0.0004, 0.049, 0.02, 0.8, 0.0031, 0.011]
        adjusted = holm_adjust(raw)
        order = sorted(range(len(raw)), key=raw.__getitem__)
        steps = [adjusted[index] for index in order]
        self.assertEqual(steps, sorted(steps))
        for before, after in zip(raw, adjusted):
            self.assertGreaterEqual(after, before)
            self.assertLessEqual(after, 1.0)

    def test_rejects_invalid_families(self):
        for bad in ([], [0.1] * 33, [-0.01], [1.01], [math.nan], [math.inf]):
            with self.assertRaises(ValueError):
                holm_adjust(bad)
        for bad in ([True], ["0.1"], [None], iter([0.1]), 0.1):
            with self.assertRaises(TypeError):
                holm_adjust(bad)
        self.assertEqual(len(holm_adjust([0.5] * 32)), 32)


if __name__ == "__main__":
    unittest.main()
