"""Synthetic wire bytes and expected episodes, independent of archive data."""

import copy
from dataclasses import asdict
import unittest

from sector_events import MalformedPacket, SectorLedger, UnsupportedPacket


class SectorEventsTests(unittest.TestCase):
    def setUp(self):
        self.ledger = SectorLedger()
        self.index = 0

    def feed(self, *packets, connection=1, life=1, server=None):
        self.index += 1
        return self.ledger.feed_frame(
            packets, connection=connection, life=life,
            record_index=10 + self.index, frame_index=self.index,
            server_ms=self.index * 7 if server is None else server,
            receipt_ms=100 + self.index * 8)

    def test_w_is_unknown_f_empty_is_explicit_and_neither_is_a_census(self):
        w = self.feed(b"W\x02\x03")[0]
        self.assertEqual((w.kind, w.sector, w.episode, w.cached_count),
                         ("W", (2, 3), 1, 0))
        self.assertTrue(w.cache_unknown)
        self.assertIsNone(w.listing_empty)
        empty = self.feed(b"F\x02\x03")[0]
        self.assertEqual((empty.entry_kind, empty.episode, empty.cached_count),
                         ("W_preceded", 1, 0))
        self.assertTrue(empty.listing_empty)
        self.assertFalse(empty.cache_unknown)
        self.assertFalse(empty.positive_reintroduction)
        self.assertEqual((self.ledger.counters.w_announcements,
                          self.ledger.counters.f_empty,
                          self.ledger.counters.f_nonempty,
                          self.ledger.counters.sector_episodes), (1, 1, 0, 1))

    def test_f_preserves_keys_sizes_colors_quarters_and_raw_source(self):
        event = self.feed(b"F\x02\x03\x09\x3f\x40\x0a\x07\xff\xc0\x14")[0]
        self.assertEqual([food.key for food in event.foods],
                         [(2, 3, 63, 64), (2, 3, 255, 192)])
        self.assertEqual([food.quarter for food in event.foods], [(0, 1), (3, 3)])
        self.assertEqual([(food.size_code, food.size, food.color) for food in event.foods],
                         [(10, 2.0, 9), (20, 4.0, 7)])
        self.assertEqual(asdict(event.source), dict(
            connection=1, life=1, epoch=1, record_index=11, frame_index=1,
            packet_index=0, server_ms=7, receipt_ms=108))
        self.assertEqual(event.entry_kind, "initial")
        self.assertEqual(event.cached_count, 2)
        self.assertFalse(event.entry_candidate)

    def test_f_is_additive_and_empty_listing_does_not_erase_cached_keys(self):
        first = self.feed(b"F\x02\x03\x00\x01\x02\x05")[0]
        second = self.feed(b"F\x02\x03\x00\x03\x04\x0a")[0]
        empty = self.feed(b"F\x02\x03")[0]
        self.assertEqual((first.cached_count, second.cached_count, empty.cached_count), (1, 2, 2))
        self.assertEqual((second.entry_kind, empty.entry_kind), ("repeated", "repeated"))
        self.assertTrue(empty.listing_empty)
        self.assertEqual(self.ledger.counters.repeated_f, 2)

    def test_clears_keep_notification_history_separate_from_cache(self):
        initial = self.feed(b"F\x02\x03\x00\x01\x02\x05")[0]
        pickup = self.feed(b"c\x02\x03\x01\x02")[0]
        self.assertEqual(pickup.cached_count, 0)
        clear = self.feed(b"w\x02\x03")[0]
        self.assertEqual((clear.clear_prior, clear.prior_announcement,
                          clear.prior_episode, clear.prior_cached_count),
                         ("known_F", "F", 1, 0))
        self.assertIsNone(clear.announcement)
        self.assertEqual(clear.last_F, initial.source)
        self.assertEqual(clear.last_w, clear.source)
        self.feed(b"W\x04\x05")
        w_only = self.feed(b"w\x04\x05")[0]
        self.assertEqual((w_only.clear_prior, w_only.prior_episode), ("W_only", 1))
        unknown = self.feed(b"w\x06\x07")[0]
        self.assertEqual((unknown.clear_prior, unknown.prior_episode), ("unknown", None))
        counts = self.ledger.counters
        self.assertEqual((counts.clears_prior_f, counts.clears_prior_w_only,
                          counts.clears_prior_unknown), (1, 1, 1))

    def test_w_after_f_does_not_erase_known_f_clear_stratum(self):
        self.feed(b"F\x02\x03", b"W\x02\x03")
        event = self.feed(b"w\x02\x03")[0]
        self.assertEqual((event.prior_announcement, event.clear_prior), ("W", "known_F"))

    def test_reintroductions_and_intervening_announcements_have_separate_episodes(self):
        self.feed(b"w\x02\x03")
        reintro = self.feed(b"F\x02\x03\x00\x01\x02\x05")[0]
        self.assertEqual((reintro.entry_kind, reintro.episode), ("after_clear", 1))
        self.assertTrue(reintro.positive_reintroduction)
        self.assertTrue(reintro.entry_candidate)
        self.feed(b"w\x02\x03")
        w = self.feed(b"W\x02\x03")[0]
        announced = self.feed(b"F\x02\x03\x00\x01\x02\x05")[0]
        self.assertEqual((w.episode, announced.episode, announced.entry_kind),
                         (2, 2, "W_preceded"))
        self.assertFalse(announced.positive_reintroduction)
        self.feed(b"w\x02\x03")
        empty = self.feed(b"F\x02\x03")[0]
        later = self.feed(b"F\x02\x03\x00\x01\x02\x05")[0]
        self.assertEqual((empty.episode, empty.entry_kind, later.entry_kind),
                         (3, "after_clear", "repeated"))
        self.assertFalse(empty.positive_reintroduction)
        self.assertFalse(later.positive_reintroduction)
        self.assertEqual(self.ledger.counters.positive_reintroductions, 1)

    def test_deltas_between_clear_and_f_do_not_become_announcement(self):
        self.feed(b"w\x02\x03")
        delta = self.feed(b"f\x02\x03\x01\x02\x05")[0]
        event = self.feed(b"F\x02\x03\x00\x01\x02\x05")[0]
        self.assertIsNone(delta.announcement)
        self.assertIsNone(delta.episode)
        self.assertTrue(event.positive_reintroduction)
        self.assertEqual(event.cached_count, 1)  # coordinate reuse, no new incarnation

    def test_same_frame_clear_f_is_ambiguous_in_either_order(self):
        self.feed(b"w\x02\x03")
        f, w = self.feed(b"F\x02\x03\x00\x01\x02\x05", b"w\x02\x03")
        self.assertEqual([e.source.packet_index for e in (f, w)], [0, 1])
        self.assertTrue(f.positive_reintroduction)
        self.assertFalse(f.entry_candidate)
        self.assertTrue(f.same_frame_clear_f and w.same_frame_clear_f)
        self.assertEqual((f.cached_count, w.prior_cached_count, w.cached_count), (1, 1, 0))
        w2, f2 = self.feed(b"w\x02\x03", b"F\x02\x03\x00\x03\x04\x05")
        self.assertEqual((w2.clear_prior, f2.episode), ("unknown", 2))
        self.assertTrue(w2.same_frame_clear_f and f2.same_frame_clear_f)
        self.assertFalse(f2.entry_candidate)
        self.assertEqual((self.ledger.counters.positive_reintroductions,
                          self.ledger.counters.unambiguous_positive_reintroductions), (2, 0))

    def test_same_frame_w_f_w_and_other_sectors_preserve_packet_order(self):
        self.feed(b"w\x02\x03")
        w, f, clear, unrelated = self.feed(
            b"W\x02\x03", b"F\x02\x03\x00\x01\x02\x05",
            b"w\x02\x03", b"F\x04\x05")
        self.assertEqual((w.episode, f.entry_kind, clear.prior_announcement),
                         (1, "W_preceded", "F"))
        self.assertFalse(f.positive_reintroduction)
        self.assertFalse(unrelated.same_frame_clear_f)
        self.assertEqual(unrelated.source.packet_index, 3)

    def test_all_valid_addition_forms_and_separate_compression_bases(self):
        packets = [b"f\x02\x03\x01\x02\x05", b"b\x03\x04\x09\x0a",
                   b"b\x04\x05\x06\x07\x08\x0f", b"f\x08\x09\x14"]
        events = self.feed(*packets)
        self.assertEqual([e.sector for e in events], [(2, 3), (2, 3), (4, 5), (4, 5)])
        self.assertEqual([e.sector_compressed for e in events], [False, True, False, True])
        self.assertEqual([e.foods[0].key for e in events],
                         [(2, 3, 1, 2), (2, 3, 3, 4), (4, 5, 6, 7), (4, 5, 8, 9)])
        self.assertEqual([e.foods[0].size for e in events], [1.0, 2.0, 3.0, 4.0])
        self.assertEqual([e.foods[0].color for e in events], [None, 9, 8, None])
        self.assertEqual([e.cached_count for e in events], [1, 2, 1, 2])
        self.assertTrue(all(e.cache_unknown for e in events))  # delta-only baselines
        unknown_eat = self.feed(b"c\x01\x02")[0]
        self.assertTrue(unknown_eat.sector_unknown)  # additions never seed pickups
        self.assertIsNone(unknown_eat.sector)

    def test_compressed_first_is_unknown_and_f_w_never_seed_delta_bases(self):
        self.feed(b"W\x02\x03", b"F\x04\x05", b"w\x06\x07")
        add, eat = self.feed(b"b\x01\x02\x05", b"<\x01\x02\x12\x34")
        self.assertTrue(add.sector_unknown and eat.sector_unknown)
        self.assertEqual((add.sector, add.local, add.size_code, add.cached_count),
                         (None, (1, 2), 5, None))
        self.assertEqual((eat.sector, eat.local, eat.eater), (None, (1, 2), 0x1234))
        self.assertEqual(self.ledger.counters.unknown_compressed_additions, 1)
        self.assertEqual(self.ledger.counters.unknown_compressed_pickups, 1)
        # An unlocated pickup may have affected any cache: keep uncertainty sticky.
        f = self.feed(b"F\x04\x05")[0]
        self.assertTrue(f.cache_unknown)

    def test_explicit_clear_resolves_unknown_cache_only_for_its_sector(self):
        self.feed(b"F\x02\x03", b"F\x04\x05", b"c\x01\x02")
        clear, other, resolved = self.feed(b"w\x02\x03", b"F\x04\x05", b"W\x02\x03")
        self.assertFalse(clear.cache_unknown)
        self.assertEqual(clear.cached_count, 0)
        self.assertTrue(other.cache_unknown)
        self.assertFalse(resolved.cache_unknown)
        again = self.feed(b"c\x01\x02", b"F\x02\x03")[1]
        self.assertTrue(again.cache_unknown)

    def test_all_pickup_forms_eater_compression_and_local_cache_only_clears(self):
        self.feed(b"F\x02\x03\x00\x01\x02\x05\x00\x03\x04\x05",
                  b"F\x04\x05\x00\x06\x07\x05")
        first, second = self.feed(b"<\x02\x03\x01\x02\x12\x34", b"C\x03\x04")
        self.assertEqual((first.food_key, first.eater, first.cached_count),
                         ((2, 3, 1, 2), 0x1234, 1))
        self.assertEqual((second.food_key, second.eater, second.cached_count),
                         ((2, 3, 3, 4), 0x1234, 0))
        third = self.feed(b"c\x04\x05\x06\x07")[0]
        self.assertIsNone(third.eater)
        self.assertEqual((third.sector, third.cached_count), ((4, 5), 0))
        fourth = self.feed(b"<\x06\x07\xab\xcd")[0]
        self.assertEqual((fourth.sector, fourth.eater), ((4, 5), 0xABCD))
        fifth = self.feed(b"C\x02\x03\x01\x02")[0]
        self.assertEqual((fifth.sector, fifth.eater), ((2, 3), 0xABCD))
        self.feed(b"f\x04\x05\x06\x07\x05", b"w\x02\x03")
        retained = self.feed(b"F\x04\x05")[0]
        self.assertEqual(retained.cached_count, 1)  # w is not a global food clear

    def test_c_and_C_before_explicit_eater_preserve_unknown_identity(self):
        for packet in (b"c\x02\x03\x01\x02", b"C\x01\x02"):
            event = self.feed(packet)[0]
            self.assertIsNone(event.eater)

    def test_reconnect_life_setup_and_game_over_do_not_join_histories(self):
        setup = b"a" + bytes(22) + b"\x13\x2a"
        for boundary in ("connection", "life", "setup", "death", "explicit_reset"):
            with self.subTest(boundary=boundary):
                self.setUp()
                self.feed(b"f\x02\x03\x01\x02\x05",
                          b"<\x02\x03\x01\x02\x12\x34", b"w\x02\x03")
                args = dict(connection=2) if boundary == "connection" else (
                    dict(life=2) if boundary == "life" else {})
                if boundary in ("setup", "death"):
                    self.feed(setup if boundary == "setup" else b"v")
                elif boundary == "explicit_reset":
                    self.ledger.reset()
                add, eat, f = self.feed(b"f\x01\x02\x05", b"C\x01\x02",
                                        b"F\x02\x03\x00\x01\x02\x05", **args)
                self.assertTrue(add.sector_unknown and eat.sector_unknown)
                self.assertIsNone(eat.eater)
                self.assertEqual((f.entry_kind, f.episode), ("initial", 1))
                self.assertIsNone(f.last_w)
                self.assertFalse(f.positive_reintroduction)

    def test_same_frame_boundary_separates_clear_listing_ambiguity(self):
        setup = b"a" + bytes(22) + b"\x13\x2a"
        w, reset, f = self.feed(b"w\x02\x03", setup, b"F\x02\x03\x00\x01\x02\x05")
        self.assertNotEqual(w.source.epoch, f.source.epoch)
        self.assertEqual(reset.kind, "reset")
        self.assertEqual(f.entry_kind, "initial")
        self.assertFalse(w.same_frame_clear_f or f.same_frame_clear_f)

    def test_bad_packet_in_later_position_is_atomic_even_across_new_life(self):
        self.feed(b"f\x02\x03\x01\x02\x05", b"w\x02\x03")
        malformed = [b"", b"W", b"W\x01\x02\x03", b"w\x01", b"w\x01\x02\x03",
                     b"F", b"F\x01", b"F\x01\x02\x03", b"F\x01\x02" + bytes(5),
                     b"f", b"f\x01\x02", b"f" + bytes(7), b"b\x01",
                     b"c", b"c\x01", b"C\x01\x02\x03", b"<\x01\x02",
                     b"<" + bytes(5), b"<" + bytes(7), b"a" + bytes(23)]
        for packet in malformed:
            with self.subTest(packet=packet):
                previous = copy.deepcopy(vars(self.ledger))
                with self.assertRaises(MalformedPacket):
                    self.feed(b"W\x08\x09", packet, life=2)
                self.assertEqual(vars(self.ledger), previous)
        resolved = self.feed(b"f\x03\x04\x05")[0]
        self.assertEqual(resolved.sector, (2, 3))

    def test_unsupported_protocol_is_separate_from_malformed_and_other_commands(self):
        previous = copy.deepcopy(vars(self.ledger))
        with self.assertRaises(UnsupportedPacket):
            self.feed(b"a" + bytes(22) + b"\x12\x2a")
        self.assertEqual(vars(self.ledger), previous)
        event = self.feed(b"p")[0]  # valid non-sector command
        self.assertEqual((event.kind, event.command, event.length), ("unsupported", "p", 1))
        self.assertEqual(self.ledger.counters.unsupported_packets, 1)

    def test_source_validation_and_unknown_server_clock_are_atomic(self):
        args = dict(connection=1, life=1, record_index=11, frame_index=1,
                    server_ms=None, receipt_ms=108)
        event = self.ledger.feed_frame([b"W\x02\x03"], **args)[0]
        self.assertIsNone(event.source.server_ms)
        for replacement in ({}, dict(receipt_ms=107), dict(record_index=-1),
                            dict(frame_index=0), dict(receipt_ms=float("nan")),
                            dict(server_ms=-1), dict(receipt_ms=None)):
            with self.subTest(replacement=replacement):
                previous = copy.deepcopy(vars(self.ledger))
                with self.assertRaises(ValueError):
                    self.ledger.feed_frame([b"w\x02\x03"], **(args | replacement))
                self.assertEqual(vars(self.ledger), previous)

    def test_setup_can_restart_server_clock_but_unexplained_regression_is_rejected(self):
        self.feed(b"W\x02\x03", server=100)
        previous = copy.deepcopy(vars(self.ledger))
        with self.assertRaisesRegex(ValueError, "server clock regressed"):
            self.feed(b"w\x02\x03", server=99)
        self.assertEqual(vars(self.ledger), previous)
        reset, f = self.feed(b"a" + bytes(22) + b"\x13\x2a", b"F\x02\x03", server=0)
        self.assertEqual((reset.kind, f.entry_kind), ("reset", "initial"))

    def test_events_are_snapshots_without_retained_output_history(self):
        first = self.feed(b"F\x02\x03\x00\x01\x02\x05")[0]
        original = asdict(first)
        for _ in range(30):
            self.feed(b"w\x02\x03", b"F\x02\x03\x00\x01\x02\x05")
        self.assertEqual(asdict(first), original)
        self.assertEqual(len(self.ledger._sectors), 1)
        self.assertEqual(len(self.ledger._sectors[(2, 3)].foods), 1)
        self.assertFalse(hasattr(self.ledger, "events"))


if __name__ == "__main__":
    unittest.main()
