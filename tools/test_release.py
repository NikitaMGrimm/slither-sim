import hashlib
import io
import json
import subprocess
import tarfile
import unittest
from unittest.mock import patch
from urllib.error import HTTPError

import release


class ReleaseTests(unittest.TestCase):
    package = {"name": "slither-sim", "version": "0.1.0", "publish": ["crates-io"]}
    commit = "a" * 40

    def test_ci_lookup_waits_for_the_exact_main_run(self):
        for replies in (["123\n"], ["", "123\n"]):
            with self.subTest(replies=replies), \
                    patch.object(release.subprocess, "run") as run, \
                    patch.object(release.subprocess, "check_output", side_effect=replies) as lookup, \
                    patch.object(release.time, "monotonic", return_value=0), \
                    patch.object(release.time, "sleep") as sleep:
                release.require_main_ci(self.commit)
                self.assertEqual(run.call_args_list[0].args[0],
                                 ["git", "merge-base", "--is-ancestor", self.commit, "origin/main"])
                self.assertEqual(run.call_args_list[1].args[0],
                                 ["gh", "run", "watch", "123", "--exit-status", "--interval", "15"])
                self.assertEqual(lookup.call_count, len(replies))
                for call in lookup.call_args_list:
                    self.assertEqual(call.args[0], [
                        "gh", "run", "list", "--workflow", "checks.yml", "--event", "push", "--branch", "main",
                        "--commit", self.commit, "--limit", "1", "--json", "databaseId", "--jq", ".[0].databaseId // empty",
                    ])
                self.assertEqual(sleep.call_count, len(replies) - 1)

    def test_missing_ci_fails_at_the_deadline_without_watching(self):
        with patch.object(release.subprocess, "run") as run, \
                patch.object(release.subprocess, "check_output", return_value=""), \
                patch.object(release.time, "monotonic", side_effect=[0, 298, 300]), \
                patch.object(release.time, "sleep") as sleep:
            with self.assertRaisesRegex(TimeoutError, "within 300 seconds"):
                release.require_main_ci(self.commit)
            sleep.assert_called_once_with(2)
            self.assertEqual(run.call_count, 1)

    def test_ci_errors_fail_without_retry(self):
        for failing_stage in ("ancestry", "lookup", "watch"):
            with self.subTest(stage=failing_stage), \
                    patch.object(release.subprocess, "run") as run, \
                    patch.object(release.subprocess, "check_output", return_value="123\n") as lookup, \
                    patch.object(release.time, "sleep") as sleep:
                error = subprocess.CalledProcessError(1, failing_stage)
                if failing_stage == "ancestry":
                    run.side_effect = error
                elif failing_stage == "lookup":
                    lookup.side_effect = error
                else:
                    run.side_effect = [None, error]
                with self.assertRaises(subprocess.CalledProcessError):
                    release.require_main_ci(self.commit)
                self.assertEqual(lookup.call_count, 0 if failing_stage == "ancestry" else 1)
                self.assertEqual(run.call_count, 2 if failing_stage == "watch" else 1)
                sleep.assert_not_called()

    def archive(self, **git):
        data = json.dumps({"git": {"sha1": self.commit, **git}}).encode()
        output = io.BytesIO()
        with tarfile.open(fileobj=output, mode="w:gz") as archive:
            member = tarfile.TarInfo("slither-sim-0.1.0/.cargo_vcs_info.json")
            member.size = len(data)
            archive.addfile(member, io.BytesIO(data))
        return output.getvalue()

    def test_wrong_tag_and_registry_fail_before_network(self):
        with patch.object(release, "published_commit") as network:
            for tag in ("main", "v0.2.0", "0.1.0", "v0.1.0\n"):
                with self.assertRaisesRegex(ValueError, "Tag must"):
                    release.prepare(tag, self.package, self.commit)
            for registry in (False, True, ["other"], None):
                with self.assertRaisesRegex(ValueError, "restrict publishing"):
                    release.prepare("v0.1.0", {**self.package, "publish": registry}, self.commit)
            network.assert_not_called()

    def test_new_version_can_publish_and_same_commit_can_resume(self):
        for existing, expected in ((None, "false"), (self.commit, "true")):
            with patch.object(release, "published_commit", return_value=existing):
                self.assertEqual(release.prepare("v0.1.0", self.package, self.commit),
                                 {"version": "0.1.0", "already_published": expected, "prerelease": "false"})
        with patch.object(release, "published_commit", return_value="b" * 40):
            with self.assertRaisesRegex(ValueError, "different commit"):
                release.prepare("v0.1.0", self.package, self.commit)

    def test_build_metadata_does_not_make_a_stable_release_prerelease(self):
        for version, expected in (("0.1.1", "false"), ("0.1.1-rc.1", "true"),
                                  ("0.1.1+build-1", "false"), ("0.1.1-rc.1+build-1", "true")):
            with patch.object(release, "published_commit", return_value=None):
                result = release.prepare(f"v{version}", {**self.package, "version": version}, self.commit)
                self.assertEqual(result["prerelease"], expected)

    def test_only_a_registry_404_means_unpublished(self):
        for status in (404, 403, 429, 500):
            with patch.object(release, "fetch", side_effect=HTTPError("url", status, "error", {}, None)):
                if status == 404:
                    self.assertIsNone(release.published_commit("slither-sim", "0.1.0"))
                else:
                    with self.assertRaises(HTTPError):
                        release.published_commit("slither-sim", "0.1.0")

    def test_archive_identity_is_checked_without_extracting_files(self):
        for dirty in (False, True):
            archive = self.archive(dirty=dirty)
            metadata = json.dumps({"version": {"checksum": hashlib.sha256(archive).hexdigest()}}).encode()
            with patch.object(release, "fetch", side_effect=[metadata, archive]):
                if dirty:
                    with self.assertRaisesRegex(ValueError, "dirty"):
                        release.published_commit("slither-sim", "0.1.0")
                else:
                    self.assertEqual(release.published_commit("slither-sim", "0.1.0"), self.commit)
        metadata = json.dumps({"version": {"checksum": "0" * 64}}).encode()
        with patch.object(release, "fetch", side_effect=[metadata, self.archive()]):
            with self.assertRaisesRegex(ValueError, "checksum"):
                release.published_commit("slither-sim", "0.1.0")


if __name__ == "__main__":
    unittest.main()
