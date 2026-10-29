"""Add the reviewed eager head between main/lazy in the existing frozen runner."""
from pathlib import Path

prior = Path(__file__).resolve().parents[2] / "2026-10-29-spawn-head-distance/run_pairs.py"
wrapper = prior.read_text()
needle = "exec(compile(text, str(prior),"
assert wrapper.count(needle) == 1
wrapper = wrapper.replace(needle, '''
text = text.replace("for arm in ['base','candidate']:", "for arm in ['base','eager','candidate']:")
text = text.replace("(['base','candidate'] if pair==-1 or pair%2==0 else ['candidate','base'])",
                    "(['base','eager','candidate'] if pair==-1 or pair%2==0 else ['candidate','eager','base'])")
text = text.replace("parents['base'].ready==parents['candidate'].ready",
                    "parents['base'].ready==parents['eager'].ready==parents['candidate'].ready")
old = "    assert same(proofs['base'][0].with_suffix('.trace'),proofs['candidate'][0].with_suffix('.trace'))"
assert text.count(old) == 1
text = text.replace(old, old + "\\n    assert same(proofs['eager'][0].with_suffix('.trace'),proofs['candidate'][0].with_suffix('.trace'))")
old = "        assert same(*(paths[a].with_suffix('.trace') for a in ['base','candidate']))"
assert text.count(old) == 1
text = text.replace(old, old + "\\n        assert same(paths['eager'].with_suffix('.trace'),paths['candidate'].with_suffix('.trace'))")
''' + needle)
exec(compile(wrapper, str(prior), "exec"), {"__file__": str(prior), "__name__": "__main__"})
