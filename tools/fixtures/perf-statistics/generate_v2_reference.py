# /// script
# requires-python = ">=3.12"
# dependencies = ["numpy==2.5.4", "scipy==1.18.1"]
# ///
"""Offline v2 constants/reference oracle. Never runs the simulator or CI."""
from fractions import Fraction
import json
import math
from pathlib import Path

import numpy as np
import scipy
from scipy.linalg import lstsq
from scipy.stats import t

OUT=Path(__file__).resolve().parent
ORDER26=[1,-1,-1,1,1,-1,1,-1,-1,1,1,-1,-1,1,1,-1,-1,1,-1,1,1,-1,1,-1,-1,1]
LOOKS=[8,12,26]+[26*2**k for k in range(1,12)]
DEN=[750]*3+[1000*2**k for k in range(1,12)]
LOWER,UPPER=-.05129329438755058,.04879016416943204


def oracle(n,mu,gamma,noise):
    pattern=[-1.1,.7,-.4,1.4,-.8,.2,1.,-1.,.5,-.3,1.2,-.6,-.9,.4,-.2,1.3,-.7,.1,.9,-1.2,.6,-.5,1.1,-.1,.3,-.8]
    pairs=[]
    for i in range(n):
        base=1+.03*((i*7)%11)
        d=mu+gamma*ORDER26[i%26]+noise*pattern[i%26]
        pairs.append({'pair_index':i+1,'order':ORDER26[i%26],
                      'baseline_wall_seconds':base,'candidate_wall_seconds':base*math.exp(d),
                      'operations_per_block':1000,'blocks':1})
    d=np.array([math.log(p['candidate_wall_seconds'])-math.log(p['baseline_wall_seconds']) for p in pairs])
    o=np.array([p['order'] for p in pairs]);assert sum(o)==0
    x=np.column_stack([np.ones(n),o]); beta=lstsq(x,d,lapack_driver='gelsd')[0]
    e=d-x@beta;sse=float(e@e);s=math.sqrt(sse/(n-2));se=s/math.sqrt(n)
    critical=float(t.isf(1/(2*DEN[LOOKS.index(n)]),n-2))
    floor=64*math.ulp(max(1.,max(abs(d))))
    result={'n':n,'df':n-2,'mu_hat':float(beta[0]),'gamma_hat':float(beta[1]),'sse':sse,'s':s,'se':se,
            'critical':critical,'change_percent':100*math.expm1(float(beta[0])),'residual_floor':floor}
    for label in ('baseline','candidate'):
        result[label+'_ms_per_operation']=math.exp(math.log(1000)+math.fsum(math.log(p[label+'_wall_seconds']) for p in pairs)/n-math.log(1000))
    if s<=floor:
        result.update(interval_log=None,interval_percent=None,label='Undecided',reason='degenerate_residuals')
    else:
        lo,hi=beta[0]-critical*se,beta[0]+critical*se
        label='Faster' if hi<LOWER else 'Slower' if lo>UPPER else 'Within' if lo>=LOWER and hi<=UPPER else 'Undecided'
        result.update(interval_log=[float(lo),float(hi)],interval_percent=[100*math.expm1(float(lo)),100*math.expm1(float(hi))],label=label,reason='valid_interval')
    return pairs,result


def main():
    table=[{'n':n,'df':n-2,'eta_numerator':1,'eta_denominator':den,'upper_tail_numerator':1,'upper_tail_denominator':2*den,
            'critical':float(t.isf(1/(2*den),n-2))} for n,den in zip(LOOKS,DEN)]
    total=10*sum((Fraction(1,den) for den in DEN),Fraction())
    assert total<=Fraction(1,20)
    data=[]
    for name,n,mu,gamma,noise in [
        ('faster_at_8',8,math.log(.90),.02,.006),('within_at_12',12,0,.02,.01),
        ('undecided_at_26',26,math.log(1.05),.015,.10),('within_at_52',52,.005,.04,.04),
        ('faster_at_52',52,math.log(.90),-.02,.04),('slower_at_52',52,math.log(1.12),.02,.04),
        ('degenerate_at_52',52,.12,.02,0),('within_at_104',104,.002,.03,.06),
        ('within_at_832',832,.001,.02,.10)]:
        pairs,result=oracle(n,mu,gamma,noise)
        data.append({'id':name,'pairs':pairs,'expected':result})
    payload={'schema':'slither-perf-proposed-v2-reference-v1',
             'status':'Mathematical implementation proposal only; does not validate hosted model or authorize CI resources.',
             'generator':{'numpy':np.__version__,'scipy':scipy.__version__,'ols':'scipy.linalg.lstsq(gelsd)','critical':'scipy.stats.t.isf(1/(2*eta_denominator), n-2)'},
             'constants':{'workloads':10,'looks':LOOKS,'order26_repeated':ORDER26,'lower':LOWER,'upper':UPPER,
                          'family_alpha_exact_numerator':total.numerator,'family_alpha_exact_denominator':total.denominator,
                          'family_alpha_float':float(total),'table':table},'datasets':data,
             'numeric_tolerance':{'relative':2e-10,'absolute':5e-12},
             'checkpoint_examples':[{'complete_pairs':45,'use_n':26},{'complete_pairs':51,'use_n':26},
                                    {'complete_pairs':52,'use_n':52},{'complete_pairs':100,'use_n':52},
                                    {'complete_pairs':104,'use_n':104},{'complete_pairs':5000,'use_n':3328},
                                    {'complete_pairs':7,'use_n':None}]}
    (OUT/'protocol-v2-reference.json').write_text(json.dumps(payload,indent=2,allow_nan=False)+'\n',encoding='utf-8',newline='\n')
    lines=['n,df,eta_denominator,upper_tail_denominator,t_critical']
    lines += [f"{r['n']},{r['df']},{r['eta_denominator']},{r['upper_tail_denominator']},{r['critical']:.17g}" for r in table]
    (OUT/'protocol-v2-critical-values.csv').write_text('\n'.join(lines)+'\n',encoding='utf-8',newline='\n')
    print('\n'.join(lines));print('Family alpha <=',float(total))


if __name__=='__main__':main()
