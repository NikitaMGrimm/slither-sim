import unittest
import analyze as a


def example(lo=115, hi=119, reflected=142, previous=100, intervening=None):
    key=(1,1)
    def motion(line,t,wanted):
        return dict(line=line,packetIndex=0,serverMs=t,receiptMs=t+3,
                    before=dict(wanted=0),after=dict(wanted=wanted),
                    changedFields=['wanted'],directionChanged=False,directionOnly=False)
    rows=[motion(2,previous,0)]
    if intervening is not None:rows.append(motion(3,intervening,0))
    rows.append(motion(6,reflected,127))
    frames=[(t+3,t,line) for line,t in [(2,previous),(3,intervening if intervening is not None else previous+7),(5,reflected-7),(6,reflected)]]
    raw=dict(motions={key:rows},motionPositions={key:{(m['line'],0):i for i,m in enumerate(rows)}},
             motionTimes={key:[m['serverMs'] for m in rows]},
             commands={4:dict(t=1000,byte=125,priorWanted=0,active=True)},
             flags={2:[],3:[],5:[],6:[]},frames={key:frames},frameLines={key:[2,3,5,6]})
    return a.evaluate('synthetic',1,'synthetic','wanted',4,1000,
                      dict(sendServerLower=lo,sendServerUpper=hi),
                      dict(serverMs=reflected,jsonlLine=6,packetIndex=0),key,raw)


class Tests(unittest.TestCase):
    def test_gate_armed(self):
        r=example();self.assertEqual(r['state'],'armed');self.assertEqual(r['bucket'],'4')
        self.assertFalse(r['contradictionUpperBound'])

    def test_early_strict(self):
        r=example(reflected=130);self.assertTrue(r['contradictionUpperBound'])
        self.assertEqual(r['marginMs'],8)

    def test_width_protects_one_ms_short_gap(self):
        r=example(reflected=141);self.assertFalse(r['contradictionUpperBound'])
        self.assertTrue(r['contradictionAbsolutePaired'])

    def test_boundary_is_not_armed(self):
        self.assertEqual(example(lo=140,hi=144,reflected=150)['state'],'reopen-boundary')

    def test_open(self):
        self.assertEqual(example(lo=145,hi=149,reflected=155)['state'],'open')

    def test_intervening_report_ambiguous(self):
        self.assertEqual(example(intervening=117)['state'],'report-in-clock-interval')

    def test_no_prior(self):
        self.assertEqual(example(lo=95,hi=99)['state'],'no-prior')

    def test_quantiles_nearest_rank(self):
        self.assertEqual(a.quantile([4,1,2,3],.5),2)
        self.assertEqual(a.quantile([4,1,2,3],.95),4)
        self.assertIsNone(a.quantile([],.5))

    def test_tick_boundary_kept(self):
        self.assertEqual(example(lo=119,hi=123)['ticksRemainingRange'],[3,4])

    def test_actual_arrival_not_send(self):
        r=example(reflected=150)
        self.assertEqual(r['arrivalCausalLower'],115)
        self.assertEqual(r['arrivalCausalUpper'],150)
        self.assertEqual(r['arrivalGateState'],'mixed')

    def test_robust_early_arrival_armed(self):
        self.assertEqual(example(reflected=130)['arrivalGateState'],'definitely-armed')


if __name__=='__main__':unittest.main()
