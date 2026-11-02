"""Offline CLI dispatch checks; no GPU, recording or image readback required."""
from contextlib import redirect_stderr, redirect_stdout
from io import StringIO
from pathlib import Path
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch

import reduce_runs as reducer


class ParityCli(unittest.TestCase):
    def test_default_pre_mip_and_revision_options_reach_the_exporter(self):
        with TemporaryDirectory() as directory:
            output = Path(directory)
            for reference, prefix in ((None, ""), (None, "pre-mip-"), (Path("retained-control"), "revision-")):
                args = ["retained-journals", "--output", str(output), "--series", "parity"]
                if reference is not None:
                    args += ["--reference", str(reference)]
                if prefix:
                    args += ["--prefix", prefix]
                with self.subTest(prefix=prefix), patch.object(reducer, "export_parity") as export, redirect_stdout(StringIO()):
                    reducer.main(args)
                    export.assert_called_once_with(Path("retained-journals"), output, reference, prefix)

    def test_parity_options_are_not_silently_ignored_by_other_series(self):
        for option in (["--reference", "retained-control"], ["--prefix", "revision-"]):
            with self.subTest(option=option), patch.object(reducer, "export_series") as export, redirect_stderr(StringIO()):
                with self.assertRaises(SystemExit) as error:
                    reducer.main(["retained-journals", "--series", "history", *option])
                self.assertEqual(error.exception.code, 2)
                export.assert_not_called()

    def test_reference_requires_a_nonempty_prefix_before_touching_baselines(self):
        with TemporaryDirectory() as directory:
            output = Path(directory)
            baselines = [output / name for name in ("parity-runs.csv", "pixels.csv")]
            for path in baselines:
                path.write_bytes(b"retained baseline\n")
            for prefix in ([], ["--prefix", ""]):
                error_text = StringIO()
                args = ["retained-journals", "--output", str(output), "--series", "parity",
                        "--reference", "retained-control", *prefix]
                with self.subTest(prefix=prefix), patch.object(reducer, "export_parity") as export, redirect_stderr(error_text):
                    with self.assertRaises(SystemExit) as error:
                        reducer.main(args)
                    self.assertEqual(error.exception.code, 2)
                    self.assertIn("--reference requires --prefix", error_text.getvalue())
                    export.assert_not_called()
                for path in baselines:
                    self.assertEqual(path.read_bytes(), b"retained baseline\n")


if __name__ == "__main__":
    unittest.main()
