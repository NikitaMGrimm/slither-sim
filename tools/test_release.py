import hashlib
import io
import json
import tarfile
import unittest
from unittest.mock import patch
from urllib.error import HTTPError

import release


class ReleaseTests(unittest.TestCase):
    package = {"name": "slither-sim", "version": "0.1.0", "publish": ["crates-io"]}
    commit = "a" * 40

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
