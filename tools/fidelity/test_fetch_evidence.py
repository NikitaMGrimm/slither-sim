import contextlib
import email.message
import hashlib
import io
import json
from pathlib import Path
import tempfile
import unittest
import subprocess
import sys
from unittest import mock
import urllib.request
import urllib.response

import compare_evidence
import fetch_evidence

MANIFEST_URL = "https://raw.githubusercontent.com/owner/repo/0123abcd/comparisons/movement.json"
BASELINE = "https://github.com/owner/repo/releases/download/evidence-1/baseline.tar.gz"
CANDIDATE = "https://github.com/owner/repo/releases/download/evidence-2/candidate.tar.gz"
SIGNED = "https://release-assets.githubusercontent.com/github-production-release-asset/1/abc?sig=x&se=1"


def sha256(data):
    return hashlib.sha256(data).hexdigest()


class FakeHTTPS(urllib.request.BaseHandler):
    """Serves canned responses so tests exercise the real opener and redirect handling offline."""

    def __init__(self, routes):
        self.routes = routes
        self.requests = []

    def https_open(self, request):
        self.requests.append((request.full_url, {key.lower() for key, _ in request.header_items()}))
        status, body, headers = self.routes[request.full_url]
        message = email.message.Message()
        for key, value in headers.items():
            message[key] = value
        response = urllib.response.addinfourl(io.BytesIO(body), message, request.full_url, status)
        response.msg = "fake"
        return response


class FakeCompare:
    """Keeps the real submission validator but records comparison inputs instead of auditing bundles."""

    validate_submission = staticmethod(compare_evidence.validate_submission)

    def __init__(self):
        self.calls = []

    def compare_submission(self, submission, resolved, *, submission_sha256):
        self.calls.append((submission, {source: path.read_bytes() for source, path in resolved.items()},
                           submission_sha256))
        self.paths = list(resolved.values())
        return {"status": "inconclusive"}

    def write_report(self, output, report):
        output.mkdir()
        (output / "report.json").write_text(json.dumps(report), encoding="utf-8")


class CheckUrlTests(unittest.TestCase):
    def test_accepts_release_downloads_signed_assets_and_raw_manifests(self):
        for url in (BASELINE, SIGNED, "https://github.com:443/owner/repo/releases/download/v1/a.zip",
                    "https://github.com/user-attachments/files/12345/evidence.tar.gz",
                    "https://objects.githubusercontent.com/github-production-release-asset-2e65be/1/x?X-Amz=1",
                    "https://user-attachments.githubusercontent.com/assets/0f3c-uuid"):
            self.assertEqual(fetch_evidence.check_url(url, fetch_evidence.BUNDLE), url)
        self.assertEqual(fetch_evidence.check_url(MANIFEST_URL, fetch_evidence.MANIFEST), MANIFEST_URL)

    def test_rejects_unsafe_or_unexpected_urls(self):
        rejected = [
            "http://github.com/owner/repo/releases/download/v1/a.zip",
            "https://user:token@github.com/owner/repo/releases/download/v1/a.zip",
            "https://github.com/owner/repo/releases/download/v1/a.zip#frag",
            "https://github.com:8443/owner/repo/releases/download/v1/a.zip",
            "https://140.82.112.3/owner/repo/releases/download/v1/a.zip",
            "https://[::1]/owner/repo/releases/download/v1/a.zip",
            "https://localhost/owner/repo/releases/download/v1/a.zip",
            "https://github.com.evil.example/owner/repo/releases/download/v1/a.zip",
            "https://github.com/owner/repo/archive/refs/heads/main.zip",
            "https://github.com/user-attachments/files/not-a-number/evidence.tar.gz",
            "https://github.com/owner/repo/releases/latest/download/a.zip",
            "https://github.com/owner/repo/releases/download/v1/a.zip?token=1",
            "https://github.com/owner/repo/releases/download/../a.zip",
            "https://github.com/owner/repo/releases/download/v1%2F..%2Fx/a.zip",
            "https://github.com/owner/repo/releases/download/v1/a zip",
            "https://release-assets.githubusercontent.com/other/1/abc",
            "https://api.github.com/repos/owner/repo/releases/assets/1",
            MANIFEST_URL,  # raw content is a manifest location, never a bundle
            7,
        ]
        for url in rejected:
            with self.subTest(url=url), self.assertRaises(ValueError):
                fetch_evidence.check_url(url, fetch_evidence.BUNDLE)
        with self.assertRaises(ValueError):
            fetch_evidence.check_url("https://raw.githubusercontent.com/owner/repo/main/x.py", fetch_evidence.MANIFEST)


class FetchTests(unittest.TestCase):
    def test_download_rechecks_budget_between_partial_reads_and_before_connect(self):
        class Response:
            status = 200
            headers = {}
            def __init__(self):
                self.parts = iter((b"a", b"b", b""))
            def __enter__(self):
                return self
            def __exit__(self, *_):
                return False
            def read(self, _):
                raise AssertionError("fill-to-size read must not hide deadline checks")
            def read1(self, _):
                return next(self.parts)
        opener = mock.Mock()
        opener.open.return_value = Response()
        with mock.patch.object(fetch_evidence, "make_opener", return_value=opener):
            sink = io.BytesIO()
            fetch_evidence.download(MANIFEST_URL, fetch_evidence.MANIFEST, 100, sha256(b"ab"),
                                    sink, None, fetch_evidence.Budget())
            self.assertEqual(sink.getvalue(), b"ab")
            expired = fetch_evidence.Budget()
            expired.deadline = 0
            opener.open.reset_mock()
            with self.assertRaisesRegex(ValueError, "seconds"):
                fetch_evidence.download(MANIFEST_URL, fetch_evidence.MANIFEST, 100, sha256(b"ab"),
                                        io.BytesIO(), None, expired)
            opener.open.assert_not_called()

    def test_ci_isolated_python_can_import_the_trusted_sibling_tools(self):
        result = subprocess.run([sys.executable, "-I", str(Path(fetch_evidence.__file__)), "--help"],
                                capture_output=True, text=True, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("--submission-url", result.stdout)

    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.output = Path(temporary.name, "report")
        self.compare = FakeCompare()
        patcher = mock.patch.object(fetch_evidence, "compare_evidence", self.compare)
        patcher.start()
        self.addCleanup(patcher.stop)
        self.bundles = {BASELINE: b"baseline bundle", CANDIDATE: b"candidate bundle"}
        self.submission = {
            "schemaVersion": 1, "comparisonId": "prey-cadence", "independence": "separate days",
            "context": "matched controlled scenario", "alpha": 0.05, "minUnits": 4, "resamples": 999, "seed": 7,
            "metrics": [{"id": "prey-gap", "name": "preyServerGapMs", "minEffect": 0, "minSamples": 1}],
            "baseline": [{"source": BASELINE, "sha256": sha256(self.bundles[BASELINE])}],
            "candidate": [{"source": CANDIDATE, "sha256": sha256(self.bundles[CANDIDATE])}],
        }
        self.routes = {}
        self.serve_manifest()
        self.routes[BASELINE] = (302, b"", {"Location": SIGNED})
        self.routes[SIGNED] = (200, self.bundles[BASELINE], {"Content-Length": str(len(self.bundles[BASELINE]))})
        self.routes[CANDIDATE] = (200, self.bundles[CANDIDATE], {})

    def serve_manifest(self, data=None):
        self.manifest = json.dumps(self.submission).encode() if data is None else data
        self.routes[MANIFEST_URL] = (200, self.manifest, {})

    def fetch(self, digest=None):
        self.transport = FakeHTTPS(self.routes)
        return fetch_evidence.fetch_and_compare(MANIFEST_URL, digest or sha256(self.manifest), self.output,
                                                self.transport)

    def assert_fails(self, pattern, digest=None):
        with self.assertRaisesRegex(ValueError, pattern):
            self.fetch(digest)
        self.assertFalse(self.output.exists())

    def test_downloads_each_pinned_source_and_cleans_up(self):
        self.assertEqual(self.fetch(), {"status": "inconclusive"})
        [(submission, resolved, digest)] = self.compare.calls
        self.assertEqual(submission, self.submission)
        self.assertEqual(resolved, self.bundles)
        self.assertEqual(digest, sha256(self.manifest))
        self.assertEqual([url for url, _ in self.transport.requests], [MANIFEST_URL, BASELINE, SIGNED, CANDIDATE])
        for _, headers in self.transport.requests:
            self.assertFalse(headers & {"authorization", "cookie", "proxy-authorization"})
        self.assertTrue((self.output / "report.json").is_file())
        self.assertFalse(any(path.exists() for path in self.compare.paths))

    def test_refuses_existing_output_before_network(self):
        self.output.mkdir()
        with self.assertRaisesRegex(ValueError, "output already exists"):
            self.fetch()
        self.assertEqual(self.transport.requests, [])

    def test_hash_mismatch_stops_before_parsing(self):
        self.serve_manifest(b"not json")
        self.assert_fails("pinned SHA-256", digest="0" * 64)
        self.assert_fails("not UTF-8 JSON")
        self.assert_fails("lowercase hex", digest=sha256(self.manifest).upper())

    def test_rejects_ambiguous_json(self):
        self.serve_manifest(b'{"schemaVersion": 1, "schemaVersion": 1}')
        self.assert_fails("duplicate JSON key")
        self.serve_manifest(b'{"schemaVersion": NaN}')
        self.assert_fails("non-finite")

    def test_validates_every_redirect(self):
        for target in ("https://evil.example/a", "http://release-assets.githubusercontent.com/github-production-x/1",
                       "https://raw.githubusercontent.com/owner/repo/main/a.json"):
            with self.subTest(target=target):
                self.routes[BASELINE] = (302, b"", {"Location": target})
                self.assert_fails("allowed public GitHub|https without")
                self.assertNotIn(target, [url for url, _ in self.transport.requests])

    def test_bounds_redirect_chains(self):
        self.routes[BASELINE] = (302, b"", {"Location": SIGNED})
        self.routes[SIGNED] = (302, b"", {"Location": BASELINE})
        self.assert_fails("download failed")

    def test_rejects_non_ok_statuses(self):
        self.routes[CANDIDATE] = (206, self.bundles[CANDIDATE], {})
        self.assert_fails("HTTP status 206")
        self.routes[CANDIDATE] = (404, b"", {})
        self.assert_fails("download failed: HTTP Error 404")

    def test_bounds_manifest_bundle_and_total_sizes(self):
        self.serve_manifest(b" " * (fetch_evidence.MAX_MANIFEST + 1))
        self.assert_fails("exceeds 131072 bytes")
        self.serve_manifest()
        self.routes[CANDIDATE] = (200, b"", {"Content-Length": str(fetch_evidence.MAX_BUNDLE + 1)})
        self.assert_fails("exceeds 33554432 bytes")
        self.routes[CANDIDATE] = (200, self.bundles[CANDIDATE], {})
        with mock.patch.object(fetch_evidence, "MAX_BUNDLE", 8):
            self.assert_fails("exceeds 8 bytes")
        with mock.patch.object(fetch_evidence, "MAX_TOTAL", len(self.manifest) + 20):
            self.assert_fails("in total")

    def test_checks_sources_before_downloading_bundles(self):
        self.submission["candidate"][0]["source"] = MANIFEST_URL
        self.serve_manifest()
        self.assert_fails("allowed public GitHub")
        self.assertEqual([url for url, _ in self.transport.requests], [MANIFEST_URL])

    def test_cli_reports_untrusted_errors_on_one_quoted_line(self):
        self.serve_manifest(b'{"schemaVersion": 2}')
        self.compare.validate_submission = mock.Mock(side_effect=ValueError("bad\n::error::injected"))
        argv = ["--submission-url", MANIFEST_URL, "--sha256", sha256(self.manifest), "--output", str(self.output)]
        stderr = io.StringIO()
        with contextlib.redirect_stderr(stderr):
            self.assertEqual(fetch_evidence.main(argv, FakeHTTPS(self.routes)), 1)
        self.assertEqual(stderr.getvalue(), 'fetch_evidence: error: "bad\\n::error::injected"\n')
        self.serve_manifest()
        del self.compare.validate_submission
        argv[3] = sha256(self.manifest)
        self.assertEqual(fetch_evidence.main(argv, FakeHTTPS(self.routes)), 0)


if __name__ == "__main__":
    unittest.main()
