import assert from 'node:assert/strict';
import { test } from 'node:test';
import { mountLogin } from '../apps/wgpu-slice/web/login-overlay.mjs';

// Authored DOM seam; exact policy is exercised by login-abi.mjs in both Wasm builds.
function fixture() {
  class Node extends EventTarget {
    style={};dataset={};children=[];textContent='';value='';disabled=false;
    appendChild(n){this.children.push(n);n.parentNode=this;return n;}setAttribute(k,v){this[k]=v;}
    focus(){this.focused=true;}remove(){this.removed=true;}
    getContext(){return {measureText:()=>({width:36})};}
  }
  const realm=new EventTarget();Object.assign(realm,{AbortController,navigator:{language:'en'},performance:{now:()=>0},innerWidth:960,innerHeight:540,location:{href:'http://127.0.0.1:19090/'},localStorage:{getItem(){return null;},setItem(){throw Error('Unavailable');}}});
  realm.document={body:new Node(),createElement:()=>{const n=new Node();n.ownerDocument=realm.document;return n;}};
  const actions=[],calls=[];let alive=true, composingValue='';
  class Policy {
    edit(value){composingValue=value;return value;}
    action(...args){actions.push(args);return args[0]==='quality'?'0':undefined;}
    observe_session(phase){calls.push(['terminal',phase]);return phase==='failed';}
    pointer(...args){calls.push(args);}
    step(){return Object.fromEntries(['logo','last','tips','holder','input','play','skin','quality_rect','spinner'].map(k=>[k,[0,0,100,30]]).concat(Object.entries({visible:true,disabled:false,alpha:1,logo_alpha:1,nickname_alpha:1,play_alpha:1,skin_alpha:1,tip_alpha:1,hover:0,press:0,scale:1,nickname:composingValue,placeholder:'Nickname',play_label:'Play',tip:'Eat',skin_label:'Change Skin',quality_label:'Graphics quality',quality:'high',spinner_visible:false,spinner_alpha:0,spinner_scale:0,spinner_time:0})));}
    free(){calls.push('free');}
  }
  const ui=mountLogin({realm,LoginMenu:Policy,isCurrent:()=>alive,available:()=>true,playing:()=>false,play:()=>calls.push('play'),release:()=>calls.push('release')});
  const emit=(node,type,fields={})=>{const e=new Event(type,{cancelable:true});Object.assign(e,fields);node.dispatchEvent(e);};
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
test('retirement preserves adopted nodes and always frees policy after a DOM cleanup failure',()=>{
  const adopted=fixture();adopted.ui.root.ownerDocument={};adopted.ui.dispose();
  assert.equal(adopted.ui.root.removed,undefined);assert.equal(adopted.calls.filter(x=>x==='free').length,1);
  const failed=fixture();failed.ui.root.remove=()=>{throw Error('Synthetic cleanup failure');};
  assert.throws(()=>failed.ui.dispose(),/Synthetic cleanup failure/);
  assert.equal(failed.calls.filter(x=>x==='free').length,1);failed.ui.dispose();
});

test('terminal projection defers native release until outside HUD publication and fences stale returns',()=>{
  const f=fixture();assert.equal(f.ui.observeSession('ready'),false);
  assert.equal(f.ui.observeSession('failed'),true);assert.equal(f.calls.includes('release'),false);
  f.ui.frame(16);assert.equal(f.calls.filter(x=>x==='release').length,1);
  f.stale();assert.equal(f.ui.observeSession('failed'),false);f.ui.dispose();
});
