import unittest
from run_botstorm import stable_storm


def rows(*,rate=lambda tick:100,living=12000):
    return [{'tick':tick,'census':{'meanLength':8,'world':{'living':living}},
             'deathsPerVirtualSecond':rate(tick),'admissionsPerVirtualSecond':rate(tick)}
            for tick in range(1000,46000,1000)]


class StormStability(unittest.TestCase):
    def test_steady_positive_churn_passes(self):
        self.assertIsNotNone(stable_storm(rows(),12000))

    def test_newborn_or_no_churn_does_not_pass(self):
        self.assertIsNone(stable_storm(rows()[:10],12000))
        self.assertIsNone(stable_storm(rows(rate=lambda tick:0),12000))

    def test_wave_and_depletion_do_not_pass(self):
        self.assertIsNone(stable_storm(rows(rate=lambda tick:tick),12000))
        self.assertIsNone(stable_storm(rows(living=11000),12000))


if __name__=='__main__':unittest.main()
