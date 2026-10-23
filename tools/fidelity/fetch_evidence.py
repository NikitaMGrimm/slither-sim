"""Download a pinned comparison submission and its bundles from public GitHub hosts, then compare them.

Every URL, including each redirect, must name an allowed GitHub host and path. Bytes are hashed
as received and checked against the pinned SHA-256 before anything parses them; bundles stay
unopened files here and compare_evidence owns their audit.
"""

import argparse
import hashlib
import http.client
import ipaddress
import json
import os
from pathlib import Path
import re
import ssl
import sys
import tempfile
import time
import urllib.parse
import urllib.request

# -I removes the script directory too. Restore only this trusted checkout's
# sibling tools, keeping PYTHONPATH and the invoking directory excluded in CI.
sys.path.insert(0, str(Path(__file__).resolve().parent))
import compare_evidence  # noqa: E402

MAX_MANIFEST = 128 * 1024
MAX_BUNDLE = 32 * 1024 * 1024
MAX_TOTAL = 128 * 1024 * 1024
MAX_SOURCES = 64
MAX_URL = 8192
MAX_REDIRECTS = 5
TIMEOUT = 30
DEADLINE = 600
CHUNK = 64 * 1024
SHA256 = re.compile(r"[0-9a-f]{64}")
SEGMENT = re.compile(r"[A-Za-z0-9._~%+=-]+")
RELEASE = re.compile(r"/[A-Za-z0-9-]+/[A-Za-z0-9._-]+/releases/download/[^/]+/[^/]+")
ATTACHMENT = re.compile(r"/user-attachments/files/[0-9]+/[^/]+")
# Release download and attachment hosts serve signed, short-lived redirect targets.
CDN_HOSTS = {
    "release-assets.githubusercontent.com": "github-production-",
    "objects.githubusercontent.com": "github-production-",
    "user-attachments.githubusercontent.com": "",
}
MANIFEST, BUNDLE = "manifest", "bundle"


def check_url(url, role):
    """Return url if it is an allowed public GitHub artifact URL for role, else raise ValueError."""
    if not isinstance(url, str) or len(url) > MAX_URL or not re.fullmatch(r"[\x21-\x7e]+", url) or "\\" in url:
        raise ValueError("URL must be printable ASCII without spaces or backslashes")
    parts = urllib.parse.urlsplit(url)
    try:
        port = parts.port
    except ValueError as error:
        raise ValueError("URL has an invalid port") from error
    host = parts.hostname or ""
    if parts.scheme != "https" or "@" in parts.netloc or "#" in url or port not in (None, 443):
        raise ValueError("URL must be https without credentials, fragment or a non-443 port")
    try:
        ipaddress.ip_address(host)
    except ValueError:
        pass
    else:
        raise ValueError("URL must not name an IP address")
    path = parts.path
    segments = path.split("/")[1:]
    if (not path.startswith("/") or re.search(r"%(2[ef]|5c)", path, re.IGNORECASE)
            or any(not SEGMENT.fullmatch(segment) or segment in (".", "..") for segment in segments)):
        raise ValueError("URL path contains unexpected segments")
    if host == "github.com" and not parts.query and (RELEASE.fullmatch(path) or ATTACHMENT.fullmatch(path)):
        return url
    if host == "raw.githubusercontent.com" and role == MANIFEST and not parts.query:
        if len(segments) >= 4 and segments[-1].endswith(".json"):
            return url
    if host in CDN_HOSTS and len(segments) >= 2 and segments[0].startswith(CDN_HOSTS[host]):
        return url
    raise ValueError("URL is not an allowed public GitHub release or attachment location")


class RedirectGuard(urllib.request.HTTPRedirectHandler):
    max_redirections = MAX_REDIRECTS

    def __init__(self, role):
        self.role = role

    def redirect_request(self, req, fp, code, msg, headers, newurl):
        check_url(newurl, self.role)
        return super().redirect_request(req, fp, code, msg, headers, newurl)


def https_transport():
    return urllib.request.HTTPSHandler(context=ssl.create_default_context())


def make_opener(role, transport):
    """Build an opener with only HTTPS, guarded redirects and error handling: no proxies, cookies or auth."""
    opener = urllib.request.OpenerDirector()
    for handler in (transport, RedirectGuard(role), urllib.request.HTTPDefaultErrorHandler(),
                    urllib.request.HTTPErrorProcessor()):
        opener.add_handler(handler)
    return opener


class Budget:
    def __init__(self):
        self.remaining = MAX_TOTAL
        self.deadline = time.monotonic() + DEADLINE

    def take(self, size):
        if size > self.remaining:
            raise ValueError(f"downloads exceed {MAX_TOTAL} bytes in total")
        if time.monotonic() > self.deadline:
            raise ValueError(f"downloads exceed {DEADLINE} seconds")
        self.remaining -= size


def download(url, role, limit, expected, sink, transport, budget):
    """Stream url into sink, bounded by limit and budget, and require the exact SHA-256 expected."""
    check_url(url, role)
    budget.take(0)
    request = urllib.request.Request(url, headers={"User-Agent": "slither-sim-evidence", "Accept-Encoding": "identity"})
    digest = hashlib.sha256()
    size = 0
    try:
        with make_opener(role, transport).open(request, timeout=TIMEOUT) as response:
            if response.status != 200:
                raise ValueError(f"unexpected HTTP status {response.status}")
            length = response.headers.get("Content-Length")
            if length is not None and (not length.isdigit() or int(length) > limit):
                raise ValueError(f"download exceeds {limit} bytes")
            # HTTPResponse.read(n) can wait to fill n across many socket reads.
            # read1 returns after one buffered read so the deadline is revisited.
            read = getattr(response, "read1", response.read)
            while chunk := read(CHUNK):
                size += len(chunk)
                if size > limit:
                    raise ValueError(f"download exceeds {limit} bytes")
                budget.take(len(chunk))
                digest.update(chunk)
                sink.write(chunk)
    except (http.client.HTTPException, OSError) as error:
        raise ValueError(f"download failed: {error}") from error
    if digest.hexdigest() != expected:
        raise ValueError("download does not match its pinned SHA-256")


def _pairs(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def _constant(value):
    raise ValueError(f"non-finite JSON number: {value}")


def parse_submission(data):
    try:
        value = json.loads(data.decode("utf-8"), object_pairs_hook=_pairs, parse_constant=_constant)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ValueError(f"submission is not UTF-8 JSON: {error}") from error
    return compare_evidence.validate_submission(value)


def bundle_sources(submission):
    """Check every source URL before any bundle download; validate_submission already rejects duplicates."""
    entries = submission["baseline"] + submission["candidate"]
    if len(entries) > MAX_SOURCES:
        raise ValueError(f"submission names more than {MAX_SOURCES} sources")
    return {check_url(entry["source"], BUNDLE): entry["sha256"] for entry in entries}


def fetch_and_compare(url, sha256, output, transport=None):
    if not SHA256.fullmatch(sha256):
        raise ValueError("submission SHA-256 must be 64 lowercase hex digits")
    output = Path(output)
    if os.path.lexists(output):
        raise ValueError("output already exists")
    transport = transport or https_transport()
    budget = Budget()
    with tempfile.TemporaryDirectory(prefix="evidence-fetch-") as directory:
        manifest = Path(directory, "submission.json")
        with manifest.open("xb") as sink:
            download(url, MANIFEST, MAX_MANIFEST, sha256, sink, transport, budget)
        submission = parse_submission(manifest.read_bytes())
        resolved = {}
        for index, (source, digest) in enumerate(bundle_sources(submission).items()):
            path = Path(directory, f"bundle-{index:02d}")
            with path.open("xb") as sink:
                download(source, BUNDLE, MAX_BUNDLE, digest, sink, transport, budget)
            resolved[source] = path
        report = compare_evidence.compare_submission(submission, resolved, submission_sha256=sha256)
        compare_evidence.write_report(output, report)
    return report


def main(argv=None, transport=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--submission-url", required=True)
    parser.add_argument("--sha256", required=True)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args(argv)
    try:
        fetch_and_compare(args.submission_url, args.sha256, args.output, transport)
    except (ValueError, TypeError, KeyError, IndexError, OSError, EOFError, OverflowError, RecursionError) as error:
        # JSON-quote untrusted text so it stays on one line and cannot start a workflow command.
        print(f"fetch_evidence: error: {json.dumps(str(error))}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
