"""The confirmation-only gate, evaluated from exact integer CPU totals."""
from fractions import Fraction


def evaluate(rows):
    assert len(rows) == 4
    assert {row['name'] for row in rows} == {
        'mature-stock', 'mature-legacy', 'short-500-stock', 'short-500-legacy'}
    mature, young = [], []
    for row in rows:
        assert [p['pair'] for p in row['pairs']] == list(range(-1, 10))
        assert all(p['discardedAB'] == (p['pair'] == -1) for p in row['pairs'])
        pairs = row['pairs'][1:]
        assert all(p[arm]['all']['ticks'] == 1080
                   for p in pairs for arm in ['base', 'candidate'])
        def ratios(kind):
            return sorted(Fraction(p['candidate'][kind]['cpuTotalNs'],
                                   p['base'][kind]['cpuTotalNs']) for p in pairs)
        def median(values):
            assert len(values) == 10
            return (values[4] + values[5]) / 2
        if row['name'].startswith('mature-'):
            whole, maintenance = ratios('all'), ratios('maintenance')
            wins = sum(p['candidate']['all']['cpuTotalNs'] < p['base']['all']['cpuTotalNs']
                       for p in pairs)
            value = {
                'row': row['name'], 'improvingWholeWindowPairs': wins,
                'requiredImprovingPairs': 8,
                'medianPairedWholeCpuRatio': float(median(whole)),
                'wholeCpuPairedRatioRange': [float(whole[0]), float(whole[-1])],
                'medianPairedMaintenanceCpuRatio': float(median(maintenance)),
                'maintenanceCpuPairedRatioRange': [float(maintenance[0]), float(maintenance[-1])],
                'qualifies': wins >= 8 and median(whole) < 1 and median(maintenance) < 1,
            }
            mature.append(value)
        else:
            costs = sorted(p['candidate']['all']['cpuTotalNs'] - p['base']['all']['cpuTotalNs']
                           for p in pairs)
            middle_sum = costs[4] + costs[5]
            young.append({
                'row': row['name'], 'ticksPerWindow': 1080,
                'medianPairedWindowCostNsTimesTwo': middle_sum,
                'medianPairedCostUsPerTick': float(Fraction(middle_sum, 2 * 1080 * 1000)),
                'pairedCostRangeUsPerTick': [costs[0] / 1080 / 1000, costs[-1] / 1080 / 1000],
                'qualifies': middle_sum <= 2 * 1080 * 10000,
            })
    return {'reportedPairsPerRow': 10, 'originalSeriesPooled': False,
            'mature': mature, 'youngAbsoluteCostLimitUs': 10, 'young': young,
            'runtimeQualifies': all(v['qualifies'] for v in mature + young)}
