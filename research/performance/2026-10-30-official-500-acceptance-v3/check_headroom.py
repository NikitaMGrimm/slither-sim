"""Exercise the unchanged ladder gate with summed helper CPU and strict bounds."""
from pathlib import Path
import argparse,ast,json,tempfile
from window import reduce_window
r=Path(__file__).resolve().parent
p=argparse.ArgumentParser();p.add_argument('--mutate-decision-only',action='store_true');args=p.parse_args()
if args.mutate_decision_only:
    source=(r/'window.py').read_text()
    assert source.count("v['cpuFraction']")==1
    source=source.replace("v['cpuFraction']","v.get('decisionCpuFraction',v['cpuFraction'])")
    scope={'__file__':str(r/'window.py'),'__name__':'mutated_headroom'}
    exec(compile(source,'mutated-decision-only-gate','exec'),scope)
    reduce_window=scope['reduce_window']
tree=ast.parse((r/'check_harness.py').read_text())
main=next(v for v in tree.body if isinstance(v,ast.FunctionDef) and v.name=='main')
fixture=next(v for v in ast.walk(main) if isinstance(v,ast.FunctionDef) and v.name=='fixture')
with tempfile.TemporaryDirectory() as directory:
    root=Path(directory)
    scope={'root':root,'json':json,'reduce_window':reduce_window}
    exec(compile(ast.Module(body=[fixture],type_ignores=[]),'linked-ladder-synthetic-fixture','exec'),scope)
    assert scope['fixture']()['clientValid']
    original=(root/'client.log').read_text().splitlines()
    def run(total, spike=None):
        rows=[]
        for line in original:
            value=json.loads(line[7:]);value.update(decisionCpuFraction=.1,connectorCpuFraction=(total-.1)/2,publisherCpuFraction=(total-.1)/2,cpuFraction=total)
            if spike is not None and value['unix']==112:
                value['cpuFraction']=spike;value['connectorCpuFraction']=(spike-.1)/2;value['publisherCpuFraction']=(spike-.1)/2
            assert abs(value['cpuFraction']-sum(value[k] for k in ['decisionCpuFraction','connectorCpuFraction','publisherCpuFraction']))<1e-12
            rows.append('CLIENT '+json.dumps(value))
        (root/'client.log').write_text('\n'.join(rows)+'\n')
        return reduce_window(root,dict(label='synthetic',bots=350,start={'unix':100},end={'unix':130}))['clientValid']
    assert run(.69)
    assert not run(.70), 'Median 70% must reject, despite 10% decision CPU'
    assert not run(.85), 'Background CPU must count toward worker headroom'
    assert run(.20,.79)
    assert not run(.20,.80), 'P99 80% must reject, despite 10% decision CPU'
print('PASS: worker total includes helpers; unchanged strict 70%/80% gate boundaries')
