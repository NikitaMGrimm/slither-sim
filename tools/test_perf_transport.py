"""Offline checks for the transport benchmark's scripts, decision fold and report shaping.

The check group's smoke run drives the real debug binary with both clients; these
tests need neither Rust nor Node.
"""

import json
import math
import subprocess
import sys
import unittest

import perf_transport
import perf_transport_client


class Watchdog(unittest.TestCase):
    def test_a_silent_process_is_killed_so_its_reader_sees_eof(self):
        process = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(60)"],
                                   stdout=subprocess.PIPE)
        watchdog = perf_transport.Watchdog(process, 0.2)
        with watchdog:
            self.assertEqual(process.stdout.read1(1024), b"")
        process.wait()
        process.stdout.close()
        self.assertTrue(watchdog.fired)

    def test_a_finished_process_is_left_alone(self):
        process = subprocess.Popen([sys.executable, "-c", "print('done')"], stdout=subprocess.PIPE)
        watchdog = perf_transport.Watchdog(process, 30)
        with watchdog:
            output = process.stdout.read()
            process.wait()
        process.stdout.close()
        self.assertEqual(output.strip(), b"done")
        self.assertFalse(watchdog.fired)
        self.assertFalse(watchdog.timer.is_alive())


def frame(heads, food, views):
    """Synthetic slither-sim-json-v6 frame with only the columns the decision reads."""
    return {"snakes": {"x": [x for x, _ in heads], "y": [y for _, y in heads]},
            "food": {"x": [x for x, _ in food], "y": [y for _, y in food]}, "views": views}


class RequestScripts(unittest.TestCase):
    def test_scripts_are_deterministic_and_cover_every_slot(self):
        for scenario in perf_transport.SCENARIOS:
            with self.subTest(scenario.name):
                script = perf_transport.request_script(scenario, 5)
                self.assertEqual(script, perf_transport.request_script(scenario, 5))
                self.assertTrue(script.endswith(b"\n"))
                lines = [json.loads(line) for line in script.splitlines()]
                self.assertEqual(len(lines), 6)
                reset, *steps = lines
                self.assertEqual((reset["op"], reset["seed"], reset["worlds"], reset["agents"],
                                  reset["decisionTicks"]), ("reset", 7, scenario.worlds, scenario.agents, 6))
                self.assertEqual(reset.get("config"), scenario.config)
                for step in steps:
                    self.assertEqual(step["op"], "step")
                    self.assertEqual(len(step["actions"]), scenario.worlds)
                    for actions in step["actions"]:
                        self.assertEqual(actions["boost"], [False] * scenario.agents)
                        self.assertEqual(len(actions["heading"]), scenario.agents)
                        self.assertTrue(all(math.isfinite(h) for h in actions["heading"]))

    def test_headings_match_the_v1_baseline_scripts(self):
        # The v1 baseline (0d367f6) sent these exact headings; v2 must simulate the same worlds.
        small, official = perf_transport.SCENARIOS[0], perf_transport.SCENARIOS[1]
        self.assertEqual(perf_transport.headings(small, 3)[1], math.pi / 2 + math.pi / 2 + 0.75)
        self.assertEqual(perf_transport.headings(official, 40)[2],
                         2 * math.pi * 2 / 128 + 0.3 * math.sin(40 / 20 + 2))

    def test_default_step_counts_apply_without_override(self):
        small = perf_transport.SCENARIOS[0]
        self.assertEqual(perf_transport.request_script(small).count(b"\n"), small.steps + 1)

    def test_outward_headings_stay_near_each_spawn_angle(self):
        scenario = next(s for s in perf_transport.SCENARIOS if s.pattern == "outward")
        for step in (0, 150, 299):
            for i, heading in enumerate(perf_transport.headings(scenario, step)):
                self.assertLessEqual(abs(heading - 2 * math.pi * i / scenario.agents), 0.3 + 1e-12)


class Decision(unittest.TestCase):
    def test_fold_skips_dead_slots_and_marks_empty_food(self):
        first = frame([(0, 0), (5, 5)], [(10, 0), (-1, -1), (1, 1)], [
            {"self": 0, "snakes": [0, 1], "food": [0, 2, 1], "prey": []},
            None,
            {"self": 1, "snakes": [1], "food": [], "prey": []}])
        second = frame([(9, 9)], [(8, 8)], [{"self": 0, "snakes": [0], "food": [0], "prey": []}])
        checksum, living, food_seen = perf_transport_client.decide({"worlds": [first, second]}, 0)
        expected = (0 * 1000003 + 2 + 1 + 7 * 3 + 13 * 2) & 0xFFFFFFFF  # Row 2 is nearest.
        expected = (expected * 1000003 + -1 + 1 + 13) & 0xFFFFFFFF
        expected = (expected * 1000003 + 0 + 1 + 7 + 13) & 0xFFFFFFFF
        self.assertEqual((checksum, living, food_seen), (expected, 3, 4))

    def test_decode_rejects_errors_and_other_protocols(self):
        ok = json.dumps({"protocol": perf_transport_client.PROTOCOL, "result": {"worlds": []}})
        self.assertEqual(perf_transport_client.decode_reply(ok.encode()), {"worlds": []})
        with self.assertRaisesRegex(RuntimeError, "Reset first"):
            perf_transport_client.decode_reply(json.dumps(
                {"protocol": perf_transport_client.PROTOCOL, "error": {
                    "code": 11, "rule": None, "world": None, "slot": None,
                    "field": None, "message": "Reset first"}}))
        with self.assertRaisesRegex(RuntimeError, "protocol"):
            perf_transport_client.decode_reply('{"protocol":"other","result":{}}')


def client_run(checksum=5, reply_bytes=1000):
    return {"checksum": checksum, "reset": {"replyBytes": 100}, "replyBytes": reply_bytes,
            "waitNs": 4_000_000, "parseNs": 2_000_000, "decideNs": 1_000_000, "living": 8, "foodSeen": 40}


class Reports(unittest.TestCase):
    def test_consistency_compares_bytes_across_modes_and_checksums_across_clients(self):
        server = {"totalBytes": 1100, "replyBytes": 1000, "stepsNs": 1}
        runs = [{"mode": "server", "repeat": 0, "raw": server},
                {"mode": "python", "repeat": 0, "raw": client_run()},
                {"mode": "node", "repeat": 0, "raw": client_run()}]
        self.assertEqual(perf_transport.check_consistency("x", runs), [])
        runs[2]["raw"] = client_run(checksum=6)
        self.assertIn("checksums", perf_transport.check_consistency("x", runs)[0])
        runs[2]["raw"] = client_run(reply_bytes=999)
        self.assertIn("bytes", perf_transport.check_consistency("x", runs)[0])

    def test_per_step_values_and_table(self):
        client = perf_transport.per_step(client_run(), "python", 2)
        self.assertEqual((client["waitMs"], client["parseMs"], client["decideMs"], client["totalMs"]),
                         (2.0, 1.0, 0.5, 3.5))
        self.assertEqual((client["replyBytes"], client["living"]), (500, 4))
        server = perf_transport.per_step({"replyBytes": 1000, "stepsNs": 3_000_000}, "server", 2)
        self.assertEqual(server, {"replyBytes": 500, "totalMs": 1.5})
        summary = perf_transport.summarize([client, perf_transport.per_step(client_run(), "python", 1)])
        self.assertEqual(summary["totalMs"], {"median": 5.25, "min": 3.5, "max": 7.0})
        table = perf_transport.markdown({"summary": [
            {"scenario": "small", "mode": "server", "steps": 2, "perStep": perf_transport.summarize([server])},
            {"scenario": "small", "mode": "python", "steps": 2, "perStep": summary}]})
        rows = table.splitlines()
        self.assertEqual(len(rows), 4)
        self.assertEqual(rows[2], "| small | server | 2 | 0.5 |  |  |  | 1.500 | 1.500-1.500 |")
        self.assertIn("| 5.250 | 3.500-7.000 |", rows[3])


if __name__ == "__main__":
    unittest.main()
