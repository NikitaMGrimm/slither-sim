"""Tiny standard-library regression for the two actual nearest-rank helpers."""
from pathlib import Path
import ast,collections,math,statistics
tree=ast.parse(Path(__file__).with_name('acceptance.py').read_text())
functions=[v for v in tree.body if isinstance(v,ast.FunctionDef) and v.name in ['quantiles','hist_quantiles']]
assert len(functions)==2
exec(compile(ast.Module(body=functions,type_ignores=[]),'acceptance-quantiles','exec'))
assert quantiles([1,2,3,4])['p50']==2
assert quantiles([1,2,3,4])['max']==4
assert quantiles([7])['p50']==7
assert hist_quantiles([{'x':[[1000,1],[2000,1],[3000,1],[4000,1]]}],'x')['p50']==2
assert hist_quantiles([{'x':[[1000,3],[4000,1]]}],'x')['p50']==1
assert hist_quantiles([{'x':[[1000,3],[4000,1]]}],'x')['max']==4
print('PASS: sample and weighted-histogram nearest-rank medians/maxima')
