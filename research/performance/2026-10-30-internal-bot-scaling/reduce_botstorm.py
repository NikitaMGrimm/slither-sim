"""Reduce the added center-seeking rows after all counting has ended."""
import json
from pathlib import Path
import statistics
import sys
from reduce import phase_summary, stats, verify_fixture, warm_food_bins
from run import digest


def summarize(root):
    root=Path(root)
    result={'build':json.loads((root/'botstorm-build.json').read_text()),
            'reducerSha256':digest(Path(__file__)),
            'method':{'policy':'own-head center steering; staggered every six ticks; no observation or boost',
                      'reportedReference':'a 12k-snake single-core reference world (64,800 across, center-steering bots); reported, not measured here',
                      'referenceHardwareAndTickRate':'unknown; older unspecified tick rate',
                      'rateUnit':'per virtual simulation second unless named wall'},'populations':[]}
    assert digest(root/'measurement-pins.json')==result['build']['masterPinsSha256']
    assert digest(root/'bin/internal_botstorm')==result['build']['binarySha256']
    for n in [12000,6000,9000]:
        directory=root/'botstorm-results'/str(n)
        receipt=directory/'qualification.json'
        if not receipt.exists():
            primary=root/'botstorm-results/12000/qualification.json'
            skipped=n!=12000 and primary.exists() and not json.loads(primary.read_text())['missedRealTime']
            result['populations'].append({'population':n,'status':'not measured',
                'reason':'12k meets the frozen throughput trigger' if skipped else 'required row incomplete; see attempts'})
            continue
        qualification=json.loads(receipt.read_text())
        verify_fixture(directory,qualification)
        census=json.loads((directory/'fixture/census.json').read_text())
        config=json.loads((directory/'fixture/world.json').read_text())['config']
        out={'population':n,'qualification':qualification,'census':census,'resolvedConfig':config,
             'warmFoodBins':warm_food_bins(directory,qualification),
             'densityRelativeToOfficial':n*670.0**2/config['radius']**2,
             'status':'stabilized' if qualification['stabilized'] else 'warmup censored'}
        rows=[json.loads((directory/f'window-{i}.json').read_text()) for i in range(5)]
        reference=qualification['finalStateSha256']
        assert all(r['finalStateSha256']==reference and r['ticks']==512 and not r['profiled'] for r in rows)
        samples=[s for r in rows for s in r['samples']]
        def timing(field):return stats([s[field]*1000 for s in samples])
        def combined(*fields):return stats([sum(s[f] for f in fields)*1000 for s in samples])
        def above(*fields):return sum(sum(s[f] for f in fields)>.007 for s in samples)/len(samples)
        counts={key:sum(s[key] for s in samples) for key in ['deaths','admitted','refused','poses','decisions']}
        profile=json.loads((directory/'profile.json').read_text())
        assert profile['finalStateSha256']==reference
        assert all(sum(s[key] for s in profile['samples'])*5==counts[key] for key in counts)
        out.update({'windows':5,'ticks':len(samples),'finalCensus':rows[0]['final'],'worldCpuMs':timing('worldCpu'),
                    'firstRestoredTickWorldCpuMs':stats([r['samples'][0]['worldCpu']*1000 for r in rows]),
                    'worldCpuMsExcludingFirstTick':stats([s['worldCpu']*1000 for r in rows for s in r['samples'][1:]]),
                    'worldWallMs':timing('worldWall'),'admissionCpuMs':timing('admissionCpu'),
                    'centerCpuMs':timing('centerCpu'),'centerWallMs':timing('centerWall'),
                    'worldPlusAdmissionCpuMs':combined('worldCpu','admissionCpu'),
                    'worldPlusAdmissionWallMs':combined('worldWall','admissionWall'),
                    'worldCpuFractionAbove7ms':above('worldCpu'),
                    'worldPlusAdmissionCpuFractionAbove7ms':above('worldCpu','admissionCpu'),
                    'worldPlusAdmissionWallFractionAbove7ms':above('worldWall','admissionWall'),
                    'wholeWindowTicksPerSecond':stats([r['ticks']/r['wallSeconds'] for r in rows]),
                    'counts':counts,'deathsPerVirtualSecond':counts['deaths']/(len(samples)*.007),
                    'admissionsPerVirtualSecond':counts['admitted']/(len(samples)*.007),
                    'deathsPerWallSecond':counts['deaths']/sum(r['wallSeconds'] for r in rows),
                    'admissionsPerWallSecond':counts['admitted']/sum(r['wallSeconds'] for r in rows),
                    'initialMemoryKiB':[r['initial']['memoryKiB'] for r in rows],
                    'finalMemoryKiB':[r['final']['memoryKiB'] for r in rows],
                    'phaseDiagnostic':phase_summary(profile,rows),
                    'perWindow':[{'wallSeconds':r['wallSeconds'],'processCpuSeconds':r['processCpuSeconds'],
                                  'finalStateSha256':r['finalStateSha256']} for r in rows]})
        result['populations'].append(out)
    (root/'botstorm-selected.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps([{'population':p['population'],'status':p['status'],
                      'worldPlusAdmissionCpuMs':p.get('worldPlusAdmissionCpuMs')} for p in result['populations']],indent=2))
    return result


if __name__=='__main__':summarize(sys.argv[1])
