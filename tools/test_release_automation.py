import copy
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import release
import release_automation as automation


class AutomationTests(unittest.TestCase):
    package = {"name": "slither-sim", "version": "0.2.0", "publish": ["crates-io"]}

    def setUp(self):
        self.env = patch.dict("os.environ", {"GH_REPO": "owner/repo"})
        self.env.start()
        self.addCleanup(self.env.stop)
        self.pr = {
            "number": 1, "user": {"login": "github-actions[bot]"}, "draft": False,
            "head": {"repo": {"full_name": "owner/repo"}, "ref": "release-plz-main", "sha": "a" * 40},
            "base": {"ref": "main", "sha": "b" * 40}, "mergeable_state": "clean",
            "body": "<!-- release-plz slither-sim 0.2.0 0.2.1 compatible -->",
        }
        self.files = [{"filename": value} for value in ("Cargo.toml", "Cargo.lock", "CHANGELOG.md")]

    def eligible(self, pr=None, files=None):
        return automation.eligible_patch(pr or self.pr, self.package, files or self.files, "b" * 40)

    def test_only_next_stable_patch_is_automatic(self):
        for proposed, expected in (("0.2.1", True), ("0.3.0", False), ("1.0.0", False),
                                   ("0.2.2", False), ("0.2.1-rc.1", False), ("0.2.1+build", False)):
            with self.subTest(proposed=proposed):
                self.assertEqual(automation.is_patch("0.2.0", proposed), expected)
        self.assertTrue(self.eligible())

    def test_skipped_breaking_stale_and_untrusted_prs_wait_for_review(self):
        for field, value in (("body", "<!-- release-plz slither-sim 0.2.0 0.2.1 skipped -->"),
                             ("body", "<!-- release-plz slither-sim 0.2.0 0.3.0 incompatible -->"),
                             ("body", ""), ("draft", True), ("mergeable_state", "behind"),
                             ("user", {"login": "someone"}),
                             ("base", {"ref": "main", "sha": "c" * 40})):
            with self.subTest(field=field, value=value):
                pr = {**self.pr, field: value}
                self.assertFalse(self.eligible(pr))
        pr = copy.deepcopy(self.pr)
        pr["head"]["repo"]["full_name"] = "fork/repo"
        self.assertFalse(self.eligible(pr))
        self.assertFalse(self.eligible(files=self.files + [{"filename": "src/lib.rs"}]))

    def test_protected_merge_is_pinned_and_errors_propagate(self):
        for rejected in (False, True):
            with self.subTest(rejected=rejected), \
                    patch.object(release, "github_pages", side_effect=[[self.pr], self.files]), \
                    patch.object(release, "github", side_effect=[self.pr, subprocess.CalledProcessError(1, "merge")]
                                 if rejected else [self.pr, {"merged": True, "sha": "c" * 40}]) as api, \
                    patch.object(automation, "git", return_value="b" * 40), \
                    patch.object(automation, "metadata_only", return_value=True), \
                    patch.object(automation, "ensure_pr_checks"):
                if rejected:
                    with self.assertRaises(subprocess.CalledProcessError):
                        automation.merge_patch(self.package)
                else:
                    self.assertEqual(automation.merge_patch(self.package), "c" * 40)
                self.assertIn(f"sha={self.pr['head']['sha']}", api.call_args.args)
                self.assertIn("merge_method=squash", api.call_args.args)

    def test_wrong_tag_is_never_moved_and_dispatch_can_resume(self):
        with patch.object(automation, "git", side_effect=["v0.2.1", "b" * 40]), \
                patch.object(release, "github") as api:
            with self.assertRaisesRegex(ValueError, "different commit"):
                automation.ensure_tag("v0.2.1", "a" * 40)
            api.assert_not_called()
        for existing in (False, True):
            with patch.object(automation, "git", side_effect=["v0.2.1", "a" * 40] if existing else [""]), \
                    patch.object(release, "github") as api:
                automation.ensure_tag("v0.2.1", "a" * 40)
                self.assertEqual(api.call_count, 0 if existing else 1)

    def test_partial_upload_repairs_original_commit_even_after_main_version_advanced(self):
        published = "a" * 40
        with patch.object(automation, "refresh_main"), \
                patch.object(release, "package_at", side_effect=[{**self.package, "version": "0.3.0"},
                                                               self.package, self.package]), \
                patch.object(release, "registry_versions", return_value=[
                    {"num": "0.2.0", "created_at": "2026-10-23T14:00:00Z"}]), \
                patch.object(release, "github_pages", return_value=[]), \
                patch.object(release, "published_commit", return_value=published), \
                patch.object(release, "cooldown_ready") as cooldown, \
                patch.object(release, "prepare") as identity, \
                patch.object(automation, "ensure_tag") as tag, \
                patch.object(automation, "merge_patch") as merge, \
                patch.object(subprocess, "run"):
            self.assertEqual(automation.select_automatic(), {"tag": "v0.2.0", "commit": published})
            cooldown.assert_not_called()
            merge.assert_not_called()
            identity.assert_called_once_with("v0.2.0", self.package, published)
            tag.assert_called_once_with("v0.2.0", published)

    def test_cooldown_prevents_new_tag_or_merge(self):
        for version in ("0.2.0", "0.2.1"):
            with patch.object(automation, "refresh_main"), \
                    patch.object(release, "package_at", return_value={**self.package, "version": version}), \
                    patch.object(release, "registry_versions", return_value=[{"num": "0.2.0"}]), \
                    patch.object(release, "github_pages", return_value=[{"tag_name": "v0.2.0", "draft": False}]), \
                    patch.object(release, "cooldown_ready", return_value=False), \
                    patch.object(automation, "ensure_tag") as tag, \
                    patch.object(automation, "merge_patch") as merge:
                self.assertIsNone(automation.select_automatic())
                tag.assert_not_called()
                merge.assert_not_called()

    def ci_setup(self, metadata=True):
        for mocked in (patch.object(automation, "refresh_main"),
                       patch.object(automation, "git", return_value="b" * 40),
                       patch.object(release, "github_pages", return_value=self.files),
                       patch.object(release, "package_at", return_value=self.package),
                       patch.object(automation, "metadata_only", return_value=metadata)):
            mocked.start()
            self.addCleanup(mocked.stop)

    def test_only_pending_native_pr_ci_is_approved(self):
        self.ci_setup()
        for conclusion in ("action_required", "", "success", "failure"):
            with self.subTest(conclusion=conclusion), \
                    patch.object(subprocess, "check_output", return_value=json.dumps([
                        {"databaseId": 42, "conclusion": conclusion}])), \
                    patch.object(release, "github", return_value={
                        "event": "pull_request", "head_sha": self.pr["head"]["sha"],
                        "pull_requests": [{"number": self.pr["number"]}]}), \
                    patch.object(subprocess, "run") as approve:
                automation.ensure_pr_checks(self.pr)
                self.assertEqual(approve.call_count, 1 if conclusion == "action_required" else 0)
                if conclusion == "action_required":
                    self.assertEqual(approve.call_args.args[0], [
                        "gh", "api", "--method", "POST", "repos/owner/repo/actions/runs/42/approve"])

    def test_source_edits_do_not_receive_ci_approval(self):
        self.ci_setup(metadata=False)
        with patch.object(subprocess, "check_output") as lookup, \
                patch.object(subprocess, "run") as approve:
            automation.ensure_pr_checks(self.pr)
            lookup.assert_not_called()
            approve.assert_not_called()

    def test_ci_approval_rejects_a_different_run_identity(self):
        self.ci_setup()
        valid = {"event": "pull_request", "head_sha": self.pr["head"]["sha"],
                 "pull_requests": [{"number": self.pr["number"]}]}
        for changes in ({"event": "workflow_dispatch"}, {"head_sha": "c" * 40},
                        {"pull_requests": [{"number": 99}]}):
            with self.subTest(changes=changes), \
                    patch.object(subprocess, "check_output", return_value='[{"databaseId":42,"conclusion":"action_required"}]'), \
                    patch.object(release, "github", return_value={**valid, **changes}), \
                    patch.object(subprocess, "run") as approve:
                with self.assertRaisesRegex(ValueError, "does not match"):
                    automation.ensure_pr_checks(self.pr)
                approve.assert_not_called()

    def test_ci_appearance_wait_is_bounded(self):
        self.ci_setup()
        for replies, waits, timed_out in ((["[]", '[{"databaseId":42,"conclusion":""}]'], 1, False),
                                         (["[]"] * 7, 6, True)):
            with self.subTest(timed_out=timed_out), \
                    patch.object(subprocess, "check_output", side_effect=replies), \
                    patch.object(automation.time, "sleep") as sleep:
                if timed_out:
                    with self.assertRaises(TimeoutError):
                        automation.ensure_pr_checks(self.pr)
                else:
                    automation.ensure_pr_checks(self.pr)
                self.assertEqual(sleep.call_count, waits)


class GitIntegrationTests(unittest.TestCase):
    def test_original_version_commit_and_metadata_validation_with_real_git_history(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(release, "ROOT", Path(directory)):
            def git(*args):
                return subprocess.check_output(["git", *args], cwd=directory, text=True).strip()

            git("init", "--quiet")
            git("config", "user.name", "Test")
            git("config", "user.email", "test@example.invalid")
            root = Path(directory)
            manifest = '[package]\nname = "slither-sim"\nversion = "0.2.0"\n'
            lock = 'version = 4\n[[package]]\nname = "slither-sim"\nversion = "0.2.0"\n'
            (root / "Cargo.toml").write_text(manifest)
            (root / "Cargo.lock").write_text(lock)
            git("add", ".")
            git("commit", "--quiet", "-m", "initial")
            base = git("rev-parse", "HEAD")
            (root / "Cargo.toml").write_text(manifest.replace("0.2.0", "0.2.1"))
            (root / "Cargo.lock").write_text(lock.replace("0.2.0", "0.2.1"))
            git("commit", "--quiet", "-am", "version bump")
            bump = git("rev-parse", "HEAD")
            (root / "Cargo.toml").write_text((root / "Cargo.toml").read_text() + 'description = "later"\n')
            git("commit", "--quiet", "-am", "later main change")
            git("update-ref", "refs/remotes/origin/main", "HEAD")
            self.assertEqual(automation.version_commit("0.2.1"), bump)
            # The only mocked Git operation is network fetch; all ancestry and TOML comparisons are real.
            original_run = subprocess.run

            def local_run(command, **kwargs):
                if command[:2] == ["git", "fetch"]:
                    return subprocess.CompletedProcess(command, 0)
                return original_run(command, **kwargs)

            with patch.object(subprocess, "run", side_effect=local_run):
                self.assertTrue(automation.metadata_only(base, bump, AutomationTests.package))
                self.assertFalse(automation.metadata_only(base, "HEAD", AutomationTests.package))
                self.assertFalse(automation.metadata_only(bump, base, {**AutomationTests.package, "version": "0.2.1"}))


if __name__ == "__main__":
    unittest.main()
