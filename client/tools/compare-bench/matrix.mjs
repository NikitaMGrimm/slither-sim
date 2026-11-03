import {schedule,transientFailure,comparison} from './analysis.mjs';

export function roundPlan(rounds,clients,{telemetryControls=true,limitedNtlView=false}={}) {
  return schedule(rounds,clients).flatMap(({round,client},index)=>['battledome','normal'].flatMap(scenario=>{
    const base={round,client,scenario,viewCondition:scenario==='normal'?'stock':client==='ntl'&&limitedNtlView?'ntl-farthest':'whole-map'};
    const on={...base,telemetry:'on'},off={...base,telemetry:'off'};
    const rows=round===1&&telemetryControls&&client!=='vlither'?(index%2?[off,on]:[on,off]):[on];
    if(scenario==='battledome'&&limitedNtlView&&['original','native'].includes(client)){
      const matched={...on,viewCondition:'ntl-farthest'};
      rows.push(...(round===1&&telemetryControls?(index%2?[{...matched,telemetry:'off'},matched]:[matched,{...matched,telemetry:'off'}]):[matched]));
    }
    return rows;
  }));
}

export function anchoredBoundary(view,census) {
  const b=view.boundary,p=census?.players?.[0],radius=census?.radius;
  if(!b||!p?.alive||!p.setup||!Number.isFinite(radius)||radius<=0)return null;
  const scale=b.radius/radius,[width,height]=view.viewport;
  const predicted=[width/2-p.x*scale,height/2-p.y*scale];
  // Camera smoothing and receipt lag are allowed; a pit circle at a different
  // radius cannot also explain the projected outer-map origin at the orbit.
  const error=Math.hypot(b.x-predicted[0],b.y-predicted[1]);
  if(error>Math.max(30,b.radius*.04))return null;
  return {physicalPixelsPerWorldUnit:scale,centerErrorPixels:error,
    wholeMap:b.x-b.radius>=0&&b.y-b.radius>=0&&b.x+b.radius<=width&&b.y+b.radius<=height,
    confidence:'outer-circle candidate anchored to host radius and projected player offset; camera smoothing tolerance 4% or 30 pixels'};
}

export function zoomPlateau(samples) {
  const scales=samples.map(s=>s.physicalPixelsPerWorldUnit);
  return scales.length>=3&&scales.every(v=>Number.isFinite(v)&&v>0)
    &&Math.max(...scales)/Math.min(...scales)<=1.03;
}
export function requireNtlDisplayGate(calibration) {
  if(calibration?.displayGate?.passed===true)return;
  const reasons=Array.isArray(calibration?.displayGate?.reasons)&&calibration.displayGate.reasons.length
    ?calibration.displayGate.reasons:['missing or non-passing displayGate evidence'];
  throw Error(`NTL display preflight rejected; no run plan or timed rounds: ${reasons.join('; ')}`);
}

export function checkNtlCalibratedView(picture,census,calibration,viewCondition,settings) {
  const fail=reason=>{throw Error(`NTL calibrated view preflight: ${reason}`);};
  if(!calibration||!Number.isFinite(calibration.physicalPixelsPerWorldUnit)||calibration.physicalPixelsPerWorldUnit<=0)fail('missing reference scale');
  if(!calibration.settings||JSON.stringify(settings)!==JSON.stringify(calibration.settings))fail('zoom UI settings differ from calibration');
  if(viewCondition!==(calibration.wholeMap?'whole-map':'ntl-farthest'))fail('row label differs from calibrated extent');
  const anchor=anchoredBoundary(picture,census);
  if(!anchor)fail('outer-map picture is not anchored');
  if(Math.abs(anchor.physicalPixelsPerWorldUnit/calibration.physicalPixelsPerWorldUnit-1)>.05)fail('physical scale differs from calibration by more than 5%');
  if(anchor.wholeMap!==calibration.wholeMap)fail('whole-map state differs from calibrated row label');
  return anchor;
}

// The same untimed setup is used by measured rows and the preflight-only path.
// Its wheel/settling work finishes before warmup, never inside the fixed window.
export async function settleNtlView(browser,calibration,viewCondition,capture,pause) {
  if(!Number.isInteger(calibration?.totalWheelNotches)||calibration.totalWheelNotches<1)throw Error('NTL calibrated view preflight: missing wheel reference');
  if(!calibration.settings||JSON.stringify(browser.ntlSettings)!==JSON.stringify(calibration.settings))throw Error('NTL calibrated view preflight: zoom UI settings differ from calibration');
  if(viewCondition!==(calibration.wholeMap?'whole-map':'ntl-farthest'))throw Error('NTL calibrated view preflight: row label differs from calibrated extent');
  await browser.zoomOut(calibration.totalWheelNotches);
  const samples=[];
  for(let i=0;i<2;i++){
    const sample=await capture(i);sample.anchor=checkNtlCalibratedView(sample.picture,sample.census,calibration,viewCondition,browser.ntlSettings);samples.push(sample);
    if(i===0)await pause(500);
  }
  if(Math.max(...samples.map(s=>s.anchor.physicalPixelsPerWorldUnit))/Math.min(...samples.map(s=>s.anchor.physicalPixelsPerWorldUnit))>1.03)throw Error('NTL calibrated view preflight: view has not settled');
  return {wheelNotches:calibration.totalWheelNotches,viewCondition,settings:browser.ntlSettings,samples};
}

export async function settleNativeView({target,width,readView,wheel,stopTelemetry}) {
  if(!Number.isFinite(target)||target<=0)throw Error('Matched NTL view calibration unavailable');
  for(let i=0;i<8;i++){
    const view=await readView(),scale=view.zoom*width/view.viewport?.[0];
    if(!Number.isFinite(scale)||scale<=0)throw Error('Native physical zoom unavailable');
    if(Math.abs(scale/target-1)<=.03){
      const setup={method:stopTelemetry?'measured setup-only view telemetry; stopped before warmup':'measured continuous view telemetry',
        physicalPixelsPerWorldUnit:scale,target,view};
      if(stopTelemetry)setup.stoppedAtMs=await stopTelemetry();
      return setup;
    }
    await wheel(Math.log(target/scale)/Math.log(1.12),i);
  }
  throw Error('Native wheel input did not reach the calibrated NTL view');
}

// Acquisition retention and comparison eligibility are separate. Select a whole
// attempt, never individual successful rows from an abandoned attempt.
export function sessionReport(collection,plan) {
  const {results,roundAttempts,...rest}=collection,selectedAttempts=[];
  const identity=row=>JSON.stringify(['round','client','scenario','viewCondition','telemetry'].map(k=>row[k]));
  const attempts=roundAttempts.map(record=>{
    const expected=plan.filter(row=>row.round===record.round).map(identity).sort();
    const rows=record.results.map(directory=>results.find(row=>row.directory===directory));
    const actual=rows.filter(row=>row?.run.attempt===record.attempt&&row.run.round===record.round).map(row=>identity(row.run)).sort();
    const complete=expected.length>0&&actual.length===expected.length&&new Set(record.results).size===expected.length&&actual.every((key,i)=>key===expected[i]);
    const selected=complete&&!record.blocked&&!selectedAttempts.some(row=>row.round===record.round);
    if(selected)selectedAttempts.push({round:record.round,attempt:record.attempt});
    return {...record,comparisonSelection:{included:selected,reason:selected?'complete round attempt':record.blocked??(!complete?'incomplete planned row set':'another complete attempt selected')}};
  });
  const included=new Set(attempts.filter(row=>row.comparisonSelection.included).flatMap(row=>row.results));
  const selected=results.filter(row=>included.has(row.directory)),comparisons=[];
  for(let i=0;i<selected.length;i++)for(let j=i+1;j<selected.length;j++){
    const a=selected[i],b=selected[j];
    if(a.run.round===b.run.round&&a.run.scenario===b.run.scenario)comparisons.push({a:a.directory,b:b.directory,...comparison(a.run,b.run)});
  }
  const selection={selectedAttempts,selectedDirectories:selected.map(row=>row.directory),selectedRowCount:selected.length};
  return {summary:{...rest,roundAttempts:attempts,selection},index:{selection,results:results.map(({run,...row})=>{
    const attempt=attempts.find(a=>a.round===run.round&&a.attempt===run.attempt);
    const comparisonSelection=included.has(row.directory)?attempt.comparisonSelection
      :{included:false,reason:attempt?.comparisonSelection.included?'acquisition outside selected planned row set':attempt?.comparisonSelection.reason??'no recorded round attempt'};
    return {...row,round:run.round,attempt:run.attempt,client:run.client,scenario:run.scenario,viewCondition:run.viewCondition,telemetry:run.telemetry,comparisonSelection};
  }),comparisons}};
}

export async function collectRounds({rounds,maxAttempts,plan,isInterrupted,preflight,acquire,persist,betweenAttempts}) {
  const results=[],roundAttempts=[];let pending=Array.from({length:rounds},(_,i)=>i+1);
  for(let attempt=1;attempt<=maxAttempts&&pending.length&&!isInterrupted();attempt++){
    const retry=[];
    for(let index=0;index<pending.length;index++){
      const round=pending[index];
      if(isInterrupted()){retry.push(...pending.slice(index));break;}
      const record={round,attempt,results:[]};roundAttempts.push(record);
      try{
        record.preflight=await preflight(round,attempt);
        if(!isInterrupted()&&(record.preflight.competingWork.length||record.preflight.idleMs<5000)){
          record.blocked='competing work or recent owner input';retry.push(round);await persist(roundAttempts);continue;
        }
        for(const item of plan.filter(item=>item.round===round)){
          if(isInterrupted())break;
          const result=await acquire({...item,attempt});results.push(result);record.results.push(result.directory);
          if(transientFailure(result.run)){record.blocked='transient validity failure; retained partial round';retry.push(round);break;}
        }
      }catch(error){if(!isInterrupted())throw error;record.interruptError=error.message;}
      if(isInterrupted()){
        record.blocked='interrupted; current and remaining rounds uncollected';
        if(retry.at(-1)===round)retry.pop();retry.push(...pending.slice(index));
        await persist(roundAttempts);break;
      }
      await persist(roundAttempts);
    }
    pending=retry;
    if(isInterrupted())break;
    if(pending.length&&attempt<maxAttempts)await betweenAttempts();
  }
  await persist(roundAttempts);
  return {results,roundAttempts,uncollectedRounds:pending,maxAttempts};
}
