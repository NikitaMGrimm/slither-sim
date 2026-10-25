"""Frozen network-path functions reused without refitting (2026-10-25 study)."""
import numpy as np
from collections import deque
from scipy.stats import theilslopes
from scipy.signal import lfilter
Q = [.01, .1, .5, .9, .95, .99, .999]
LAGS = [1, 2, 5, 10, 20, 50, 100]

def summary(a):
    a=np.asarray(a); a=a[np.isfinite(a)]
    if not len(a): return dict(n=0)
    return dict(n=len(a),mean=float(a.mean()),sd=float(a.std()),minimum=float(a.min()),maximum=float(a.max()),
                **{f'p{q*100:g}':float(np.quantile(a,q)) for q in Q})

def rolling_min(t,x,half=10000):
    out=np.empty(len(t)); candidates=deque(); right=0
    for i,ti in enumerate(t):
        while right<len(t) and t[right]<=ti+half:
            while candidates and x[candidates[-1]]>=x[right]: candidates.pop()
            candidates.append(right); right+=1
        while candidates and t[candidates[0]]<ti-half: candidates.popleft()
        out[i]=x[candidates[0]]
    return out

def baseline(t,s):
    # A robust straight clock-rate correction estimated from 5 s bin 1% floors.
    # Not a claim that the floor is absolute one-way propagation delay.
    x=t-t[0]; off=t-s
    bins=(x//5000).astype(int)
    b=np.unique(bins)
    tx=np.array([np.median(x[bins==k]) for k in b])
    floors=np.array([np.quantile(off[bins==k],.01) for k in b])
    slope=float(theilslopes(floors,tx)[0]) if len(b)>=3 else 0.
    residual=off-slope*x
    lin=residual-np.quantile(residual,.001)
    local=residual-rolling_min(t,residual)
    return slope,lin,local

def acf_time(t,x,mask=None):
    # Sample last receipt on a 10 ms wall clock, suppress stale samples (>50 ms).
    grid=np.arange(t[0],t[-1],10.)
    ix=np.searchsorted(t,grid,side='right')-1
    ok=(ix>=0)&(grid-t[ix]<=50)
    if mask is not None: ok &= mask[ix]
    z=np.minimum(x,np.quantile(x[mask] if mask is not None else x,.99))[ix]
    z-=np.mean(z[ok]); v=np.mean(z[ok]**2)
    out={}
    for lag in [10,20,50,100,200,500,1000,3000]:
        k=lag//10; good=ok[:-k]&ok[k:]
        out[str(lag)]=float(np.mean(z[:-k][good]*z[k:][good])/v) if good.sum()>10 and v>0 else None
    return out

def simulate(s,params,seed=104):
    # Lognormal ordinary jitter, a Gaussian OU log-factor, and sparse exponential
    # outliers. A single delayed frame holds later frames behind it in TCP order.
    median,sigma,shared,tau,rate,spike_mean=params
    rng=np.random.default_rng(seed)
    grid=np.arange(0,s[-1]-s[0]+20,5.)
    rho=np.exp(-.005/tau)
    z=lfilter([np.sqrt(1-rho*rho)],[1,-rho],rng.standard_normal(len(grid)))
    latent=np.interp(s-s[0],grid,z)
    normal=np.exp(np.log(median)+sigma*(np.sqrt(shared)*latent+np.sqrt(1-shared)*rng.standard_normal(len(s))))
    # Wall-clock Poisson delivery pauses, independent of the number of game messages.
    # Overlapping pauses merge by their furthest ending time.
    n_events=max(32,int((s[-1]-s[0])/1000*3*4)+32)
    starts=s[0]+np.cumsum(rng.exponential(1000/rate,n_events))
    raw_ends=starts+rng.exponential(spike_mean,n_events)
    prefix_ends=np.maximum.accumulate(raw_ends)
    group_starts=np.r_[True,starts[1:]>prefix_ends[:-1]]
    ends=np.maximum.reduceat(raw_ends,np.flatnonzero(group_starts))
    starts=starts[group_starts]
    due=s+normal
    ix=np.searchsorted(starts,due,side='right')-1
    pause_end=np.where(ix>=0,ends[np.maximum(ix,0)],s[0])
    arrived=np.maximum.accumulate(np.maximum(due,pause_end))
    e=arrived-s
    # Apply the same finite 20 s floor definition used for the observed series.
    e-=rolling_min(arrived,e)
    return arrived,e

