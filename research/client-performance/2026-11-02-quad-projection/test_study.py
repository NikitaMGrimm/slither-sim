"""Offline source reconstruction and retained-results checks; no timing or captures."""
import gzip
import hashlib
import json
from pathlib import Path
import re
import runpy
import unittest

FOLDER = Path(__file__).resolve().parent


def reverse_delta(reviewed, delta):
    target = reviewed.splitlines(keepends=True)
    lines = delta.splitlines(keepends=True)
    result = []
    cursor = 0
    i = 2  # one-file unified diff headers
    while i < len(lines):
        match = re.match(r'@@ -\d+(?:,\d+)? \+(\d+)(?:,\d+)? @@', lines[i])
        if not match:
            raise ValueError('invalid source delta hunk')
        start = int(match[1]) - 1
        result.extend(target[cursor:start])
        cursor = start
        i += 1
        while i < len(lines) and not lines[i].startswith('@@ '):
            line = lines[i]
            if line[0] in ' +':
                if target[cursor] != line[1:]:
                    raise ValueError('source delta does not match reviewed bytes')
                cursor += 1
            if line[0] in ' -':
                result.append(line[1:])
            i += 1
    return ''.join(result + target[cursor:])


class Study(unittest.TestCase):
    def test_reconstructs_exact_measured_source_and_unchanged_production(self):
        data = json.loads((FOLDER / 'identities.json').read_text(encoding='utf-8'))
        reviewed = gzip.decompress((FOLDER / data['source_comparison']['reviewed_source_snapshot']).read_bytes())
        delta = (FOLDER / 'frame-measured-to-reviewed.patch').read_bytes()
        measured = reverse_delta(reviewed.decode(), delta.decode()).encode()
        candidate = data['arms']['candidate']
        self.assertEqual(hashlib.sha256(reviewed).hexdigest(), candidate['final_frame_source_sha256'])
        self.assertEqual(hashlib.sha256(measured).hexdigest(), candidate['measured_frame_source_sha256'])
        self.assertEqual(hashlib.sha256(delta).hexdigest(), data['source_comparison']['delta_sha256'])
        def production(blob):
            return b'\n'.join(line for line in blob.split(b'#[cfg(test)]')[0].splitlines()
                              if not line.lstrip().startswith(b'//'))
        self.assertEqual(production(measured), production(reviewed))

    def test_retained_review_rows_and_narrow_claim(self):
        module = runpy.run_path(str(FOLDER / 'recompute.py'))
        result = module['review_summary']()
        self.assertIn('7 of 10 paired default-art medians improve; median paired reduction 6.575%.', result)
        self.assertIn('| paired-32768 | 21.660 | 13.320 | -38.505%', result)
        data = json.loads((FOLDER / 'identities.json').read_text(encoding='utf-8'))
        self.assertEqual(hashlib.sha256((FOLDER / 'review-converter.csv').read_bytes()).hexdigest(),
                         data['independent_converter_review']['runs_sha256'])


if __name__ == '__main__':
    unittest.main()
