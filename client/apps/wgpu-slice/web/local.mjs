import init, { LocalGameplay, LoginMenu, game_backing_size, CosmeticImages } from './pkg/slither_wgpu_slice.js';
import { localHost } from './local-host.mjs';
import { installLocalInput } from './local-input.mjs';
import { loadCosmetics } from './cosmetics.mjs';
import { mountLogin } from './login-overlay.mjs';
import { loginArt } from './login-art.mjs';
const canvas = document.getElementById('game'), status = document.getElementById('status');
let host, owner, login, artLease, raf, disposed = false, endpoint = '', canPlay = false, cosmeticBytes = 0;
const abort = new AbortController(), documentAtStart = document, locationAtStart = location.href;
const current = () => !disposed && document === documentAtStart && location.href === locationAtStart;
function extent() {
  const [w,h]=game_backing_size(canvas.clientWidth,canvas.clientHeight);
  if(canvas.width!==w)canvas.width=w;
  if(canvas.height!==h)canvas.height=h;
  return [w,h,1];
}
function dispose() {
  if(disposed)return;
  disposed=true;cancelAnimationFrame(raf);abort.abort();login?.dispose();artLease?.dispose();
  if(host)host.dispose();else owner?.free();
  host=owner=undefined;
}
addEventListener('pagehide',dispose,{once:true});
try {
  if(!navigator.gpu)throw new Error('WebGPU unavailable');
  await init();
  if(current()) {
    owner=await LocalGameplay.create(canvas,performance.now());
    if(!current()){owner.free();owner=undefined;}
    else {
      try {
        const response=await fetch('./config.json',{credentials:'omit',signal:abort.signal});
        if(response.ok) {
          const config=await response.json();
          if(current()) {
            endpoint=config.simulatorUrl ?? '';
            if(config.cosmeticsBaseUrl) {
              const base=new URL(config.cosmeticsBaseUrl,location.href);
              if(base.origin!==location.origin)throw new Error('Same-origin cosmetics required');
              cosmeticBytes=await loadCosmetics({owner,CosmeticImages,baseUrl:base,accessory:config.accessory??-1,signal:abort.signal,isCurrent:current,includeMenu:false}) ?? 0;
            }
          }
        }
      } catch(error){if(current())console.warn(`Local resources unavailable: ${String(error)}`);}
      if(current()) {
        canPlay=owner.can_play();
        host=localHost({owner,socket:url=>new WebSocket(url),now:()=>performance.now(),extent,
          readout(text,available,failed,error){canPlay=available;status.textContent=failed?`Client unavailable: ${String(error)}`:'';}});
        const release=()=>host.call(()=>{for(const code of ['ArrowLeft','ArrowRight','ArrowUp','Space'])owner.key(code,false);owner.mouse_up();});
        artLease=loginArt({current,signal:abort.signal,pixelBudget:16*1024*1024-cosmeticBytes});
        // Rust covers the world before the independently fading HUD.
        login=mountLogin({LoginMenu,art:artLease.art,world:canvas,fadeWorld:false,isCurrent:current,available:()=>canPlay&&!!endpoint,
          playing:()=>owner.playing(),release,cancel:()=>host.call(()=>owner.stop()),
          play(nickname,quality){host.call(()=>{owner.requested_quality(quality==='high');owner.play(endpoint,nickname,0);});}});
        installLocalInput({realm:window,canvas,owner,host,start:()=>login.activate('enter'),controls:[login.button,login.quality],signal:abort.signal});
        function draw(time){if(current()){host.frame(time);const state=owner.login_view();login.observeSession(state,state);login.frame(time);raf=requestAnimationFrame(draw);}}
        raf=requestAnimationFrame(draw);
      } else {owner?.free();owner=undefined;}
    }
  }
} catch(error){if(current())status.textContent=`Client unavailable: ${String(error)}`;dispose();}
