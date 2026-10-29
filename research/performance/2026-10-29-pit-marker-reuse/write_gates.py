"""Reuse the common native gate writer against the shrinking-marker row.

Usage: python write_gates.py PRIOR_WRITER OUTPUT CORRECTNESS_JSON VERIFICATION_JSON
"""
from pathlib import Path
import sys
prior=Path(sys.argv.pop(1));text=prior.read_text()
assert text.count('fixed-parity.json')==2
text=text.replace('fixed-parity.json','row-parity.json')
exec(compile(text,str(prior),'exec'),{'__file__':str(prior),'__name__':'__main__'})
