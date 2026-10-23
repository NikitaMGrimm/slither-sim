"""Offline navigation checks for the checkout and the crate's declared include set.

Checks inline Markdown links and heading/explicit anchors, excluding fenced
examples. Does not fetch external URLs or interpret reference-style/HTML links.
"""

import fnmatch
from pathlib import Path
import posixpath
import re
import tomllib
import unittest
from urllib.parse import unquote, urlsplit


ROOT = Path(__file__).resolve().parents[1]


def prose(text):
    return re.sub(r"^(```|~~~).*?^\1[^\n]*$", "", text, flags=re.MULTILINE | re.DOTALL)


def links(text):
    for target in re.findall(r'\[[^\]\n]*\]\(([^\s)]+)(?:\s+"[^"]*")?\)', prose(text)):
        url = urlsplit(target.strip("<>"))
        if not url.scheme and not url.netloc:
            yield unquote(url.path), unquote(url.fragment)


def anchors(text):
    text = prose(text)
    found = set(re.findall(r'<a\s+(?:id|name)="([^"]+)"', text))
    counts = {}
    for heading in re.findall(r"^#+ (.+)$", text, re.MULTILINE):
        slug = re.sub(r"[^\w -]", "", heading.lower()).replace(" ", "-")
        count = counts.get(slug, 0)
        counts[slug] = count + 1
        found.add(f"{slug}-{count}" if count else slug)
    return found


def packaged(path, includes):
    """Current include entries are literal directories/files or single-level globs."""
    return any(path == entry.lstrip("/")
               or path.startswith(entry.lstrip("/") + "/")
               or fnmatch.fnmatchcase(path, entry.lstrip("/")) for entry in includes)


def broken_links(files, available):
    failures = []
    for name in files:
        for target, fragment in links((ROOT / name).read_text(encoding="utf-8")):
            resolved = posixpath.normpath(posixpath.join(posixpath.dirname(name), target)) if target else name
            if resolved.startswith(("../", "/")):
                failures.append(f"{name} -> outside checkout: {target}")
                continue
            # Compare exact spelling even on case-insensitive Windows filesystems.
            if resolved not in available:
                failures.append(f"{name} -> {target} (missing or unpackaged)")
            elif fragment and resolved.endswith(".md"):
                if fragment not in anchors((ROOT / resolved).read_text(encoding="utf-8")):
                    failures.append(f"{name} -> {target}#{fragment} (missing anchor)")
    return failures


class DocumentationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.files = {p.relative_to(ROOT).as_posix() for directory in
                     ("docs", "research", "src", "tests", "examples", "assets", "schemas", "tools", "evidence", ".github")
                     for p in (ROOT / directory).rglob("*") if p.is_file()}
        cls.files.update(p.name for p in ROOT.iterdir() if p.is_file())
        cls.markdown = sorted(p for p in cls.files if p.endswith(".md"))

    def test_checkout_links_and_anchors(self):
        self.assertEqual(broken_links(self.markdown, self.files), [])

    def test_main_repository_links_resolve_in_checkout(self):
        prefix = "https://github.com/NikitaMGrimm/slither-sim/blob/main/"
        for name in self.markdown:
            text = prose((ROOT / name).read_text(encoding="utf-8"))
            for target in re.findall(r'\]\((' + re.escape(prefix) + r'[^)]+)\)', text):
                url = urlsplit(target)
                path = unquote(url.path.split("/blob/main/", 1)[1])
                with self.subTest(source=name, target=path):
                    self.assertIn(path, self.files)
                    if url.fragment and path.endswith(".md"):
                        self.assertIn(unquote(url.fragment), anchors((ROOT / path).read_text(encoding="utf-8")))

    def test_packaged_guides_do_not_depend_on_excluded_research(self):
        includes = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))["package"]["include"]
        available = {p for p in self.files if packaged(p, includes)}
        guides = [p for p in self.markdown if p in available]
        self.assertTrue(guides)
        self.assertFalse(any(p.startswith("research/") for p in available))
        self.assertEqual(broken_links(guides, available), [])

    def test_study_reports_are_reachable_from_index(self):
        index = "research/README.md"
        reached = set()
        pending = [index]
        while pending:
            name = pending.pop()
            if name in reached:
                continue
            reached.add(name)
            for target, _ in links((ROOT / name).read_text(encoding="utf-8")):
                path = ((ROOT / name).parent / target).resolve()
                if path.is_relative_to(ROOT / "research") and path.suffix == ".md" and path.is_file():
                    pending.append(path.relative_to(ROOT).as_posix())
        reports = {p for p in self.files if p.startswith("research/") and p.endswith("/REPORT.md")}
        self.assertTrue(reports)
        self.assertEqual(reports - reached, set())

    def test_parser_ignores_external_and_fenced_examples(self):
        self.assertEqual(list(links('[A](notes%20here.md#section)\n[B](#here)\n'
                                    '[Web](https://example.com/absent)\n'
                                    '```md\n[Example](absent.md)\n```\n'
                                    '~~~md\n[Example](other.md)\n~~~\n')),
                         [("notes here.md", "section"), ("", "here")])
        self.assertEqual(anchors('# Same\n# Same\n<a id="old-link"></a>\n'),
                         {"same", "same-1", "old-link"})


if __name__ == "__main__":
    unittest.main()
