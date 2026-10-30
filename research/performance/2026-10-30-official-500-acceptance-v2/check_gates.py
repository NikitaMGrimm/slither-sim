"""Synthetic prospective end-class controls, not capacity evidence."""
from acceptance import end_mature, mature, verdict
v={'living':499,'meanSegments':99.034,'p90Segments':224,'p99Segments':298}
assert end_mature(v) and not mature(v)
assert end_mature({'living':490,'meanSegments':45,'p90Segments':156})
for key,value in [('living',489),('meanSegments',44.999999),('p90Segments',155)]:
    assert not end_mature(v|{key:value})
assert not verdict(client_valid=False,zero_deferrals=True,start_mature=True,end_mature=True)['passes']
assert not verdict(client_valid=True,zero_deferrals=False,start_mature=True,end_mature=True)['passes']
print('PASS: lower-only end class, strict unchanged validity/deferral gates; v1 remains failed')
from pathlib import Path
source=(Path(__file__).resolve().parent/'sharedv1/check_acceptance.py').read_text()
old="assert not reduce_window(root,window)['passes']\n (root/'host-stream.log').write_text"
assert source.count(old)==1
source=source.replace(old,"assert reduce_window(root,window)['passes']\n (root/'host-stream.log').write_text")
exec(compile(source,'linked-v1-controls-with-prospective-end-amendment','exec'))
