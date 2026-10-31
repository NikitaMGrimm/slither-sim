"""Run examples/bot.py's food-seeking controller in-process through the C ABI.

Build the library with `cargo build --release --locked -p slither-sim-ffi`. The
reset goes through a JSON request; steps and frames use typed arrays, without
JSON. Decisions match examples/bot.py, so both print the same summary.
"""

import argparse
import ctypes
import json
import math
import os
from pathlib import Path
import sys

ABI_VERSION = 4
PROTOCOL = "slither-sim-json-v6"
NONE = 0xFFFFFFFF

F64, U32, U8 = ctypes.POINTER(ctypes.c_double), ctypes.POINTER(ctypes.c_uint32), ctypes.POINTER(ctypes.c_uint8)


class WorldRules(ctypes.Structure):
    _fields_ = [("boundary_mode", ctypes.c_uint32), ("flags", ctypes.c_uint32),
                ("pit_radius", ctypes.c_double), ("min_score", ctypes.c_uint32),
                ("kill_score_seconds", ctypes.c_double)]


class Frame(ctypes.Structure):
    """`slither_frame` from ffi/include/slither_sim.h, member for member."""

    _fields_ = [
        ("time", ctypes.c_double), ("radius", ctypes.c_double), ("vision", ctypes.c_double),
        ("ticks", ctypes.c_uint32), ("living", ctypes.c_uint32),
        *((name, ctypes.c_size_t) for name in ("slots", "snakes", "body", "food", "prey", "deaths")),
        ("snake_slot", U32), ("snake_x", F64), ("snake_y", F64), ("snake_heading", F64),
        ("snake_speed", F64), ("snake_radius", F64), ("snake_length", F64), ("snake_segments", U32),
        ("snake_fam", F64), ("snake_score", F64), ("snake_kills", U32), ("snake_kill_score", U32),
        ("snake_body_start", U32), ("snake_body_x", F64), ("snake_body_y", F64),
        ("food_x", F64), ("food_y", F64), ("food_value", F64), ("food_size", F64), ("food_origin", U8),
        ("prey_id", ctypes.POINTER(ctypes.c_uint64)), ("prey_x", F64), ("prey_y", F64),
        ("prey_heading", F64), ("prey_target_heading", F64), ("prey_speed", F64), ("prey_size", F64),
        ("view_self", U32), ("view_snakes_start", U32), ("view_snakes", U32), ("view_food_start", U32),
        ("view_food", U32), ("view_prey_start", U32), ("view_prey", U32),
        ("death_time", F64), ("death_victim", U32), ("death_killer", U32), ("death_reason", U8),
        ("death_food", U32), ("rules", WorldRules),
    ]


def library_name():
    if os.name == "nt":
        return "slither_sim_ffi.dll"
    return "libslither_sim_ffi.dylib" if sys.platform == "darwin" else "libslither_sim_ffi.so"


def load(path):
    lib = ctypes.CDLL(str(path))
    session = ctypes.c_void_p
    lib.slither_abi_version.restype = ctypes.c_uint32
    lib.slither_session_new.restype = session
    lib.slither_session_free.argtypes = [session]
    lib.slither_request.argtypes = [session, ctypes.c_char_p, ctypes.c_size_t,
                                    ctypes.POINTER(ctypes.c_void_p), ctypes.POINTER(ctypes.c_size_t)]
    lib.slither_step.argtypes = [session, F64, ctypes.c_size_t, U8, ctypes.c_size_t]
    lib.slither_read_frame.argtypes = [session, ctypes.c_size_t, ctypes.POINTER(Frame)]
    lib.slither_last_error.argtypes = [session]
    lib.slither_last_error.restype = ctypes.c_char_p
    for name in ("slither_request", "slither_step", "slither_read_frame"):
        getattr(lib, name).restype = ctypes.c_int32
    if lib.slither_abi_version() != ABI_VERSION:
        raise RuntimeError("incompatible slither-sim ABI")
    return lib


class Session:
    def __init__(self, lib):
        self.lib = lib
        self.handle = lib.slither_session_new()
        if not self.handle:
            raise RuntimeError("cannot create a session")
        self.frame = Frame()

    def close(self):
        self.lib.slither_session_free(self.handle)
        self.handle = None

    def check(self, code):
        if code:
            raise RuntimeError(self.lib.slither_last_error(self.handle).decode())

    def request(self, message):
        line = json.dumps(message, allow_nan=False).encode()
        reply, length = ctypes.c_void_p(), ctypes.c_size_t()
        self.check(self.lib.slither_request(self.handle, line, len(line), ctypes.byref(reply), ctypes.byref(length)))
        reply = json.loads(ctypes.string_at(reply, length.value))
        if reply.get("protocol") != PROTOCOL:
            raise RuntimeError("incompatible simulator protocol")
        if "error" in reply:
            raise RuntimeError(reply["error"]["message"])
        return reply["result"]

    def step(self, headings, boosts):
        self.check(self.lib.slither_step(self.handle, (ctypes.c_double * len(headings))(*headings), len(headings),
                                         (ctypes.c_uint8 * len(boosts))(*boosts), len(boosts)))

    def read_frame(self, world):
        """The world's frame; its pointers stay valid until the next call on this session."""
        self.check(self.lib.slither_read_frame(self.handle, world, ctypes.byref(self.frame)))
        return self.frame


def heading(frame, slot):
    """Steer to the nearest visible food, or back toward the center near the rim."""
    me = frame.view_self[slot]
    x, y = frame.snake_x[me], frame.snake_y[me]
    start, end = frame.view_food_start[slot], frame.view_food_start[slot + 1]
    if end > start and math.hypot(x, y) < frame.radius * 0.8:
        food_x, food_y = frame.food_x, frame.food_y
        target = min(frame.view_food[start:end], key=lambda i: (food_x[i] - x) ** 2 + (food_y[i] - y) ** 2)
        return math.atan2(food_y[target] - y, food_x[target] - x)
    return math.atan2(-y, -x)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    root = Path(__file__).resolve().parents[1]
    default = os.environ.get("SLITHER_SIM_FFI") or root / "target" / "release" / library_name()
    parser.add_argument("--library", type=Path, default=default,
                        help="the cdylib (default: $SLITHER_SIM_FFI or target/release)")
    parser.add_argument("--steps", type=int, default=200)
    args = parser.parse_args()
    if args.steps < 1:
        parser.error("--steps must be positive")
    if not args.library.is_file():
        parser.error("build the library with cargo build --release --locked -p slither-sim-ffi, or pass --library")

    session = Session(load(args.library.resolve()))
    try:
        # One world; pass "worlds": N and give N * agents actions per step for more.
        session.request({"op": "reset", "seed": 7, "agents": 4})
        frame = session.read_frame(0)
        completed = 0
        while completed < args.steps and frame.living:
            headings = [math.nan if frame.view_self[slot] == NONE else heading(frame, slot)
                        for slot in range(frame.slots)]
            session.step(headings, [0] * len(headings))
            frame = session.read_frame(0)
            completed += 1
        print(f"decisions={completed} time={frame.time:.3f}s living={frame.living}")
    finally:
        session.close()


if __name__ == "__main__":
    main()
