"""Recompute the report's descriptive tables from selected public JSON."""
from pathlib import Path
import sys,json
root=Path(sys.argv[1]) if len(sys.argv)>1 else Path(__file__).resolve().parent
s=json.loads((root/'results.json').read_text());t=json.loads((root/'tail-results.json').read_text())
def f(x):return f'{x:.3f}'
def table(title,headers,rows):
 return '\n\n'+title+'\n\n| '+' | '.join(headers)+' |\n| '+' | '.join(['---']*len(headers))+' |\n'+'\n'.join('| '+' | '.join(map(str,row))+' |' for row in rows)+'\n'
def cases():
 for name,c in s['cases'].items():
  yield ('A' if c['arm']=='base' else 'B')+name.split('-pair-')[1].split('-')[0],c['holds'][0],t['cases'][name],c
out='All CPU costs below are milliseconds per completed tick. A = 551cb103; B = b750004a. Rows preserve AB/BA/AB order. Each primary row has 4,200 ticks.'
out+=table('Primary aligned world CPU',['Cohort','p50','p99','max','mean'],[[i,*[f(h['worldCpuMsPerTick'][k]) for k in ['p50','p99','max','mean']]] for i,h,_,_ in cases()])
out+=table('Primary simulation CPU (includes broadcast)',['Cohort','p50','p99','max','mean','Broadcast mean'],[[i,*[f(h['simulationCpuMsPerTick'][k]) for k in ['p50','p99','max','mean']],f(h['phaseAverageCpuMsPerTick']['broadcast'])] for i,h,_,_ in cases()])
out+=table('Primary socket CPU',['Cohort','p50','p99','max','mean'],[[i,*[f(h['socketCpuMsPerTick'][k]) for k in ['p50','p99','max','mean']]] for i,h,_,_ in cases()])
out+=table('Maintenance versus other ticks',['Cohort','Kind','Ticks','p50','p99','max','mean'],[[i,label,d['primary'][key]['ticks'],*[f(d['primary'][key]['worldCpuMsPerTick'][k]) for k in ['p50','p99','max','mean']]] for i,_,d,_ in cases() for key,label in [('maintenance','Maintenance'),('ordinary','Other')]])
out+=table('Primary slowest 1% (42 ticks per cohort)',['Cohort','World mean','Simulation mean','Socket mean','Maintenance %','Post-report %'],[[i,f(d['primary']['slowestOnePercent']['worldCpuMsPerTick']['mean']),f(d['primary']['slowestOnePercent']['simulationCpuMsPerTick']['mean']),f(d['primary']['slowestOnePercent']['socketCpuMsPerTick']['mean']),f(d['primary']['slowestOnePercent']['maintenanceFraction']*100),f(d['primary']['slowestOnePercent']['postReportFraction']*100)] for i,_,d,_ in cases()])
out+=table('Separate coarse diagnostic slowest 1% (27 ticks per cohort)',['Cohort','Socket','Broadcast','Bracketed physics','Admission','Unbracketed simulation'],[[i,f(d['coarse']['slowestOnePercent']['socketCpuMsPerTick']['mean']),f(d['coarse']['slowestOnePercent']['phaseMeanCpuMsPerTick']['broadcast']),f(d['coarse']['slowestOnePercent']['bracketedPhysicsMeanCpuMsPerTick']),f(d['coarse']['slowestOnePercent']['phaseMeanCpuMsPerTick']['admission']),f(d['coarse']['slowestOnePercent']['unbracketedSimulationMeanCpuMsPerTick'])] for i,_,d,_ in cases()])
out+=table('Client headroom and cadence',['Cohort','Client cores','Max worker CPU p99 %','Worst 1-s input late p99 ms','Max pass ms','Sampled max receipt age ms','Kernel queue max B'],[[i,f(h['client']['processCpuCoresMean']),f(max(v['p99'] for v in h['client']['workersCpuFraction'].values())*100),f(h['client']['worstOneSecondLateP99Ms']),f(h['client']['maxPassMs']),f(h['client']['maxReceiptAge']*1000),h['socket']['queues']['kernelSendBytes']['max']] for i,h,_,_ in cases()])
out+=table('Separate continued-world syscall brackets (all ticks, not tail-conditional)',['Cohort','Read','Write','Readiness poll'],[[i,*[f(c['syscallDiagnostic']['averageCpuMsPerTick'][k]) for k in ['read','write','readinessPoll']]] for i,_,_,c in cases()])
if (root/'fine-results.json').exists():
 fine=json.loads((root/'fine-results.json').read_text())['fine']
 labels=[('preparation','Preparation'),('snakeVisibilityEncoding','Snake visibility/encoding'),('recipientFood','Recipient food'),('foodMembershipRefreshOwnership','Membership refresh/ownership (within food)'),('subscriptionUnionSync','Subscription union (within preparation)'),('subscriptionAdvanceAndPostPhysics','Subscription advance/post-physics'),('frameEmissionGrouping','Frame emission/grouping')]
 out+=table('One separate current-main nested diagnostic (inclusive scopes)',['Scope','All ticks','Maintenance','Other','Slowest 1%'],[[label,*[f(fine[group]['nestedInclusiveMeanCpuMsPerTick'][key]) for group in ['all','maintenance','ordinary','slowestOnePercent']]] for key,label in labels])
print(out)
