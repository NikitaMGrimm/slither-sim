"""Reuse prior CPU/quantile helpers without copying their implementations."""
from pathlib import Path
import importlib.util,ast,statistics
r=Path(__file__).resolve().parent
def module(name,path):
 spec=importlib.util.spec_from_file_location(name,path);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);return m
prior=module('prior248',r/'shared248/select_results.py')
processes=module('process248',r/'shared248/run.py')
tree=ast.parse((r/'shared256/select.py').read_text());scope={'statistics':statistics,'p':prior}
exec(compile(ast.Module(body=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name in ['q','slope']],type_ignores=[]),'linked-256-summary','exec'),scope)
q=scope['q'];slope=scope['slope']
