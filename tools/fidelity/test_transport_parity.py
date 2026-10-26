import base64
import copy
from contextlib import redirect_stdout
import gzip
import hashlib
import io
import json
import math
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import local_capture
import transport_parity as tp


def setup(version=19):
    packet = bytearray(30)
    packet[0], packet[23], packet[24] = ord("a"), version, 42
    packet[6:8] = (300).to_bytes(2, "big")
    return bytes(packet)


def appearance(sid=1, points=2):
    packet = bytearray(25)
    packet[0] = ord("s")
    packet[1:3] = sid.to_bytes(2, "big")
    packet[10:12] = (6000).to_bytes(2, "big")
    return bytes(packet) + bytes(6) + bytes([127, 127]) * (points - 2) + bytes(2)


def incoming(receipt, *packets, elapsed=0):
    payload = packets[0] if len(packets) == 1 else b"".join(
        len(packet).to_bytes(2, "big") + packet for packet in packets)
    return ["i", receipt, base64.b64encode(elapsed.to_bytes(2, "big") + payload).decode("ascii")]


class TransportParityTests(unittest.TestCase):
    def archive(self, rows, complete=True):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        path = Path(temporary.name) / "synthetic.jsonl.gz"
        header = dict(type="header", schema=1, t=0, server=dict(ip="127.0.0.1", port=8766), scenario="synthetic")
        if complete:
            frames = sum(isinstance(row, list) and row[0] == "i" for row in rows)
            rows = [*rows, dict(type="meta", event="capture-end", t=rows[-1][1] if isinstance(rows[-1], list)
                               else rows[-1]["t"], frames=frames)]
        with gzip.open(path, "wt", encoding="utf-8", newline="\n") as stream:
            for row in [header, *rows]:
                stream.write(json.dumps(row) + "\n")
        return path

    def anchors(self, batched=False, count=160, points=100):
        rows = [incoming(0, setup(), appearance(points=points), elapsed=65535)]
        previous = 0
        for i in range(1, count + 1):
            at = math.ceil(i * 224 / 42) * 42 if batched else i * 224
            rows.append(incoming(at, b"G", elapsed=at-previous))
            previous = at
        return rows

    def test_perfect_anchors_and_42ms_batching(self):
        for batched in (False, True):
            with self.subTest(batched=batched):
                path = self.archive(self.anchors(batched))
                before = path.read_bytes()
                report = tp.measure([path])
                self.assertEqual(path.read_bytes(), before)
                self.assertEqual(report["captures"][0]["sha256"], tp.pc.sha256(path))
                self.assertEqual(report["decoderSha256"], tp.pc.sha256(Path(tp.sp.__file__)))
                for clock in ("receipt", "server"):
                    values = report["summary"]["anchors"][clock]
                    self.assertEqual(values["all"]["n"], 151)
                    self.assertEqual(values["big"], values["all"])
                    if batched:
                        self.assertGreater(values["all"]["abs"]["p50"], 2)
                    else:
                        self.assertAlmostEqual(values["all"]["rms"], 0)

    def test_large_means_body_points_not_number_of_reports(self):
        summary = tp.measure([self.archive(self.anchors(points=2))])["summary"]
        self.assertGreater(summary["anchors"]["receipt"]["all"]["n"], 100)
        self.assertEqual(summary["anchors"]["receipt"]["big"]["n"], 0)

    def test_delay_resets_at_setup_and_reconnect_and_excludes_setup(self):
        rows = []
        for epoch in range(3):
            base = epoch * 10000
            if epoch != 1:
                rows.append(dict(type="meta", event="connect", t=base))
            rows.append(incoming(base, setup(), elapsed=65535))
            for i, extra in enumerate((0, 10, 0, 20), 1):
                rows.append(incoming(base + 500 + i * 100 + extra, b"p", elapsed=100))
        report = tp.measure([self.archive(rows)])
        self.assertEqual(len(report["captures"][0]["connections"]), 3)
        delays = report["summary"]["excessDelayMs"]
        self.assertEqual(delays["n"], 12)
        self.assertEqual(delays["p50"], 0)
        self.assertEqual(delays["p90"], 20)
        self.assertEqual(report["summary"]["headerDeltaHistogramMs"], {100: 12})

    def test_rolling_minimum_matches_naive_centered_inclusive_window(self):
        values = [(i * 37) % 101 + i/100 for i in range(3000)]
        expected = [value - min(values[max(0, i-600):i+601]) for i, value in enumerate(values)]
        self.assertEqual(tp.rolling_excess(values), expected)
        self.assertEqual(tp.rolling_excess([]), [])

    def test_motion_gaps_are_per_snake_and_keep_zero_and_long_gaps(self):
        rows = [incoming(0, setup(), appearance(1), appearance(2)),
                incoming(7, b"e\x00", b"e\x00\x02\x00", elapsed=7),
                incoming(49, b"e\x00", b"e\x00", b"e\x00\x02\x00", elapsed=42),
                incoming(449, b"e\x00", elapsed=400)]
        motion = tp.measure([self.archive(rows)])["summary"]["motionGapsMs"]
        self.assertEqual(motion["own"]["n"], 3)
        self.assertEqual((motion["own"]["p10"], motion["own"]["p50"], motion["own"]["p99"]), (0, 42, 400))
        self.assertEqual(motion["other"]["n"], 1)
        self.assertEqual(motion["other"]["p50"], 42)

    def test_reused_snake_ids_break_anchor_and_motion_runs(self):
        rows = [incoming(0, setup(), appearance(1))]
        for i in range(1, 9):
            rows.append(incoming(i * 224, b"G", b"e\x00", elapsed=224))
        rows.append(incoming(9 * 224, b"s\x00\x01", appearance(1), elapsed=224))
        for i in range(10, 18):
            rows.append(incoming(i * 224, b"G", b"e\x00", elapsed=224))
        summary = tp.measure([self.archive(rows)])["summary"]
        self.assertEqual(summary["anchors"]["receipt"]["all"]["n"], 0)
        self.assertEqual(summary["motionGapsMs"]["own"]["n"], 14)

    def test_speed_changes_skip_windows(self):
        rows = self.anchors(count=10)
        rows[6] = incoming(6 * 224, b"3\x12", b"G", elapsed=224)
        summary = tp.measure([self.archive(rows)])["summary"]
        self.assertEqual(summary["anchors"]["receipt"]["all"]["n"], 0)
        self.assertEqual(summary["anchorSkippedWindows"]["speedVariation"], 1)

    def test_packet_family_rates_reuse_census_counts(self):
        path = self.archive([incoming(0, setup()), incoming(7, b"p", b"p", elapsed=7)])
        report = tp.measure([path])
        family = next(row for row in report["summary"]["packetFamilies"] if row["command"] == "p")
        self.assertEqual(family["packets"], 2)
        self.assertAlmostEqual(family["perSecond"]["server"], 2000/7)

    def test_collector_timing_is_cumulative_not_summed(self):
        rows = self.anchors()
        rows.extend([dict(type="meta", event="collector-timing", t=40000, lateTicks=3, maxTickGapMs=90),
                     dict(type="meta", event="collector-timing", t=41000, lateTicks=4, maxTickGapMs=120)])
        timing = tp.measure([self.archive(rows)])["captures"][0]["collectorTiming"]
        self.assertEqual(timing, dict(events=2, available=True, lateTicks=4, maxTickGapMs=120))

    def test_rejects_incomplete_wrong_protocol_and_duplicate_archives(self):
        with self.assertRaisesRegex(ValueError, "capture-end"):
            tp.measure([self.archive(self.anchors(), complete=False)])
        with self.assertRaisesRegex(ValueError, "protocol 19"):
            tp.measure([self.archive([incoming(0, setup(18))])])
        path = self.archive(self.anchors())
        with self.assertRaisesRegex(ValueError, "duplicate"):
            tp.measure([path, path])

    def test_comparison_official_envelope_missing_samples_and_regression(self):
        report = tp.measure([self.archive(self.anchors())])
        tolerances = json.loads(tp.DEFAULT_TOLERANCES.read_text())
        with redirect_stdout(io.StringIO()) as output:
            self.assertEqual(tp.compare(report, report, tolerances), 0)
            candidate = json.loads(json.dumps(report))
            candidate["summary"]["anchors"]["receipt"]["all"]["abs"]["p95"] = 2
            self.assertEqual(tp.compare(report, candidate, tolerances), 1)
            candidate = json.loads(json.dumps(report))
            candidate["summary"]["excessDelayMs"]["n"] = 0
            self.assertEqual(tp.compare(report, candidate, tolerances), 1)
        self.assertIn("FAIL (n=0", output.getvalue())

    def test_loopback_guard(self):
        self.assertEqual(local_capture.loopback_endpoint("127.0.0.1:8766"), ("127.0.0.1", 8766))
        for bind in ("0.0.0.0:8766", "162.19.235.91:444", "localhost:8766", "[::1]:8766", "127.0.0.1:0"):
            with self.subTest(bind=bind), self.assertRaisesRegex(ValueError, "loopback"):
                local_capture.loopback_endpoint(bind)

    def test_summary_reference_omits_detail_without_changing_gates(self):
        path = self.archive(self.anchors())
        compact = tp.measure([path], summary_only=True, relative_to=path.parent)
        capture = compact["captures"][0]
        self.assertEqual(capture["path"], path.name)
        self.assertNotIn("connections", capture)
        self.assertNotIn("packetFamilies", capture["summary"])
        with redirect_stdout(io.StringIO()):
            self.assertEqual(tp.compare(compact, tp.measure([path]), json.loads(tp.DEFAULT_TOLERANCES.read_text())), 0)

    def test_local_missing_reference_fails_before_creating_output(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            config = root / "host.json"
            config.write_text('{"bind": "127.0.0.1:18766"}')
            with self.assertRaises(FileNotFoundError):
                local_capture.run(config, root / "output", 60, root / "missing.json", tp.DEFAULT_TOLERANCES)
            self.assertFalse((root / "output").exists())

    def test_local_default_uses_clean_server_reference_and_allows_home_override(self):
        with patch.object(local_capture, "run", return_value=0) as run:
            self.assertEqual(local_capture.main(["--config", "host.json", "--out", "new"]), 0)
            self.assertEqual(run.call_args.args[3], tp.DEFAULT_REFERENCE)
            self.assertEqual(tp.DEFAULT_REFERENCE.name, "transport-official-near-server.json")
            home = "tests/fixtures/transport-official-reference.json"
            local_capture.main(["--config", "host.json", "--out", "new", "--reference", home])
            self.assertEqual(run.call_args.args[3], Path(home))

    def test_clean_selection_never_bridges_a_mask_or_changes_baseline_context(self):
        epoch = tp.Epoch(1, 1, 0)
        epoch.frames = [(0, 0), (10, 10), (30, 20), (40, 30)]
        # Mask [15, 35]: only the original 0->10 gap survives. Removing masked
        # receipts first would fabricate a 30 ms gap and alter the delay floor.
        samples = epoch.finish(lambda a, b: b < 15 or a > 35)
        self.assertEqual(samples.frames, 1)
        self.assertEqual(samples.gaps["server"], [10])
        self.assertEqual(samples.durations["receipt"], 10)
        self.assertEqual(samples.delays, [0])

    def test_clean_anchor_requires_the_whole_estimation_window(self):
        epoch = tp.Epoch(1, 1, 0)
        epoch.frames = [(i*224, i*224) for i in range(11)]
        epoch.anchors[(1, 1)] = [(i*224, i*224, 6, 100) for i in range(10)]
        samples = epoch.finish(lambda a, b: not a <= 1000 <= b)
        self.assertEqual(samples.anchors["receipt"]["all"], [])
        self.assertEqual(samples.skipped["selection"], 1)

    def test_invalid_deflate_is_recorded_as_an_exclusion(self):
        valid = self.archive(self.anchors())
        corrupt = valid.with_name("corrupt.jsonl.gz")
        # Valid gzip header, invalid DEFLATE block type (reserved value 3).
        corrupt.write_bytes(b"\x1f\x8b\x08\x00" + bytes(6) + b"\x07" + bytes(8))
        report = tp.measure([valid, corrupt], skip_invalid=True)
        self.assertEqual(len(report["captures"]), 1)
        self.assertEqual(report["excluded"][0]["sha256"], tp.pc.sha256(corrupt))
        self.assertIn("invalid block type", report["excluded"][0]["reason"])

    def test_view_bins_use_preceding_membership_and_keep_pooled_statistics(self):
        rows = [incoming(0, setup(), appearance())]
        previous = 2
        for second, end in enumerate((4, 7, 12, 22, 23), 1):
            rows.append(incoming(second*1000, *(appearance(sid) for sid in range(previous, end)), elapsed=1000))
            previous = end
        rows.append(incoming(6000, b"p", elapsed=1000))
        report = tp.measure([self.archive(rows)])
        bins = report["captures"][0]["viewStrata"]["bins"]
        # Two peers at t=1000 affect the NEXT gap, never the preceding second.
        expected = dict.fromkeys(tp.tv.BINS, 1)
        self.assertEqual({k: v["seconds"] for k, v in bins.items()}, expected)
        self.assertEqual(report["summary"]["frames"], 6)
        self.assertEqual(report["summary"]["frameGapsMs"]["server"]["p50"], 1000)
        self.assertNotIn("viewStrata", report["summary"])
        self.assertEqual([tp.tv.binof(n) for n in (0, 1, 2, 3, 5, 6, 10, 11, 20, 21)],
                         ["0", "1-2", "1-2", "3-5", "3-5", "6-10", "6-10", "11-20", "11-20", "21+"])

    def test_view_whole_windows_include_silence_and_censor_bin_crossings(self):
        rows = [incoming(0, setup(), appearance()), incoming(3000, b"p", elapsed=3000)]
        report = tp.measure([self.archive(rows)])
        zero = report["captures"][0]["viewStrata"]["bins"]["0"]
        self.assertEqual(zero["seconds"], 3)
        self.assertEqual(zero["frames"], 1)
        self.assertEqual(zero["wholeWindows"]["seconds"], 3)
        self.assertEqual(zero["wholeWindows"]["framesPerSecond"], 0)
        rows = [incoming(0, setup(), appearance()), incoming(1500, appearance(2), elapsed=1500),
                incoming(3000, b"p", elapsed=1500)]
        bins = tp.measure([self.archive(rows)])["captures"][0]["viewStrata"]["bins"]
        self.assertEqual(bins["0"]["wholeWindows"]["seconds"], 1)
        self.assertEqual(bins["1-2"]["wholeWindows"]["seconds"], 1)
        self.assertEqual(sum(b["wholeWindows"]["seconds"] for b in bins.values()), 2)

    def test_view_time_weighted_activity_and_event_load(self):
        fast = bytearray(appearance())
        fast[10:12] = (14000).to_bytes(2, "big")
        rows = [incoming(0, setup(), bytes(fast), appearance(2)),
                incoming(1000, b"3\x12", b"E\x40", b"G", b"p", b"p", b"F", b"y\x00\x01", elapsed=1000),
                incoming(4000, b"p", elapsed=3000)]
        data = tp.measure([self.archive(rows)])["captures"][0]["viewStrata"]["bins"]["1-2"]
        a = data["activity"]
        # 1 second at 14, then 3 at 1: frame-weighted mean would be wrong.
        self.assertEqual(a["ownSpeed"], 4.25)
        self.assertEqual(a["ownBoost"], 0.25)
        self.assertEqual(a["ownFast"], 0.25)
        self.assertEqual(a["ownTurning"], 0.75)
        self.assertEqual(a["ownTurnChangesPerSnakeSecond"], 0.25)
        self.assertEqual(a["peerSpeed"], 6)
        self.assertEqual(a["meanPeers"], 1)
        self.assertEqual(a["ownAnchorsPerSnakeSecond"], 0.25)
        self.assertEqual(a["pingPerSecond"], 0.75)
        self.assertEqual(a["foodPerSecond"], 0.25)
        self.assertEqual(a["preyPerSecond"], 0.25)

    def test_view_life_setup_and_connection_boundaries_are_not_bridged(self):
        rows = [incoming(0, setup(), appearance()), incoming(1000, b"p", elapsed=1000),
                incoming(2000, b"s\x00\x01", elapsed=1000),
                incoming(3000, b"p", elapsed=1000), incoming(4000, appearance(), elapsed=1000),
                incoming(5000, b"p", elapsed=1000), incoming(6000, setup(), appearance(), elapsed=1000),
                incoming(7000, b"p", elapsed=1000), dict(type="meta", event="close", t=8000),
                dict(type="meta", event="connect", t=9000), incoming(9000, setup(), appearance()),
                incoming(10000, b"p", elapsed=1000)]
        data = tp.measure([self.archive(rows)])["captures"][0]["viewStrata"]["bins"]["0"]
        self.assertEqual(data["seconds"], 4)
        self.assertEqual(data["frames"], 4)
        self.assertEqual(data["wholeWindows"]["seconds"], 4)

    def view_report(self, exposures=None):
        """Deliberately synthetic workload/rates, independent of official bounds."""
        limits = json.loads(tp.DEFAULT_TOLERANCES.read_text())
        bins = tp.tv.ViewSamples().summary(tp.quantiles)["bins"]
        for label, seconds in (exposures or {"1-2": 60}).items():
            row = bins[label]
            row.update(seconds=seconds, frames=seconds*10, le9Fraction=0.8)
            row["wholeWindows"].update(seconds=seconds, frames=seconds*10, framesPerSecond=10)
            row["activity"].update({key: 1 for key in limits["viewMatched"]["activity"]})
        return dict(schemaVersion=1, wireVersion=19, status="complete", summary={},
                    captures=[dict(viewStrata=dict(contract=tp.tv.CONTRACT, bins=bins))])

    def view_compare(self, reference, candidate, tolerances=None):
        with redirect_stdout(io.StringIO()) as output:
            code = tp.compare_views(reference, candidate, tolerances or json.loads(tp.DEFAULT_TOLERANCES.read_text()))
        return code, output.getvalue()

    def test_view_minimum_exposure_and_windows_are_inconclusive(self):
        report = self.view_report()
        self.assertEqual(self.view_compare(report, report)[0], 0)
        for key, value in (("seconds", 29.999), ("wholeWindows", 19)):
            candidate = copy.deepcopy(report)
            row = candidate["captures"][0]["viewStrata"]["bins"]["1-2"]
            if key == "wholeWindows":
                row[key]["seconds"] = value
            else:
                row[key] = value
            for r, c in ((report, candidate), (candidate, report)):
                code, output = self.view_compare(r, c)
                self.assertEqual(code, 3)
                self.assertIn("INCONCLUSIVE (minimum", output)
        exact = self.view_report({"1-2": 30})
        exact["captures"][0]["viewStrata"]["bins"]["1-2"]["wholeWindows"]["seconds"] = 20
        self.assertEqual(self.view_compare(exact, exact)[0], 0)

    def test_view_activity_mismatch_and_missing_data_never_pass(self):
        report = self.view_report()
        for key, value in (("ownBoost", 0.89), ("peerAnchorsPerSnakeSecond", 2),
                           ("meanPeers", 2), ("pingPerSecond", None), ("ownSpeed", float("nan"))):
            candidate = copy.deepcopy(report)
            candidate["captures"][0]["viewStrata"]["bins"]["1-2"]["activity"][key] = value
            code, output = self.view_compare(report, candidate)
            self.assertEqual(code, 3)
            self.assertIn("NOT ACTIVITY-MATCHED", output)
            self.assertIn(key, output)

    def test_view_rate_failure_is_separate_from_activity_mismatch(self):
        reference = self.view_report()
        for value in (7, 13):
            candidate = copy.deepcopy(reference)
            candidate["captures"][0]["viewStrata"]["bins"]["1-2"]["wholeWindows"]["framesPerSecond"] = value
            code, output = self.view_compare(reference, candidate)
            self.assertEqual(code, 1)
            self.assertIn("FAIL (emission rate)", output)

    def test_view_candidate_mixture_is_fixed_without_support_renormalization(self):
        candidate = self.view_report({"1-2": 30, "3-5": 90})
        candidate["captures"][0]["viewStrata"]["bins"]["1-2"]["wholeWindows"]["framesPerSecond"] = 5
        reference = copy.deepcopy(candidate)
        # Different reference exposure must not change the candidate's 1:3 weights.
        reference["captures"][0]["viewStrata"]["bins"]["1-2"]["seconds"] = 90
        code, output = self.view_compare(reference, candidate)
        self.assertEqual(code, 0)
        self.assertIn("weighted frames/s=8.750/8.750", output)
        candidate["captures"][0]["viewStrata"]["bins"]["1-2"]["seconds"] = 29
        code, output = self.view_compare(reference, candidate)
        self.assertEqual(code, 3)
        self.assertIn("exposure weight=0.7563", output)
        self.assertNotIn("mixture: PASS", output)

    def test_view_every_candidate_and_reference_capture_validated_separately(self):
        reference = self.view_report()
        candidate = copy.deepcopy(reference)
        bad = copy.deepcopy(candidate["captures"][0])
        bad["viewStrata"]["bins"]["1-2"]["wholeWindows"]["framesPerSecond"] = 20
        candidate["captures"].append(bad)
        code, output = self.view_compare(reference, candidate)
        self.assertEqual(code, 1)
        self.assertIn("c1/r1 mixture: PASS", output)
        self.assertIn("c2/r1 mixture: FAIL", output)
        code, output = self.view_compare(candidate, reference)
        self.assertEqual(code, 1)
        self.assertIn("c1/r2 mixture: FAIL", output)

    def test_view_missing_reference_bins_and_invalid_contract(self):
        candidate = self.view_report()
        reference = copy.deepcopy(candidate)
        del reference["captures"][0]["viewStrata"]
        code, output = self.view_compare(reference, candidate)
        self.assertEqual(code, 3)
        self.assertIn("reference lacks view bins", output)
        reference["captures"][0]["viewStrata"] = dict(contract="unknown")
        with self.assertRaisesRegex(ValueError, "contract"):
            self.view_compare(reference, candidate)
        reference["captures"][0]["viewStrata"] = None
        with self.assertRaisesRegex(ValueError, "must be an object"):
            self.view_compare(reference, candidate)

    def test_view_cli_is_opt_in_and_does_not_invoke_pooled_comparison(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary)/"report.json"
            path.write_text(json.dumps(self.view_report()))
            with patch.object(tp, "compare", return_value=1) as pooled, patch.object(tp, "compare_views", return_value=3) as view:
                self.assertEqual(tp.main(["compare", str(path), str(path)]), 1)
                view.assert_not_called()
                self.assertEqual(tp.main(["compare", str(path), str(path), "--view-matched"]), 3)
                self.assertEqual(pooled.call_count, 1)

    def test_view_reference_is_additive_and_matches_frozen_study(self):
        reference = json.loads(tp.DEFAULT_REFERENCE.read_text())
        study_dir = tp.ROOT/"research/fidelity/2026-10-26-sparse-view-cadence"
        study = {r["sha256"]: r for r in json.loads((study_dir/"results.json").read_text())["primary"]}
        residual_dir = tp.ROOT/"research/fidelity/2026-10-26-cadence-residual"
        residual = {r["inputSha256"]: r for r in json.loads((residual_dir/"results.json").read_text())}
        for capture in reference["captures"]:
            data = capture.pop("viewStrata")
            self.assertEqual(data["contract"], "transport-view-v1")
            prior = study[capture["sha256"]]
            for label, row in data["bins"].items():
                gold = prior["summaries"]["left_"+label]
                self.assertEqual(row["frames"], gold["n"])
                self.assertEqual(row["seconds"], gold["exposureMs"]/1000)
                self.assertEqual(row["le9Fraction"], gold["le9"])
                self.assertEqual(row["wholeWindows"]["seconds"], prior["windows"][label]["n"])
            gold = residual[capture["sha256"]]
            for key, source in (("peerFast", "fastPct"), ("peerBoost", "boost14Pct"), ("peerTurning", "turningPct")):
                self.assertAlmostEqual(data["bins"]["6-10"]["activity"][key]*100, gold[source], places=8)
        provenance = reference.pop("viewStrataProvenance")
        original = (json.dumps(reference, indent=2)+"\n").encode()
        self.assertEqual(hashlib.sha256(original).hexdigest(), "2b8beddc67962f62580705c8d1d8749d024285e6dffd3702a9e5ab2dd8b7f6ac")
        self.assertEqual(provenance["originalReferenceSha256"], hashlib.sha256(original).hexdigest())

    def test_view_zero_peer_structural_missing_activity_is_eligible(self):
        report = tp.measure([self.archive(self.anchors(count=160))])
        self.assertEqual(self.view_compare(report, report)[0], 0)

    def test_view_rate_failure_takes_precedence_over_inconclusive(self):
        reference = self.view_report({"1-2": 10, "3-5": 60})
        candidate = copy.deepcopy(reference)
        candidate["captures"][0]["viewStrata"]["bins"]["3-5"]["wholeWindows"]["framesPerSecond"] = 20
        code, output = self.view_compare(reference, candidate)
        self.assertEqual(code, 1)
        self.assertIn("INCONCLUSIVE (minimum", output)
        self.assertIn("View result: FAIL", output)

    def test_view_invalid_bounds_are_rejected_even_without_exposure(self):
        report = self.view_report({"1-2": 1})
        limits = json.loads(tp.DEFAULT_TOLERANCES.read_text())
        limits["viewMatched"]["emissionRate"]["absolute"] = -1
        with self.assertRaisesRegex(ValueError, "bounds"):
            self.view_compare(report, report, limits)


if __name__ == "__main__":
    unittest.main()
