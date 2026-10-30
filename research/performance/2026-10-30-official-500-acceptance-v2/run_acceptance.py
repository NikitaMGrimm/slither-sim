"""Retain v1's exact warmup, window, report selection and archive sequence."""
from pathlib import Path
r=Path(__file__).resolve().parent
exec(compile((r/'sharedv1/run_acceptance.py').read_text(),str(r/'linked-v1-run.py'),'exec'),globals())
