"""Check the slither_sim extension against the JSON protocol it wraps.

Run through run.py, which imports the extension from a built wheel.
"""

from array import array
import ast
from collections import UserDict
import ctypes
import math
import os
from pathlib import Path
import struct
import subprocess
import sys
import threading
import unittest

import slither_sim
from slither_sim import NO_ORIGIN, NONE, REASONS, Session

# A small arena so that snakes die at the boundary and on bodies within a few dozen steps.
CONFIG = {"radius": 700, "dynamicRadius": False}
WORLDS, AGENTS = 3, 6


def as_json(frame):
    """A typed frame in the JSON frame's layout, so that dict equality compares every column."""
    def rows(start, values, slot):
        return list(values[start[slot]:start[slot + 1]])

    views = []
    for slot, me in enumerate(frame.view_self):
        views.append(None if me == NONE else {
            "self": me,
            "snakes": rows(frame.view_snakes_start, frame.view_snakes, slot),
            "food": rows(frame.view_food_start, frame.view_food, slot),
            "prey": rows(frame.view_prey_start, frame.view_prey, slot),
        })
    return {
        "time": frame.time, "ticks": frame.ticks, "living": frame.living,
        "radius": frame.radius, "vision": frame.vision,
        "snakes": {
            "slot": list(frame.snake_slot), "x": list(frame.snake_x), "y": list(frame.snake_y),
            "heading": list(frame.snake_heading), "speed": list(frame.snake_speed),
            "radius": list(frame.snake_radius), "length": list(frame.snake_length),
            "segments": list(frame.snake_segments), "fam": list(frame.snake_fam),
            "score": list(frame.snake_score), "kills": list(frame.snake_kills),
            "bodyStart": list(frame.snake_body_start), "bodyX": list(frame.snake_body_x),
            "bodyY": list(frame.snake_body_y),
        },
        "food": {
            "x": list(frame.food_x), "y": list(frame.food_y), "value": list(frame.food_value),
            "size": [None if math.isnan(size) else size for size in frame.food_size],
            "origin": [None if origin == NO_ORIGIN else origin for origin in frame.food_origin],
        },
        "prey": {
            "id": list(frame.prey_id), "x": list(frame.prey_x), "y": list(frame.prey_y),
            "heading": list(frame.prey_heading), "targetHeading": list(frame.prey_target_heading),
            "speed": list(frame.prey_speed), "size": list(frame.prey_size),
        },
        "views": views,
        "deaths": [{
            "time": frame.death_time[i], "victim": frame.death_victim[i],
            "killer": None if frame.death_killer[i] == NONE else frame.death_killer[i],
            "reason": REASONS[frame.death_reason[i]], "food": frame.death_food[i],
        } for i in range(len(frame.death_time))],
    }


def decide(frames, step):
    """World-major actions: every third slot boosts outward, the rest weave toward the center."""
    headings, boosts = [], []
    for frame in frames:
        for slot, me in enumerate(frame.view_self):
            if me == NONE:
                headings.append(math.nan)
                boosts.append(False)
                continue
            x, y = frame.snake_x[me], frame.snake_y[me]
            if slot % 3 == 0:
                headings.append(math.atan2(y, x))
                boosts.append(True)
            else:
                headings.append(math.atan2(-y, -x) + 0.4 * math.sin(step / 7 + slot))
                boosts.append(slot % 2 == 1)
    return headings, boosts


def json_step(session, headings, boosts):
    agents = session.agents
    actions = [{
        "heading": [None if math.isnan(h) else h for h in headings[w * agents:(w + 1) * agents]],
        "boost": boosts[w * agents:(w + 1) * agents],
    } for w in range(session.worlds)]
    return session.request({"op": "step", "actions": actions})


def frames(session):
    return [session.frame(world) for world in range(session.worlds)]


def fingerprint(session):
    return [as_json(frame) for frame in frames(session)]


def reset(session, **options):
    session.reset(**{"seed": 3, "worlds": WORLDS, "agents": AGENTS, "config": CONFIG, **options})


class TypedStepTests(unittest.TestCase):
    def test_typed_frames_equal_json_frames_through_deaths(self):
        typed, encoded = Session(), Session()
        reset(typed)
        reply = encoded.request({"op": "reset", "seed": 3, "worlds": WORLDS, "agents": AGENTS, "config": CONFIG})
        self.assertEqual(fingerprint(typed), reply["result"]["worlds"])
        deaths, reasons, killers, nan_steps = set(), set(), set(), 0
        for step in range(80):
            headings, boosts = decide(frames(typed), step)
            nan_steps += any(math.isnan(h) for h in headings)
            typed.step(headings, boosts)
            reply = json_step(encoded, headings, boosts)
            self.assertEqual(reply["protocol"], slither_sim.PROTOCOL)
            self.assertEqual(fingerprint(typed), reply["result"]["worlds"], f"step {step}")
            for world, frame in enumerate(frames(typed)):
                deaths.update((world, victim) for victim in frame.death_victim)
                reasons.update(frame.death_reason)
                killers.update(k != NONE for k in frame.death_killer)
        self.assertEqual({world for world, _ in deaths}, set(range(WORLDS)))
        self.assertEqual(reasons, {0, 1})
        self.assertEqual(killers, {False, True})
        self.assertGreater(nan_steps, 0)

    def test_columns_have_fixed_typecodes(self):
        session = Session()
        reset(session)
        frame = session.frame(0)
        codes = {"d": 8, "I": 4, "Q": 8, "B": 1}
        for name in dir(frame):
            column = getattr(frame, name)
            if isinstance(column, array):
                self.assertEqual(column.itemsize, codes[column.typecode], name)
        self.assertEqual(frame.snake_x.typecode, "d")
        self.assertEqual(frame.view_self.typecode, "I")
        self.assertEqual(frame.prey_id.typecode, "Q")
        self.assertEqual(frame.food_origin.typecode, "B")
        self.assertEqual(len(frame.view_food_start), AGENTS + 1)
        self.assertEqual(len(frame.snake_body_start), len(frame.snake_x) + 1)

    def test_frames_are_independent_copies(self):
        session = Session()
        reset(session)
        before = session.frame(0)
        kept = as_json(before)
        session.step(*decide([session.frame(w) for w in range(WORLDS)], 0))
        self.assertEqual(as_json(before), kept)
        self.assertNotEqual(as_json(session.frame(0)), kept)


class InputTests(unittest.TestCase):
    def inputs(self):
        session = Session()
        reset(session)
        headings, boosts = decide(frames(session), 0)
        return headings, boosts

    def stepped(self, headings, boosts):
        session = Session()
        reset(session)
        session.step(headings, boosts)
        return fingerprint(session)

    def test_buffers_and_sequences_step_alike(self):
        headings, boosts = self.inputs()
        expected = self.stepped(headings, boosts)
        flags = bytes(boosts)
        doubled = array("d", [x for h in headings for x in (h, 0.0)])
        for label, h, b in [
            ("tuples", tuple(headings), tuple(boosts)),
            ("generators", (h for h in headings), (b for b in boosts)),
            ("ints", headings, [int(b) for b in boosts]),
            ("arrays", array("d", headings), array("B", flags)),
            ("bytes", array("d", headings), flags),
            ("bytearray", array("d", headings), bytearray(flags)),
            ("memoryview", memoryview(array("d", headings)), memoryview(flags)),
            ("bool format", array("d", headings), memoryview(flags).cast("?")),
            ("strided", memoryview(doubled)[::2], flags),
            ("2-D", memoryview(array("d", headings)).cast("B").cast("d", [WORLDS, AGENTS]), flags),
        ]:
            with self.subTest(label):
                self.assertEqual(self.stepped(h, b), expected)

    def test_byte_orders_step_alike(self):
        headings, boosts = self.inputs()
        expected = self.stepped(headings, boosts)
        count = len(headings)
        for label, h, format in [
            ("big-endian ctypes", (ctypes.c_double.__ctype_be__ * count)(*headings), ">d"),
            ("little-endian ctypes", (ctypes.c_double.__ctype_le__ * count)(*headings), "<d"),
        ]:
            with self.subTest(label):
                self.assertEqual(memoryview(h).format, format)
                self.assertEqual(self.stepped(h, bytes(boosts)), expected)

    def test_big_endian_nan_for_a_living_slot_is_rejected(self):
        headings, boosts = self.inputs()
        headings[AGENTS + 1] = math.nan
        typed, listed = Session(), Session()
        reset(typed)
        reset(listed)
        before = fingerprint(typed)
        with self.assertRaises(ValueError) as caught:
            typed.step((ctypes.c_double.__ctype_be__ * len(headings))(*headings), bytes(boosts))
        with self.assertRaises(ValueError) as expected:
            listed.step(headings, boosts)
        self.assertEqual(str(caught.exception), str(expected.exception))
        self.assertEqual(fingerprint(typed), before)

    def test_unaligned_buffers(self):
        headings, boosts = self.inputs()
        expected = self.stepped(headings, boosts)
        packed = b"x" + struct.pack(f"={len(headings)}d", *headings)
        unaligned = memoryview(packed)[1:].cast("d")
        self.assertEqual(self.stepped(unaligned, memoryview(b"x" + bytes(boosts))[1:]), expected)

    def test_empty_buffers_reach_session_validation(self):
        session = Session()
        reset(session)
        with self.assertRaises(ValueError) as caught:
            session.step(array("d"), b"")
        self.assertEqual(str(caught.exception), "One heading and one boost per agent of every world "
                         f"required: got 0 headings and 0 boosts, need {WORLDS * AGENTS}")

    def test_none_heading_in_sequences_means_no_action(self):
        session = Session()
        reset(session)
        for step in range(40):
            headings, boosts = decide(frames(session), step)
            session.step([None if math.isnan(h) else h for h in headings], boosts)
        self.assertLess(sum(frame.living for frame in frames(session)), WORLDS * AGENTS)

    def test_numpy_arrays(self):
        try:
            import numpy as np
        except ImportError:
            self.skipTest("numpy is not installed")
        headings, boosts = self.inputs()
        expected = self.stepped(headings, boosts)
        h = np.array(headings, dtype=np.float64)
        self.assertEqual(self.stepped(h, np.array(boosts, dtype=np.bool_)), expected)
        self.assertEqual(self.stepped(h.reshape(WORLDS, AGENTS), np.array(boosts, dtype=np.uint8)), expected)
        session = Session()
        reset(session)
        x = np.frombuffer(session.frame(0).snake_x, dtype=np.float64)
        self.assertEqual(list(x), list(session.frame(0).snake_x))

    def test_wrong_item_types_raise_type_error(self):
        headings, boosts = self.inputs()
        session = Session()
        reset(session)
        for label, h, b in [
            ("float32 headings", array("f", headings), boosts),
            ("byte headings", bytes(len(headings)), boosts),
            ("int64 boosts", headings, array("q", [0] * len(boosts))),
            ("double boosts", headings, array("d", [0] * len(boosts))),
            ("string headings", ["north"] * len(headings), boosts),
            ("two-item format", memoryview(array("d", headings)).cast("B").cast("Q"), boosts),
        ]:
            with self.subTest(label), self.assertRaises(TypeError):
                session.step(h, b)
        self.assertEqual(session.frame(0).ticks, 0)


class ErrorTests(unittest.TestCase):
    def test_errors_before_reset(self):
        session = Session()
        self.assertEqual((session.worlds, session.agents), (0, 0))
        with self.assertRaisesRegex(ValueError, "^Reset first$"):
            session.step([], [])
        with self.assertRaisesRegex(ValueError, "^Reset first$"):
            session.frame(0)
        self.assertEqual(session.request({"op": "step", "actions": []}),
                         {"protocol": slither_sim.PROTOCOL, "error": "Reset first"})

    def test_rejected_resets_keep_the_previous_worlds(self):
        session = Session()
        reset(session)
        before = fingerprint(session)
        for options, message in [
            ({"worlds": 0}, "Need 1..1024 worlds, 1..128 agents and 1..1000 decisionTicks"),
            ({"agents": 129}, "Need 1..1024 worlds, 1..128 agents and 1..1000 decisionTicks"),
            ({"seed": -1}, "invalid value: integer `-1`, expected u32"),
            ({"config": {"radius": 700, "colour": 1}}, "unknown field `colour`"),
        ]:
            with self.subTest(options), self.assertRaises(ValueError) as caught:
                reset(session, **options)
            self.assertIn(message, str(caught.exception))
            self.assertEqual((session.worlds, session.agents), (WORLDS, AGENTS))
            self.assertEqual(fingerprint(session), before)

    def test_reset_errors_match_json_errors(self):
        typed, encoded = Session(), Session()
        with self.assertRaises(ValueError) as caught:
            typed.reset(seed=1, worlds=2, agents=3, decision_ticks=0)
        reply = encoded.request({"op": "reset", "seed": 1, "worlds": 2, "agents": 3, "decisionTicks": 0})
        self.assertEqual(str(caught.exception), reply["error"])

    def test_default_reset_matches_the_protocol_defaults(self):
        typed, encoded = Session(), Session()
        typed.reset()
        reply = encoded.request({"op": "reset"})
        self.assertEqual((typed.worlds, typed.agents), (1, 4))
        self.assertEqual(fingerprint(typed), reply["result"]["worlds"])

    def test_rejected_steps_change_no_world(self):
        session, control = Session(), Session()
        reset(session)
        reset(control)
        headings, boosts = decide(frames(session), 0)
        before = fingerprint(session)
        wrong = list(headings)
        wrong[AGENTS + 1] = math.nan  # A living slot of world 1, after valid world 0 actions.
        for label, h, b, message in [
            ("short", headings[:-1], boosts,
             f"One heading and one boost per agent of every world required: got {len(headings) - 1} "
             f"headings and {len(boosts)} boosts, need {len(boosts)}"),
            ("no action for a living slot", wrong, boosts, None),
            ("infinite heading", [math.inf] * len(headings), boosts, None),
        ]:
            with self.subTest(label), self.assertRaises(ValueError) as caught:
                session.step(h, b)
            if message:
                self.assertEqual(str(caught.exception), message)
            self.assertEqual(fingerprint(session), before)
        session.step(headings, boosts)
        control.step(headings, boosts)
        self.assertEqual(fingerprint(session), fingerprint(control))

    def test_typed_step_errors_match_json_errors(self):
        typed, encoded = Session(), Session()
        reset(typed)
        encoded.request({"op": "reset", "seed": 3, "worlds": WORLDS, "agents": AGENTS, "config": CONFIG})
        headings, boosts = decide(frames(typed), 0)
        headings[AGENTS + 1] = math.nan
        with self.assertRaises(ValueError) as caught:
            typed.step(headings, boosts)
        self.assertTrue(str(caught.exception).startswith("World 1: "))
        self.assertEqual(str(caught.exception), json_step(encoded, headings, boosts)["error"])

    def test_unknown_world(self):
        session = Session()
        with self.assertRaisesRegex(ValueError, "^Reset first$"):
            session.frame(-1)
        reset(session)
        for world in (WORLDS, -1, -WORLDS, 2**100, -(2**100)):
            with self.subTest(world), self.assertRaisesRegex(IndexError, "^Unknown world$"):
                session.frame(world)
        with self.assertRaises(TypeError):
            session.frame(0.0)

    def test_mappings_other_than_dicts_are_rejected_like_their_stubs(self):
        session = Session()
        reset(session)
        before = fingerprint(session)
        with self.assertRaises(TypeError):
            reset(session, config=UserDict(CONFIG))
        self.assertEqual(fingerprint(session), before)
        with self.assertRaises(TypeError):
            session.request(UserDict({"op": "info"}))
        stub = ast.parse(Path(slither_sim.__file__).with_name("__init__.pyi").read_text(encoding="utf-8"))
        methods = {node.name: node for node in ast.walk(stub) if isinstance(node, ast.FunctionDef)}
        annotations = {arg.arg: ast.unparse(arg.annotation)
                       for name in ("reset", "request") for arg in methods[name].args.args if arg.annotation}
        self.assertEqual(annotations["config"], "dict[str, Any] | None")
        self.assertEqual(annotations["message"], "dict[str, Any] | str | bytes")

    def test_package_version_is_the_simulator_version(self):
        info = Session().request({"op": "info"})["result"]
        self.assertEqual(slither_sim.__version__, info["version"])

    def test_requests_accept_dicts_text_and_bytes(self):
        session = Session()
        replies = [session.request(message) for message in ({"op": "info"}, '{"op":"info"}', b'{"op":"info"}')]
        self.assertEqual(replies[0], replies[1])
        self.assertEqual(replies[0], replies[2])
        info = replies[0]["result"]
        self.assertEqual(tuple(info["origins"]), slither_sim.ORIGINS)
        self.assertEqual(info["maxAgents"], 128)
        self.assertIn("error", session.request(b"{not json"))
        self.assertIn("error", session.request({"op": "fly"}))


class ThreadTests(unittest.TestCase):
    @staticmethod
    def run_session(seed, out):
        session = Session()
        reset(session, seed=seed)
        for step in range(40):
            session.step(*decide(frames(session), step))
            out.append(fingerprint(session))

    def test_sessions_in_threads_match_serial_runs(self):
        serial = {seed: [] for seed in (5, 6)}
        for seed, out in serial.items():
            self.run_session(seed, out)
        threaded = {seed: [] for seed in serial}
        threads = [threading.Thread(target=self.run_session, args=item) for item in threaded.items()]
        for thread in threads:
            thread.start()
        for thread in threads:
            thread.join()
        self.assertEqual(threaded, serial)

    def test_overlapping_calls_on_one_session_raise(self):
        session = Session()
        session.reset(seed=1, worlds=64, agents=32, decision_ticks=100)
        headings = [0.0] * (64 * 32)
        boosts = [False] * len(headings)
        done, failures, errors = threading.Event(), [], []

        def stepper():
            try:
                for _ in range(5):
                    session.step(headings, boosts)
            except BaseException as error:  # Reported by the main thread.
                failures.append(error)
            finally:
                done.set()

        thread = threading.Thread(target=stepper)
        thread.start()
        while not done.is_set():
            try:
                session.frame(0)
            except RuntimeError as error:
                errors.append(error)
        thread.join()
        self.assertEqual(failures, [])
        self.assertTrue(errors, "no call overlapped a step")
        self.assertGreater(session.frame(0).ticks, 0)


class ExampleTests(unittest.TestCase):
    def test_example_bot_runs(self):
        root = Path(__file__).resolve().parents[1]
        package = Path(slither_sim.__file__).resolve().parents[1]
        environment = {**os.environ, "PYTHONPATH": os.pathsep.join([str(package), os.environ.get("PYTHONPATH", "")])}
        result = subprocess.run([sys.executable, str(root / "examples" / "bot.py"), "--steps", "10"],
                                capture_output=True, text=True, env=environment, check=True)
        self.assertEqual(result.stdout, "decisions=10 time=0.420s living=4\n")


if __name__ == "__main__":
    unittest.main()
