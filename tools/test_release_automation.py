import contextlib
import copy
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import release
import release_automation as automation

# Variables that point Git away from the working directory; hooks set GIT_DIR, for example.
GIT_LOCATION = ("GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_OBJECT_DIRECTORY",
                "GIT_ALTERNATE_OBJECT_DIRECTORIES", "GIT_COMMON_DIR", "GIT_NAMESPACE")


@contextlib.contextmanager
def fixture_repository():
    """A new temporary repository as `release.ROOT` that inherited Git variables cannot redirect."""
    with (tempfile.TemporaryDirectory() as directory, patch.dict(os.environ),
          patch.object(release, "ROOT", Path(directory))):
        for name in GIT_LOCATION:
            os.environ.pop(name, None)
        subprocess.run(["git", "init", "--quiet"], cwd=directory, check=True)
        git_dir = subprocess.check_output(["git", "rev-parse", "--absolute-git-dir"], cwd=directory, text=True)
        if not os.path.samefile(git_dir.strip(), Path(directory) / ".git"):
            raise AssertionError(f"fixture Git commands reached {git_dir.strip()}")
        yield directory


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
                    patch.object(automation, "automatic_ready", return_value=False), \
                    patch.object(automation, "manual_release_merge", return_value=None), \
                    patch.object(automation, "version_commit", return_value="c" * 40), \
                    patch.object(automation, "ensure_tag") as tag, \
                    patch.object(automation, "merge_patch") as merge:
                self.assertIsNone(automation.select_automatic())
                tag.assert_not_called()
                merge.assert_not_called()

    def test_automatic_retry_keeps_a_manual_merge_authorization(self):
        package = {**self.package, "version": "0.3.0"}
        commit = "c" * 40
        with patch.object(automation, "refresh_main"), \
                patch.object(release, "package_at", return_value=package), \
                patch.object(release, "registry_versions", return_value=[{"num": "0.2.0"}]), \
                patch.object(release, "github_pages", return_value=[{"tag_name": "v0.2.0", "draft": False}]), \
                patch.object(automation, "version_commit", return_value=commit), \
                patch.object(automation, "manual_release_merge", return_value={"merged_at": "2026-10-24T08:00:00Z"}), \
                patch.object(automation, "automatic_ready") as clock, \
                patch.object(release, "prepare") as identity, \
                patch.object(automation, "ensure_tag") as tag, \
                patch.object(subprocess, "run"):
            self.assertEqual(automation.select_automatic(), {"tag": "v0.3.0", "commit": commit})
            clock.assert_not_called()
            identity.assert_called_once_with("v0.3.0", package, commit)
            tag.assert_called_once_with("v0.3.0", commit)

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
        with fixture_repository() as directory:
            def git(*args):
                return subprocess.check_output(["git", *args], cwd=directory, text=True).strip()

            git("config", "user.name", "Test")
            git("config", "user.email", "test@example.invalid")
            root = Path(directory)
            manifest = '[package]\nname = "slither-sim"\nversion = "0.2.0"\n'
            lock = 'version = 4\n[[package]]\nname = "slither-sim"\nversion = "0.2.0"\n'
            (root / "Cargo.toml").write_text(manifest)
            (root / "Cargo.lock").write_text(lock)
            (root / "CHANGELOG.md").write_text("Initial\n")
            git("add", ".")
            git("commit", "--quiet", "-m", "initial")
            base = git("rev-parse", "HEAD")
            (root / "Cargo.toml").write_text(manifest.replace("0.2.0", "0.2.1"))
            (root / "Cargo.lock").write_text(lock.replace("0.2.0", "0.2.1"))
            (root / "CHANGELOG.md").write_text("Release 0.2.1\n")
            git("commit", "--quiet", "-am", "version bump")
            bump = git("rev-parse", "HEAD")
            (root / "Cargo.toml").write_text((root / "Cargo.toml").read_text() + 'description = "later"\n')
            git("commit", "--quiet", "-am", "later main change")
            git("update-ref", "refs/remotes/origin/main", "HEAD")
            self.assertEqual(automation.version_commit("0.2.1"), bump)
            self.assertEqual(automation.version_commit("0.2.0"), base)
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
                pr = {
                    "number": 1, "user": {"login": "github-actions[bot]"}, "draft": False,
                    "head": {"repo": {"full_name": "owner/repo"}, "ref": "release-plz-main"},
                    "base": {"ref": "main"}, "merged": True, "merge_commit_sha": bump,
                    "merged_by": {"type": "User"}, "merged_at": "2026-10-24T08:00:00Z",
                    "body": "<!-- release-plz slither-sim 0.2.0 0.2.1 compatible -->",
                }
                with patch.dict("os.environ", {"GH_REPO": "owner/repo"}), \
                        patch.object(release, "github_pages", return_value=[pr]), \
                        patch.object(release, "github", return_value=pr):
                    self.assertEqual(automation.manual_release_merge(bump), pr)
                    self.assertIsNone(automation.manual_release_merge(base))

    def test_inherited_workspace_versions_move_with_the_root_package(self):
        with fixture_repository() as directory:
            root = Path(directory)

            def git(*args):
                return subprocess.check_output(["git", *args], cwd=directory, text=True).strip()

            def commit(files, message):
                for name, text in files.items():
                    (root / name).parent.mkdir(parents=True, exist_ok=True)
                    (root / name).write_text(text)
                git("add", ".")
                git("commit", "--quiet", "-m", message)
                return git("rev-parse", "HEAD")

            def lock(root_version, ffi_version, tool_version="1.0.0"):
                return "version = 4\n" + "".join(
                    f'[[package]]\nname = "{name}"\nversion = "{version}"\n'
                    for name, version in (("slither-sim", root_version), ("slither-sim-ffi", ffi_version),
                                          ("tool", tool_version)))

            git("config", "user.name", "Test")
            git("config", "user.email", "test@example.invalid")
            manifest = ('[package]\nname = "slither-sim"\nversion.workspace = true\n'
                        '[workspace]\nmembers = ["ffi", "tool"]\n[workspace.package]\nversion = "0.2.0"\n')
            base = commit({
                "Cargo.toml": manifest,
                "ffi/Cargo.toml": '[package]\nname = "slither-sim-ffi"\nversion.workspace = true\n',
                "tool/Cargo.toml": '[package]\nname = "tool"\nversion = "1.0.0"\n',
                "Cargo.lock": lock("0.2.0", "0.2.0"),
                "CHANGELOG.md": "Initial\n",
            }, "initial")
            self.assertEqual(release.package_at(base)["version"], "0.2.0")
            bump = {"Cargo.toml": manifest.replace("0.2.0", "0.2.1"), "CHANGELOG.md": "Release 0.2.1\n"}
            heads = {}
            for label, lockfile in (("members follow", lock("0.2.1", "0.2.1")),
                                    ("member left behind", lock("0.2.1", "0.2.0")),
                                    ("independent member bumped", lock("0.2.1", "0.2.1", "0.2.1"))):
                git("checkout", "--quiet", "--detach", base)
                heads[label] = commit({**bump, "Cargo.lock": lockfile}, label)
            self.assertEqual(release.package_at(heads["members follow"])["version"], "0.2.1")
            original_run = subprocess.run

            def local_run(command, **kwargs):
                if command[:2] == ["git", "fetch"]:
                    return subprocess.CompletedProcess(command, 0)
                return original_run(command, **kwargs)

            with patch.object(subprocess, "run", side_effect=local_run):
                for label, expected in (("members follow", True), ("member left behind", False),
                                        ("independent member bumped", False)):
                    with self.subTest(label):
                        self.assertEqual(automation.metadata_only(base, heads[label], AutomationTests.package),
                                         expected)


class ManualReleaseTests(unittest.TestCase):
    commit = "c" * 40
    parent = "b" * 40

    def setUp(self):
        self.before = {"name": "slither-sim", "version": "0.2.1", "publish": ["crates-io"]}
        self.package = {**self.before, "version": "0.3.0"}
        self.files = "Cargo.toml\nCargo.lock\nCHANGELOG.md"
        self.pr = {
            "number": 42, "user": {"login": "github-actions[bot]"}, "draft": False,
            "head": {"repo": {"full_name": "owner/repo"}, "ref": "release-plz-main"},
            "base": {"ref": "main"}, "merged": True, "merge_commit_sha": self.commit,
            "merged_by": {"type": "User"}, "merged_at": "2026-10-24T08:00:00Z",
            "body": "<!-- release-plz slither-sim 0.2.1 0.3.0 incompatible -->",
        }
        patches = (
            patch.dict("os.environ", {"GH_REPO": "owner/repo"}),
            patch.object(automation, "git", side_effect=self.git),
            patch.object(release, "package_at", side_effect=lambda rev:
                         self.before if rev in (self.parent, f"{self.commit}^1") else self.package),
            patch.object(release, "github_pages", side_effect=lambda *_: [self.pr]),
            patch.object(release, "github", side_effect=lambda *_: self.pr),
            patch.object(automation, "metadata_only", return_value=True),
        )
        for item in patches:
            item.start()
            self.addCleanup(item.stop)

    def git(self, *args):
        if args == ("rev-list", "--parents", "-n", "1", self.commit):
            return f"{self.commit} {self.parent}"
        if args == ("diff", "--name-only", self.parent, self.commit):
            return self.files
        raise AssertionError(f"Unexpected git call: {args}")

    def test_manual_patch_minor_and_major_merges_authorize_the_exact_release(self):
        for version in ("0.2.2", "0.3.0", "1.0.0"):
            with self.subTest(version=version):
                self.package["version"] = version
                self.pr["body"] = f"<!-- release-plz slither-sim 0.2.1 {version} incompatible -->"
                self.assertEqual(automation.manual_release_merge(self.commit), self.pr)

    def test_ordinary_code_and_untrusted_or_bot_merges_grant_no_exception(self):
        valid = copy.deepcopy(self.pr)
        changes = ({"merged": False}, {"merged_by": {"type": "Bot"}}, {"merged_by": None},
                   {"merged_at": None}, {"merge_commit_sha": "d" * 40},
                   {"user": {"login": "someone"}}, {"draft": True},
                   {"head": {"repo": {"full_name": "fork/repo"}, "ref": "release-plz-main"}},
                   {"head": {"repo": {"full_name": "owner/repo"}, "ref": "feature/other"}},
                   {"base": {"ref": "develop"}}, {"body": ""},
                   {"body": "<!-- release-plz other 0.2.1 0.3.0 compatible -->"},
                   {"body": "<!-- release-plz slither-sim 0.2.0 0.3.0 compatible -->"},
                   {"body": "<!-- release-plz slither-sim 0.2.1 0.4.0 compatible -->"})
        for change in changes:
            with self.subTest(change=change):
                self.pr = {**copy.deepcopy(valid), **change}
                self.assertIsNone(automation.manual_release_merge(self.commit))
        self.pr = valid
        self.package["version"] = self.before["version"]
        with patch.object(release, "github_pages") as api:
            self.assertIsNone(automation.manual_release_merge(self.commit))
            api.assert_not_called()

    def test_source_edits_and_ambiguous_associations_are_rejected(self):
        self.files += "\nsrc/lib.rs"
        self.assertIsNone(automation.manual_release_merge(self.commit))
        self.files = "Cargo.toml\nCargo.lock\nCHANGELOG.md"
        with patch.object(automation, "metadata_only", return_value=False):
            self.assertIsNone(automation.manual_release_merge(self.commit))
        for associated in ([], [self.pr, self.pr]):
            with patch.object(release, "github_pages", return_value=associated):
                self.assertIsNone(automation.manual_release_merge(self.commit))

    def test_automatic_timer_uses_the_verified_manual_merge_time(self):
        with patch.object(automation, "version_commit", return_value=self.commit), \
                patch.object(release, "cooldown_ready", return_value=False) as clock:
            self.assertFalse(automation.automatic_ready(self.package))
            clock.assert_called_once_with("slither-sim", manual_release={
                "version": "0.3.0", "merged_at": "2026-10-24T08:00:00Z"})

    def test_upload_gate_keeps_identity_checks_and_only_skips_delay_for_a_manual_merge(self):
        for published, manual, elapsed, expected in ((True, False, False, "true"),
                                                     (False, True, False, "true"),
                                                     (False, False, False, "false"),
                                                     (False, False, True, "true")):
            with self.subTest(published=published, manual=manual, elapsed=elapsed), \
                    patch.object(release, "prepare", return_value={"already_published": str(published).lower()}) as identity, \
                    patch.object(automation, "manual_release_merge", return_value=self.pr if manual else None) as auth, \
                    patch.object(automation, "automatic_ready", return_value=elapsed) as clock:
                self.assertEqual(automation.publication_ready("v0.3.0", self.commit)["ready"], expected)
                identity.assert_called_once_with("v0.3.0", self.package, self.commit)
                self.assertEqual(auth.call_count, 0 if published else 1)
                self.assertEqual(clock.call_count, 0 if published or manual else 1)
                if not published and not manual:
                    clock.assert_called_once_with(self.before)
        with patch.object(release, "prepare", side_effect=ValueError("identity mismatch")), \
                patch.object(automation, "manual_release_merge") as auth:
            with self.assertRaisesRegex(ValueError, "identity mismatch"):
                automation.publication_ready("v0.3.0", self.commit)
            auth.assert_not_called()

    def test_manual_selection_uses_the_push_commit_even_after_main_advances(self):
        with patch.dict("os.environ", {"GITHUB_SHA": self.commit}), \
                patch.object(automation, "refresh_main"), \
                patch.object(automation, "git", return_value=self.commit) as git, \
                patch.object(automation, "manual_release_merge", return_value=self.pr) as auth, \
                patch.object(automation, "publication_target", return_value={"commit": self.commit}) as target:
            self.assertEqual(automation.select_merged(), {"commit": self.commit})
            git.assert_called_once_with("rev-parse", f"{self.commit}^{{commit}}")
            auth.assert_called_once_with(self.commit)
            target.assert_called_once_with(self.commit)
            auth.return_value = None
            target.reset_mock()
            self.assertIsNone(automation.select_merged())
            target.assert_not_called()

    def test_main_push_is_routed_to_merge_validation_and_tags_keep_their_identity(self):
        for ref, selected in (("refs/heads/main", True), ("refs/tags/v0.3.0", False)):
            with self.subTest(ref=ref), \
                    patch.dict("os.environ", {"GITHUB_EVENT_NAME": "push", "GITHUB_REF": ref,
                                              "GITHUB_REF_NAME": "v0.3.0", "RELEASE_TAG": ""}), \
                    patch("sys.argv", ["release_automation.py", "select"]), \
                    patch("sys.stdout", new_callable=io.StringIO) as output, \
                    patch.object(automation, "select_merged", return_value={
                        "tag": "v0.3.0", "commit": self.commit}) as merged, \
                    patch.object(automation, "git", return_value=self.commit), \
                    patch.object(automation, "select_automatic") as automatic:
                automation.main()
                self.assertIn(f"commit={self.commit}", output.getvalue())
                self.assertEqual(merged.call_count, int(selected))
                automatic.assert_not_called()


if __name__ == "__main__":
    unittest.main()
