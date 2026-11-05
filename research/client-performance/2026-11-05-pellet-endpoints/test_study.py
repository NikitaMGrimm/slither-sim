import copy
import unittest

import recompute


class StudyTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.data = recompute.load()

    def rejects(self, change):
        data = copy.deepcopy(self.data)
        change(data)
        with self.assertRaises(AssertionError):
            recompute.validate(data)

    def test_retained_controls(self):
        self.assertEqual(len(recompute.validate(self.data)), 5)

    def test_same_binary_cannot_be_both_arms(self):
        self.rejects(lambda d: d["identities"]["endpoints"].update(binary_sha256=d["identities"]["baseline"]["binary_sha256"]))

    def test_missing_configuration(self):
        self.rejects(lambda d: d["images"].pop("authored/rtx/native/whole"))

    def test_short_image_history(self):
        self.rejects(lambda d: d["images"]["authored/rtx/native/far"]["endpoints"].update(callbacks=599))

    def test_inactive_canonical_control(self):
        def change(data):
            for row in data["canonical"].values():
                row["canonical_active"] = False
        self.rejects(change)

    def test_canonical_arm_requires_its_own_executable(self):
        self.rejects(lambda d: d["canonical"]["endpoints"].update(binary_sha256=d["identities"]["baseline"]["binary_sha256"]))

    @unittest.skipUnless((recompute.ROOT / "sources.json.gz").exists(), "source archives are kept outside Git")
    def test_corrupted_historical_source(self):
        self.rejects(lambda d: d["sources"].update(endpoints=d["sources"]["endpoints"] + "\n"))

    def test_compiler_overlap_cannot_be_selected(self):
        self.rejects(lambda d: next(r for r in d["runs"] if r["pair_status"] == "accepted").update(compile_samples="1"))

    def test_rebase_requires_full_image_matrix(self):
        self.rejects(lambda d: d["rebase"]["images"].pop("authored/igpu/original/whole"))

    def test_rebase_cannot_use_the_prior_binary(self):
        self.rejects(lambda d: d["rebase"]["identities"]["endpoints"].update(binary_sha256=d["identities"]["endpoints"]["binary_sha256"]))

    @unittest.skipUnless((recompute.ROOT / "sources.json.gz").exists(), "source archives are kept outside Git")
    def test_rebase_source_corruption(self):
        self.rejects(lambda d: d["rebase_sources"]["common"]["client/apps/wgpu-slice/src/game.rs"].update(runtime="corrupted"))

    def test_rebase_does_not_relabel_timing(self):
        self.rejects(lambda d: d["rebase"].update(timing_rerun=True))


if __name__ == "__main__":
    unittest.main()
