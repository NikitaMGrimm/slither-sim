"""V2: only the prospective end-class amendment changes v1's verdict."""
from pathlib import Path
import importlib.util
r=Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('v1_acceptance',r/'sharedv1/acceptance.py')
v1=importlib.util.module_from_spec(spec);spec.loader.exec_module(v1)
mature=v1.mature
verdict=v1.verdict
quantiles=v1.quantiles
hist_quantiles=v1.hist_quantiles
def end_mature(v):
    return v['living']>=490 and v['meanSegments']>=45 and v['p90Segments']>=156
def reduce_window(root,window):
    result=v1.reduce_window(root,window)
    result.update(verdict(client_valid=result['clientValid'],zero_deferrals=result['deferredTicks']==0,start_mature=result['gates']['startInMatureClass'],end_mature=end_mature(result['endpointCensus']['fullWindowAfter'])))
    return result
