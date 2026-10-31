import assert from 'node:assert/strict';
import { test } from 'node:test';
import { mountLogin } from '../apps/wgpu-slice/web/login-overlay.mjs';

// Authored DOM seam; exact policy is exercised by login-abi.mjs in both Wasm builds.
function fixture(extra = {}, adapter = {}, storage) {
  class Node extends EventTarget {
    style={};dataset={};children=[];textContent='';value='';disabled=false;
    appendChild(n){this.children.push(n);n.parentNode=this;return n;}setAttribute(k,v){this[k]=v;}
    focus(){this.focused=true;}remove(){this.removed=true;}
    getContext(){return {measureText:()=>({width:36})};}
  }
  const realm=new EventTarget();Object.assign(realm,{AbortController,navigator:{language:'en'},performance:{now:()=>0},innerWidth:960,innerHeight:540,location:{href:'http://127.0.0.1:19090/'},localStorage:{getItem(){return null;},setItem(){throw Error('Unavailable');}}});
  realm.document={body:new Node(),createElement:()=>{const n=new Node();n.ownerDocument=realm.document;return n;}};
  if(storage) realm.localStorage=storage;
  const actions=[],calls=[];let alive=true, composingValue='';
  class Policy {
    restore_settings(...args){calls.push(['restore_settings',...args]);}
    settings_record(){return '2:0:original';}
    world_resolution(){return 'original';}
    edit(value){composingValue=value;return value;}
    action(...args){actions.push(args);return args[0]==='quality'?'0':undefined;}
    play_label(){return 'Play';}
    observe_life(phase){calls.push(['terminal',phase]);return phase==='failed';}
    pointer(...args){calls.push(args);}
    step(){return Object.fromEntries(['logo','last','tips','holder','input','play','skin','quality_rect','spinner'].map(k=>[k,[0,0,100,30]]).concat(Object.entries({visible:true,disabled:false,play_disabled:extra.disabled??false,ordinary_controls_visible:true,last_label:'',last_number:'\u00a0',last_suffix:'',last_label_alpha:.45,alpha:1,logo_alpha:1,nickname_alpha:1,play_alpha:1,skin_alpha:1,tip_alpha:1,hover:0,press:0,scale:1,nickname:composingValue,placeholder:'Nickname',play_label:'Play',tip:'Eat',skin_label:'Change Skin',quality_label:'Graphics quality',quality:'high',spinner_visible:false,spinner_alpha:0,spinner_scale:0,spinner_time:0,...extra})));}
    cancel_pending_play(){calls.push('cancel_pending');}
    free(){calls.push('free');}
  }
  const ui=mountLogin({realm,LoginMenu:Policy,isCurrent:()=>alive,available:()=>true,playing:()=>false,play:()=>calls.push('play'),release:()=>calls.push('release'),...adapter});
  const emit=(node,type,fields={})=>{const e=new Event(type,{cancelable:true});Object.assign(e,fields);node.dispatchEvent(e);return e;};
  return {ui,realm,emit,actions,calls,stale(){alive=false;}};
}
test('IME Enter belongs to composition through key release; pointer Play uses committed value',()=>{
  const f=fixture();f.emit(f.ui.input,'compositionstart');f.ui.input.value='raw';
  f.emit(f.ui.input,'keydown',{key:'Enter',code:'Enter',isComposing:true});
  f.emit(f.ui.input,'compositionend');f.emit(f.ui.input,'keydown',{key:'Enter',code:'Enter'});
  assert.equal(f.actions.length,0);
  f.emit(f.realm,'keyup',{code:'Enter'});f.emit(f.ui.input,'keydown',{key:'Enter',code:'Enter'});
  assert.deepEqual(f.actions,[['enter',false,true]]);
  f.ui.dispose();
});
test('composition click defers until commit; stale events and callbacks cannot publish',()=>{
  const f=fixture();f.emit(f.ui.input,'compositionstart');f.emit(f.ui.button,'click');assert.equal(f.actions.length,0);
  f.ui.input.value='committed';f.emit(f.ui.input,'compositionend');assert.deepEqual(f.actions,[['play',false,true]]);
  f.stale();f.emit(f.ui.quality,'click');assert.equal(f.ui.frame(100),undefined);assert.equal(f.actions.length,1);
  f.ui.dispose();f.ui.dispose();assert.equal(f.calls.filter(x=>x==='free').length,1);
});
test('quality survives unavailable storage and focus releases held input',()=>{
  const f=fixture();f.emit(f.ui.quality,'click');f.emit(f.ui.root,'focusin');f.emit(f.ui.root,'focusout');
  assert.deepEqual(f.actions,[['quality',false,true]]);assert.equal(f.calls.filter(x=>x==='release').length,2);
  f.ui.frame(16);assert.equal(f.ui.quality['aria-pressed'],'true');f.ui.dispose();
});

test('settings storage migration reads without repair and writes only on explicit quality change',()=>{
  const writes=[];
  const f=fixture({}, {}, {getItem:key=>key==='slither-client-settings-v1'?'1:0':'1',setItem:(...args)=>writes.push(args)});
  assert.deepEqual(f.calls.find(x=>x[0]==='restore_settings'),['restore_settings','1:0','1']);
  f.ui.frame(0); assert.deepEqual(writes,[]);
  f.emit(f.ui.quality,'click');
  assert.deepEqual(writes,[['slither-client-settings-v1','2:0:original'],['slither-client-quality-v1','0']]);
  f.ui.dispose();
  const failed=fixture({}, {}, {getItem(){throw Error('unreadable');},setItem(){throw Error('unwritable');}});
  assert.deepEqual(failed.calls.find(x=>x[0]==='restore_settings'),['restore_settings',undefined,'1']);
  assert.doesNotThrow(()=>failed.emit(failed.ui.quality,'click')); failed.ui.dispose();
});
test('retirement preserves adopted nodes and always frees policy after a DOM cleanup failure',()=>{
  const adopted=fixture();adopted.ui.root.ownerDocument={};adopted.ui.dispose();
  assert.equal(adopted.ui.root.removed,undefined);assert.equal(adopted.calls.filter(x=>x==='free').length,1);
  const failed=fixture();failed.ui.root.remove=()=>{throw Error('Synthetic cleanup failure');};
  assert.throws(()=>failed.ui.dispose(),/Synthetic cleanup failure/);
  assert.equal(failed.calls.filter(x=>x==='free').length,1);failed.ui.dispose();
});

test('terminal projection defers native release until outside HUD publication and fences stale returns',()=>{
  const f=fixture();assert.equal(f.ui.observeSession('ready'),false);
  assert.equal(f.ui.observeSession({phase:'failed'}),true);assert.equal(f.calls.includes('release'),false);
  f.ui.frame(16);assert.equal(f.calls.filter(x=>x==='release').length,1);
  f.stale();assert.equal(f.ui.observeSession({phase:'failed'}),false);f.ui.dispose();
});

test('last length is a split line and champion suppresses ordinary controls',()=>{
  const f=fixture({ordinary_controls_visible:false,champion_pending:true,champion_label:'Champion message handoff pending',last_label:'Your final length was',last_number:'1001',last_suffix:'!',last_label_alpha:.45});
  f.ui.frame(16);
  const node=role=>f.ui.root.children.find(n=>n.dataset.role===role);
  assert.deepEqual(node('last-length').children.map(n=>n.textContent),['Your final length was ','1001','!']);
  assert.equal(node('last-length').children[0].style.opacity,.45);
  for(const role of ['nickname','nickname-holder','play','skin'])assert.equal(node(role).style.display,'none');
  assert.equal(node('champion-pending').textContent,'Champion message handoff pending');
  f.ui.dispose();
});

test('WebGPU midpoint keeps canvas opacity at one and the independently faded HUD at one half',()=>{
  for(const fadeWorld of [false,true]) {
    const world={style:{opacity:1}},hudAlpha=.5;
    const f=fixture({world_alpha:.5,alpha:.5},{world,fadeWorld});
    f.ui.frame(16);
    assert.equal(world.style.opacity,fadeWorld?.5:1);
    if(!fadeWorld)assert.equal(hudAlpha*world.style.opacity,.5);
    f.ui.dispose();
  }
});


test('returning Play is enabled while nickname stays disabled and locks on click without admission',()=>{
  const f=fixture({disabled:true,play_disabled:false,alpha:.4,scale:1.036},{available:()=>false});
  f.ui.frame(16);
  assert.equal(f.ui.input.disabled,true);
  assert.equal(f.ui.button.disabled,false);
  f.emit(f.ui.button,'click');f.emit(f.ui.button,'click');
  assert.equal(f.actions.filter(a=>a[0]==='play').length,1);
  assert.equal(f.ui.button.disabled,true);
  f.emit(f.realm,'keydown',{code:'Escape'});
  assert.ok(f.calls.includes('cancel_pending'));
  f.ui.dispose();
});


test('return Enter delegates to policy and suppresses the native button default click',()=>{
  const f=fixture({disabled:true,play_disabled:false});
  f.ui.frame(16);
  const event=f.emit(f.realm,'keydown',{key:'Enter',code:'Enter',repeat:false});
  assert.equal(event.defaultPrevented,true);
  assert.deepEqual(f.actions,[['enter',false,true]]);
  f.ui.dispose();
});
