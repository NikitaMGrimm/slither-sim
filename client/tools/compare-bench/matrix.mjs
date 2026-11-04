import {schedule,transientFailure,comparison,anchoredBoundary,vlitherPhysicalScale,checkVlitherPicture} from './analysis.mjs';

export function roundPlan(rounds,clients,{telemetryControls=true,limitedNtlView=false}={}) {
  const matched=limitedNtlView&&clients.includes('ntl');
  return schedule(rounds,clients).flatMap(({round,client},index)=>['battledome','normal'].flatMap(scenario=>{
    const base={round,client,scenario,viewCondition:scenario==='normal'?'stock':matched?'ntl-farthest':'whole-map'};
    const on={...base,telemetry:'on'},off={...base,telemetry:'off'};
    const rows=round===1&&telemetryControls&&client!=='vlither'?(index%2?[off,on]:[on,off]):[on];
    return rows;
  }));
}
export function preflightPlan(clients,limitedNtlView) {
  const viewCondition=limitedNtlView?'ntl-farthest':'whole-map';
  const rows=clients.map(client=>({round:0,client,scenario:'battledome',telemetry:'on',viewCondition}));
  if(limitedNtlView&&clients.includes('native'))rows.splice(2,0,{round:0,client:'native',scenario:'battledome',telemetry:'off',viewCondition});
  rows.push(...clients.map(client=>({round:0,client,scenario:'normal',telemetry:'on',viewCondition:'stock'})));
  return rows;
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
export function requireSetupAvailability(focus,current) {
  if(!Number.isFinite(focus?.lastInputTick)||!Number.isFinite(current?.idleMs)||current.idleMs<5000
    ||focus.lastInputTick!==current.lastInputTick||current.foreground!==true||current.visible!==true
    ||current.minimized!==false||current.occluded!==false)throw Error('Owner input or window obscured during setup/warmup; no timed capture');
}

export async function settleBrowserVisibility(focus,pid,read,pause) {
  for(let i=0;;i++){
    const current=await read(i);
    // Chrome fullscreen can briefly create a separate owned surface. Wait for
    // it to disappear; acceptance still requires the unmodified visibility gate.
    const ownedTransient=Number.isFinite(focus?.lastInputTick)&&Number.isFinite(current?.idleMs)&&current.idleMs>=5000
      &&focus.lastInputTick===current.lastInputTick&&current.foreground===true&&current.visible===true&&current.minimized===false
      &&current.occluded===true&&current.occludingWindows?.length>0
      &&current.occludingWindows.every(w=>w.pid===pid&&w.windowClass==='Chrome_WidgetWin_1');
    if(!ownedTransient||i===7){requireSetupAvailability(focus,current);return current;}
    await pause(500);
  }
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
  // Calibration's first 80-notch picture must already share the plateau. A new
  // package that does not reproduce it fails the unchanged picture checks.
  const wheelNotches=80;
  await browser.zoomOut(wheelNotches);
  const samples=[];
  for(let i=0;i<2;i++){
    const sample=await capture(i);sample.anchor=checkNtlCalibratedView(sample.picture,sample.census,calibration,viewCondition,browser.ntlSettings);samples.push(sample);
    if(i===0)await pause(500);
  }
  if(Math.max(...samples.map(s=>s.anchor.physicalPixelsPerWorldUnit))/Math.min(...samples.map(s=>s.anchor.physicalPixelsPerWorldUnit))>1.03)throw Error('NTL calibrated view preflight: view has not settled');
  return {wheelNotches,referenceWheelNotches:calibration.totalWheelNotches,viewCondition,settings:browser.ntlSettings,samples};
}

export function vlitherWheel(view,target,width,height) {
  const scale=vlitherPhysicalScale(view,width,height),step=view.wheelStep;
  if(!Number.isFinite(scale)||!Number.isFinite(step)||step<=0||!Number.isFinite(target)||target<=0)throw Error('Vlither zoom preflight: physical scale or stock wheel step unavailable');
  if(!Number.isFinite(view.stockZoomMinimum)||!Number.isFinite(view.stockZoomMaximum)||target<view.stockZoomMinimum||target>view.stockZoomMaximum)throw Error('Vlither zoom preflight: target outside unchanged stock zoom range');
  if(!Number.isFinite(view.wheelZoom)||Math.abs(view.wheelZoom/scale-1)>.005)throw Error('Vlither zoom preflight: stock zoom has not settled');
  const exact=Math.log(target/scale)/step;
  const error=n=>Math.abs(scale*Math.exp(n*step)/target-1);
  const whole=[Math.floor(exact),Math.ceil(exact)].sort((a,b)=>error(a)-error(b))[0];
  const wholeFits=error(whole)<=.05;
  const notches=wholeFits?whole:Math.round(exact*120)/120;
  if(!Number.isFinite(notches)||Math.abs(notches)>200||error(notches)>.05)throw Error('Vlither zoom preflight: stock wheel quantum cannot reach target tolerance');
  return {notches,method:wholeFits?'whole stock wheel notches':'fractional stock wheel offset (1/120 notch)',wheelStep:step};
}

export async function settleVlitherView({calibration,width,height,readView,wheel,capture,pause}) {
  const target=calibration?.physicalPixelsPerWorldUnit,adjustments=[];
  if(calibration?.wholeMap!==false)throw Error('Vlither zoom preflight: missing limited-view calibration');
  let matched=false;
  // One read beyond the last wheel, so the final adjustment's result is judged.
  for(let i=0;i<=8;i++){
    const view=await readView(),adjustment=vlitherWheel(view,target,width,height);
    if(Math.abs(vlitherPhysicalScale(view,width,height)/target-1)<=.05){matched=true;break;}
    if(i===8)break;
    adjustments.push(adjustment);await wheel(adjustment.notches,i);
  }
  if(!matched)throw Error('Vlither zoom preflight: stock wheel did not reach target');
  const pictureSamples=[];
  for(let i=0;i<2;i++){
    const sample=await capture(i);
    sample.anchor=checkVlitherPicture(sample,calibration,width,height);pictureSamples.push(sample);
    if(i===0)await pause(500);
  }
  if(Math.max(...pictureSamples.map(s=>s.anchor.physicalPixelsPerWorldUnit))/Math.min(...pictureSamples.map(s=>s.anchor.physicalPixelsPerWorldUnit))>1.03)throw Error('Vlither zoom preflight: anchored pictures have not settled');
  return {method:'stock GLFW wheel callback; framebuffer scale checked by host-anchored pictures',adjustments,pictureSamples};
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
