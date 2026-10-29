"""Reuse the retained command inventory's credential redaction."""
from pathlib import Path
import importlib.util,sys
p=Path('/tmp/slither-maintenance-round2-20261029/cleanup/shared/reproduce.py')
spec=importlib.util.spec_from_file_location('shared',p)
s=importlib.util.module_from_spec(spec);spec.loader.exec_module(s)
s.inventory(Path(sys.argv[1]),sys.argv[2])
