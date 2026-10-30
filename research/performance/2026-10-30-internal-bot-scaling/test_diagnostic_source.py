from pathlib import Path
import unittest
from diagnostic_source import instrument


class DiagnosticSource(unittest.TestCase):
    def test_clock_sites_are_added_only_to_generated_copies(self):
        directory=Path(__file__).parent
        source=(directory/'profile.rs').read_text()
        transformed=instrument(source)
        self.assertNotIn('world_start_monotonic',source)
        self.assertEqual(transformed.count('s.world_start_monotonic = cpu(1)'),1)
        storm=source.split('fn main() {')[0]+(directory/'botstorm.rs').read_text()
        transformed=instrument(storm)
        for variable in ['s','base']:
            self.assertEqual(transformed.count(variable+'.admission_start_monotonic = cpu(1)'),1)
            self.assertEqual(transformed.count(variable+'.world_end_monotonic = cpu(1)'),1)


if __name__=='__main__':unittest.main()
