"""Continuation paths/seal only; all primary metrics and gates use frozen analyze.py."""
import argparse
import ctypes
import datetime
import json
from pathlib import Path
import sys
import analyze as a
from witnesses import collect

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]


def read_plan():
    plan=json.loads((HERE/'second-continuation-plan.json').read_text(encoding='utf-8'))
    original=json.loads((HERE/'plan.json').read_text(encoding='utf-8'))
    for key in ('questions','network','scheduler','telemetry'):
        if plan[key]!=original[key]: raise ValueError('changed frozen '+key)
    for pin in plan['sourcePins']:
        if a.digest(REPO/pin['path'])!=pin['sha256']: raise ValueError('changed source '+pin['path'])
    return plan


def check_seal(raw, pins):
    seal=json.loads((raw/'analysis-seal.json').read_text(encoding='utf-8'))
    if seal['developmentSha256']!=a.digest(raw/'development.json') or seal['sourcePins']!=pins:
        raise ValueError('development or analysis source changed after seal')


def main():
    p=argparse.ArgumentParser(); p.add_argument('--raw',type=Path,required=True)
    p.add_argument('--validation',action='store_true'); args=p.parse_args()
    if sys.platform=='win32':
        kernel=ctypes.windll.kernel32; kernel.GetCurrentProcess.restype=ctypes.c_void_p
        kernel.SetProcessAffinityMask.argtypes=(ctypes.c_void_p,ctypes.c_size_t)
        if not kernel.SetProcessAffinityMask(kernel.GetCurrentProcess(),48): raise OSError('analysis affinity failed')
    plan=read_plan()
    if args.validation: check_seal(args.raw,plan['sourcePins'])
    rows=[]
    for batch in plan['batches']:
        if (batch['split']=='heldout')!=args.validation: continue
        path=args.raw/batch['round']/batch['id']/'network0.jsonl.gz'
        if not path.exists(): rows.append(dict(batch=batch,missing=True)); continue
        row=a.analyze(path,batch,plan)
        if not row['quality']['issues']:
            row['schedulerWitnesses']=collect(path)
            for family,threshold in [('motion','lt35'),('mass','lt100'),('prey','lt300')]:
                for label in ('full','clean'):
                    expected=row['scheduler']['gaps'].get(family,{}).get(label,{}).get(threshold,0)
                    actual=row['schedulerWitnesses']['counts'].get(family+'-'+label,{}).get('serverBelowThreshold',0)
                    if expected!=actual: raise ValueError('witness count disagrees with frozen analysis')
        rows.append(row)
    out=args.raw/('validation.json' if args.validation else 'development.json')
    with out.open('x',encoding='utf-8',newline='\n') as f:
        json.dump(dict(analysisSha256=a.digest(__file__),primaryAnalysisSha256=a.digest(HERE/'analyze.py'),
                       numpy=a.np.__version__,scipy=a.scipy.__version__,results=rows),f,indent=2,allow_nan=False)
        f.write('\n')
    if not args.validation:
        with (args.raw/'analysis-seal.json').open('x',encoding='utf-8',newline='\n') as f:
            json.dump(dict(sealedUtc=datetime.datetime.now(datetime.timezone.utc).isoformat(),
                           developmentSha256=a.digest(out),sourcePins=plan['sourcePins']),f,indent=2)
            f.write('\n')
    print(out,flush=True)


if __name__=='__main__': main()
