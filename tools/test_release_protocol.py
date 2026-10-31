"""Offline publication guards for the independently versioned protocol crate."""
import os
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch
from urllib.error import HTTPError

import release
import release_automation as automation
from test_release_automation import fixture_repository


class ProtocolReleaseTests(unittest.TestCase):
    package = {"name": "slither-protocol", "version": "0.1.0", "publish": ["crates-io"]}

    def test_protocol_reuse_requires_clean_verified_registry_identity_and_identical_source(self):
        self.assertEqual(release.version_tag(self.package), "slither-protocol-v0.1.0")
        for same in (False, True):
            with patch.object(release, "published_commit", return_value="a" * 40), \
                    patch.object(release, "same_protocol_source", return_value=same) as source:
                if same:
                    result = release.prepare("slither-protocol-v0.1.0", self.package, "b" * 40)
                    self.assertEqual(result["already_published"], "true")
                else:
                    with self.assertRaisesRegex(ValueError, "different commit"):
                        release.prepare("slither-protocol-v0.1.0", self.package, "b" * 40)
                source.assert_called_once_with("a" * 40, "b" * 40)
        with patch.object(release, "published_commit", side_effect=ValueError("checksum mismatch")), \
                patch.object(release, "same_protocol_source") as source:
            with self.assertRaisesRegex(ValueError, "checksum"):
                release.prepare("slither-protocol-v0.1.0", self.package, "b" * 40)
            source.assert_not_called()

    def test_registry_visibility_wait_is_bounded_and_rechecks_archive(self):
        for responses, times, expected in (([b'', b'{"vers":"0.1.0"}'], [0, 1], True),
                                            ([b''], [0, 301], False)):
            with patch.object(release, "package_at", return_value=self.package), \
                    patch.object(release, "fetch", side_effect=responses), \
                    patch.object(release.time, "monotonic", side_effect=times), \
                    patch.object(release.time, "sleep"), \
                    patch.object(release, "prepare", return_value={"already_published": "true"}) as identity:
                if expected:
                    release.wait_protocol("b" * 40)
                    identity.assert_called_once_with("slither-protocol-v0.1.0", self.package, "b" * 40)
                else:
                    with self.assertRaises(TimeoutError):
                        release.wait_protocol("b" * 40)
                    identity.assert_not_called()
        with patch.object(release, "package_at", return_value=self.package), \
                patch.object(release, "fetch", side_effect=HTTPError("index", 403, "denied", {}, None)):
            with self.assertRaises(HTTPError):
                release.wait_protocol("b" * 40)

    def test_prerequisite_keeps_protocol_cooldown_and_selected_package(self):
        for ready, published in (("false", "false"), ("true", "false"), ("true", "true")):
            with patch.dict(os.environ, {"RELEASE_PACKAGE": "slither-sim"}), \
                    patch.object(automation, "protocol_dependency", return_value=True), \
                    patch.object(release, "package_at", return_value=self.package), \
                    patch.object(automation, "publication_ready", return_value={
                        "ready": ready, "already_published": published}) as gate:
                self.assertEqual(automation.protocol_prerequisite("b" * 40), {
                    "ready": ready, "protocol_needed": str(published == "false").lower()})
                gate.assert_called_once_with("slither-protocol-v0.1.0", "b" * 40)
                self.assertEqual(release.selected_name(), "slither-sim")

    def test_protocol_only_and_combined_metadata_do_not_authorize_source_or_dependency_changes(self):
        with fixture_repository() as directory:
            root = Path(directory)

            def git(*args):
                return subprocess.check_output(["git", *args], cwd=root, text=True).strip()

            def commit(files):
                for name, text in files.items():
                    (root / name).parent.mkdir(parents=True, exist_ok=True)
                    (root / name).write_text(text)
                git("add", ".")
                git("commit", "-qm", "fixture")
                return git("rev-parse", "HEAD")

            def lock(sim, protocol):
                return ''.join(f'[[package]]\nname = "{name}"\nversion = "{version}"\n'
                               for name, version in (("slither-sim", sim), ("slither-protocol", protocol)))

            git("config", "user.name", "Fixture")
            git("config", "user.email", "fixture@example.invalid")
            manifest = ('[package]\nname="slither-sim"\nversion.workspace=true\n'
                        '[workspace]\nmembers=["crates/slither-protocol"]\n'
                        '[workspace.package]\nversion="0.8.2"\n')
            protocol = '[package]\nname="slither-protocol"\nversion="0.1.0"\n'
            base = commit({"Cargo.toml": manifest, "crates/slither-protocol/Cargo.toml": protocol,
                           "Cargo.lock": lock("0.8.2", "0.1.0"), "CHANGELOG.md": "sim\n",
                           "crates/slither-protocol/CHANGELOG.md": "protocol\n"})
            original_run = subprocess.run

            def run(command, **kwargs):
                if command[:2] == ["git", "fetch"]:
                    return subprocess.CompletedProcess(command, 0)
                return original_run(command, **kwargs)

            for combined in (False, True):
                for incompatible in (False, True):
                    git("checkout", "-q", "--detach", base)
                    proposed = "0.2.0" if incompatible else "0.1.1"
                    files = {"crates/slither-protocol/Cargo.toml": protocol.replace("0.1.0", proposed),
                             "crates/slither-protocol/CHANGELOG.md": "next\n",
                             "Cargo.lock": lock("0.8.3" if combined else "0.8.2", proposed)}
                    if combined:
                        files.update({"Cargo.toml": manifest.replace("0.8.2", "0.8.3"), "CHANGELOG.md": "next\n"})
                    head = commit(files)
                    with patch.object(subprocess, "run", side_effect=run):
                        self.assertEqual(automation.metadata_only(base, head, self.package), not incompatible)
                        self.assertTrue(automation.metadata_only(base, head, self.package, patch_only=False))
                        markers = [("slither-protocol", "0.1.0", proposed, "compatible")]
                        if combined:
                            markers.append(("slither-sim", "0.8.2", "0.8.3", "compatible"))
                        self.assertTrue(automation.metadata_only(base, head, self.package,
                                                                patch_only=False, markers=markers))
                        for forged in ([(*markers[0][:2], "9.0.0", "compatible"), *markers[1:]],
                                       [*markers, markers[0]]):
                            self.assertFalse(automation.metadata_only(base, head, self.package,
                                                                     patch_only=False, markers=forged))
                    bad = commit({"crates/slither-protocol/src/lib.rs": "// extra source\n"})
                    with patch.object(subprocess, "run", side_effect=run):
                        self.assertFalse(automation.metadata_only(base, bad, self.package, patch_only=False))

    def test_independent_selection_falls_back_after_simulator_is_complete(self):
        with patch.dict(os.environ, {"GITHUB_EVENT_NAME": "schedule", "RELEASE_TAG": ""}), \
                patch("sys.argv", ["release_automation.py", "select"]), \
                patch("builtins.print") as output, \
                patch.object(automation, "select_automatic", side_effect=[None, {
                    "tag": "slither-protocol-v0.1.1", "commit": "b" * 40}]):
            automation.main()
            output.assert_any_call("package=slither-protocol")

    def test_first_protocol_import_uses_cooldown_without_reading_an_absent_parent_manifest(self):
        with fixture_repository() as directory, automation.selected_package(release.PROTOCOL):
            root = Path(directory)

            def git(*args):
                return subprocess.check_output(["git", *args], cwd=root, text=True).strip()

            git("config", "user.name", "Fixture")
            git("config", "user.email", "fixture@example.invalid")
            (root / "Cargo.toml").write_text('[package]\nname="slither-sim"\nversion="0.8.2"\n')
            git("add", ".")
            git("commit", "-qm", "before bootstrap")
            path = root / release.MANIFESTS[release.PROTOCOL]
            path.parent.mkdir(parents=True)
            path.write_text('[package]\nname="slither-protocol"\nversion="0.1.0"\npublish=["crates-io"]\n')
            git("add", ".")
            git("commit", "-qm", "bootstrap")
            bootstrap = git("rev-parse", "HEAD")
            self.assertIsNone(automation.manual_release_merge(bootstrap))
            with patch.object(automation, "version_commit", return_value=bootstrap), \
                    patch.object(release, "cooldown_ready", return_value=False) as cooldown:
                self.assertFalse(automation.automatic_ready(self.package))
                cooldown.assert_called_once_with("slither-protocol", manual_release=None)


if __name__ == "__main__":
    unittest.main()
