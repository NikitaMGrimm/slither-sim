// One semantic DOM adapter for Canvas and WebGPU. All animation, normalization,
// rectangles and Play locking come from LoginMenu; no CSS animation or timers.
export function mountLogin({ realm = globalThis, LoginMenu, parent = realm.document.body,
  isCurrent = () => true, available, playing, play, release, nickname = '', art = {}, world, fadeWorld = true, onEdit = () => {}, cancel = () => {} }) {
  const document = realm.document, location = realm.location.href;
  const locationObject=realm.location, top=realm.top;
  let disposed = false, composing = false, imeEnter = false, pendingClick = false, releasePending = false, view;
  const abort = new realm.AbortController();
  const documentCurrent=()=>realm.document===document && realm.location===locationObject && realm.location.href===location && realm.top===top;
  const current = () => !disposed && isCurrent() && documentCurrent();
  const listen = (target, type, fn) => target.addEventListener(type, e => { if (current()) fn(e); }, { signal: abort.signal });
  let stored = '1';
  try { stored = realm.localStorage.getItem('slither-client-quality-v1') ?? '1'; } catch { /* Memory remains authoritative. */ }
  const policy = new LoginMenu(realm.navigator.language, stored);
  const root = document.createElement('div'); root.dataset.login = '';
  root.style.cssText = 'position:fixed;inset:0;z-index:20;pointer-events:none;color:#e0e0ff;font-family:Arial,sans-serif';
  const element = (tag, name, text = '') => {
    const node = document.createElement(tag); node.dataset.role = name; node.textContent = text;
    node.style.cssText = 'position:absolute;box-sizing:border-box;margin:0;padding:0;border:0;text-align:center;';
    root.appendChild(node); return node;
  };
  const logo = element('div', 'logo', 'slither');
  logo.style.cssText += 'font-weight:bold;color:#82d65b;display:flex;align-items:center;justify-content:center;text-shadow:0 4px 12px #000';
  const last = element('div', 'last-length', '\u00a0');
  last.textContent='';last.setAttribute('role','status');
  const lastLabel=document.createElement('span'),lastNumber=document.createElement('b'),lastSuffix=document.createElement('span');
  last.appendChild(lastLabel);last.appendChild(lastNumber);last.appendChild(lastSuffix);
  const champion = element('div','champion-pending');champion.setAttribute('role','status');
  const tips = element('div', 'tips');
  const holder = element('div', 'nickname-holder');
  holder.style.cssText += 'background:#4c447c;border:2px solid #000;border-radius:29px;box-shadow:0 6px 35px #0009';
  const input = element('input', 'nickname');
  input.type = 'text'; input.maxLength = 24; input.autocomplete = 'off'; input.spellcheck = false;
  input.style.cssText += 'pointer-events:auto;background:transparent;color:#e0e0ff;text-align:left;outline:none;border-radius:0';
  input.value = policy.edit(nickname);
  const button = element('button', 'play', 'Play'); button.type = 'button';
  button.style.cssText += 'pointer-events:auto;overflow:hidden;border-radius:34px;font-weight:bold;color:white;box-shadow:0 3px 20px #000b;cursor:pointer';
  button.textContent='';
  const buttonLayers=['linear-gradient(#75f0ca,#48ab84)','linear-gradient(#0c1412,#283b36)'].map(background=>{
    const layer=document.createElement('span');layer.setAttribute('aria-hidden','true');
    layer.style.cssText=`position:absolute;inset:0;border-radius:inherit;pointer-events:none;background:${background}`;
    button.appendChild(layer);return layer;
  });
  const buttonLabel=document.createElement('span');buttonLabel.textContent='Play';buttonLabel.style.cssText='position:relative;pointer-events:none';button.appendChild(buttonLabel);
  const skin = element('div', 'skin');
  // PR6 supplies the chooser. This is an intentionally passive, labelled icon.
  skin.setAttribute('role', 'img');
  skin.style.cssText += 'display:flex;flex-direction:column;justify-content:center;align-items:center;font-size:14px;color:#b7b1de';
  const quality = element('button', 'quality'); quality.type = 'button';
  quality.style.cssText += 'pointer-events:auto;background:transparent;color:#b7b1de;font:14px Arial;cursor:pointer';
  const spinner = element('div', 'spinner'); spinner.setAttribute('role','status'); spinner.setAttribute('aria-label','Connecting');
  const dots = Array.from({length:16}, () => { const dot=document.createElement('i'); dot.style.cssText='position:absolute;border-radius:50%'; spinner.appendChild(dot); return dot; });
  const measure = document.createElement('canvas').getContext('2d');
  const started = realm.performance.now();
  const activate = kind => {
    if (!current() || (kind==='play' && button.disabled)) return;
    if (!composing) input.value = policy.edit(input.value);
    policy.action(kind, composing || (kind==='enter' && imeEnter), available());
    // Immediate projection disables the DOM before a second click. step() alone
    // emits the frozen request on the next animation update.
    if ((kind === 'play' || kind === 'enter') && !composing && (kind!=='enter' || !imeEnter) && (kind==='play' || available())) {
      input.disabled = button.disabled = true;
    }
  };
  listen(input,'input', () => { if (!composing) {input.value=policy.edit(input.value);onEdit(input.value);} });
  listen(input,'compositionstart', () => {composing=true;});
  listen(input,'compositionend', () => {
    composing=false; input.value=policy.edit(input.value);
    onEdit(input.value);
    if (pendingClick) {pendingClick=false; activate('play');}
  });
  listen(input,'keydown', e => {
    if (e.key==='Enter' || e.code==='Enter' || e.code==='NumpadEnter') {
      if (e.isComposing || composing || e.keyCode===229 || e.key==='Process') imeEnter=true;
      else if (e.key==='Enter' && !imeEnter && !e.repeat) {e.preventDefault(); activate('enter');}
    }
  });
  listen(realm,'keydown', e => {
    if(view?.disabled && !view.play_disabled && e.key==='Enter') {
      // A focused native button must not turn a rejected Enter into a click.
      e.preventDefault();
      if(!e.repeat) activate('enter');
    }
    if(e.code==='Escape' && view?.disabled) {policy.cancel_pending_play();cancel();}
  });
  listen(realm,'keyup', e => {if (e.code==='Enter' || e.code==='NumpadEnter') imeEnter=false;});
  listen(button,'click', () => {if (composing) pendingClick=true; else activate('play');});
  for (const [type,hover,press] of [['pointerenter',true,false],['pointerleave',false,false],['pointerdown',true,true],['pointerup',true,false],['pointercancel',false,false]]) {
    listen(button,type,()=>policy.pointer(hover,press,available()));
  }
  listen(quality,'click',()=> {
    const value=policy.action('quality',false,available());
    if(value!==undefined) try {realm.localStorage.setItem('slither-client-quality-v1',value);} catch { /* Best effort only. */ }
  });
  listen(root,'focusin',()=>release());
  listen(root,'focusout',()=>release());
  listen(realm,'blur',()=> {imeEnter=false;pendingClick=false;release();});
  const place = (node,r) => {const [x,y,w,h]=r; Object.assign(node.style,{left:`${x}px`,top:`${y}px`,width:`${w}px`,height:`${h}px`});};
  const image = (node, role, fallback) => {
    if (node.dataset.art===role && node.dataset.source===(art[role]??'')) return;
    node.dataset.art=role;node.dataset.source=art[role]??''; node.textContent='';
    if(art[role]) {const img=document.createElement('img');img.src=art[role];img.alt='';img.style.cssText='width:100%;height:100%;object-fit:fill';node.appendChild(img);}
    else if(role==='skin') {
      const icon=document.createElement('span'),label=document.createElement('span');
      icon.style.cssText='position:absolute;left:23px;top:36px;width:48px;height:25px;border-radius:20px;background:linear-gradient(#56ac81,#366c51)';
      label.textContent=fallback;label.style.cssText='position:absolute;left:0;bottom:0;width:100%;height:24px;line-height:24px';
      node.appendChild(icon);node.appendChild(label);
    }else node.textContent=fallback;
  };
  parent.appendChild(root);
  return {
    root, input, button, quality, activate,
    observeSession(session, capture) {
      if (!current()) return false;
      const returned=policy.observe_life(session.phase,session.outcome??'',session.playing,
        session.deathPending,session.returnActive,session.returnFraction,capture?.score,capture?.ownToken);
      if (!returned) return false;
      composing=imeEnter=pendingClick=false;
      releasePending=true;
      return true;
    },
    frame(now) {
      if (!current()) return;
      if(releasePending){releasePending=false;release();}
      measure.font='bold 20px Arial';
      // RAF's frame-start timestamp may precede mounting later in that frame.
      view=policy.step(Math.max(0,now-started),realm.innerWidth,realm.innerHeight,measure.measureText(policy.play_label()).width,available(),playing());
      for(const [node,key] of [[logo,'logo'],[last,'last'],[tips,'tips'],[holder,'holder'],[input,'input'],[button,'play'],[skin,'skin'],[quality,'quality_rect'],[spinner,'spinner']]) place(node,view[key]);
      root.style.display=view.visible || view.spinner_visible ? 'block':'none';
      for(const node of [holder,input,button,skin])node.style.display=view.ordinary_controls_visible?'':'none';
      champion.style.display=view.champion_pending?'block':'none';
      place(champion,view.holder);champion.textContent=view.champion_label;champion.style.opacity=view.alpha;
      champion.style.fontSize=`${15*view.scale}px`;
      last.style.fontSize=`${15*view.scale}px`;last.style.lineHeight=`${view.last[3]}px`;last.style.color='white';
      lastLabel.textContent=view.last_label?view.last_label+' ':'';lastLabel.style.opacity=view.last_label_alpha;
      lastNumber.textContent=view.last_number;lastSuffix.textContent=view.last_suffix;
      if(world)world.style.opacity=fadeWorld?view.world_alpha:1;
      for(const [node,alpha] of [[logo,view.logo_alpha],[last,view.alpha],[tips,view.tip_alpha],[holder,view.nickname_alpha],[input,view.nickname_alpha],[button,view.play_alpha],[skin,view.skin_alpha],[quality,view.alpha]]) node.style.opacity=alpha;
      logo.style.fontSize=`${40*view.scale}px`; input.style.font=`${15*view.scale}px Arial`;
      tips.style.fontSize=`${14*view.scale}px`;tips.style.lineHeight=`${view.tips[3]}px`;
      button.style.fontSize=`${20*view.scale}px`;button.style.background=`linear-gradient(${view.busy?'#4b5b54,#32413a':'#56ac81,#366c51'})`;
      buttonLayers[0].style.opacity=view.hover;buttonLayers[1].style.opacity=view.press;buttonLabel.style.opacity=view.busy?.45:1;
      input.placeholder=view.placeholder;input.setAttribute('aria-label',view.placeholder);
      input.disabled=quality.disabled=view.disabled; button.disabled=view.play_disabled;
      input.style.opacity=view.nickname_alpha*(view.busy?.5:1);
      if(!composing && input.value!==view.nickname) input.value=view.nickname;
      buttonLabel.textContent=view.play_label;tips.textContent=view.tip;
      image(logo,'logo:0','slither');image(skin,'skin',view.skin_label);skin.setAttribute('aria-label',`${view.skin_label} (preview unavailable)`);
      image(quality,`quality-${view.quality}`,view.quality_caption);
      quality.setAttribute('aria-label',`${view.quality_label}: ${view.quality_value}`);quality.setAttribute('aria-pressed',String(view.quality==='high'));
      spinner.setAttribute('aria-label',view.connecting_label);
      spinner.style.display=view.spinner_visible?'block':'none';spinner.style.opacity=view.spinner_alpha;
      for(const [i,dot] of dots.entries()) {const phase=i*Math.PI/8+view.spinner_time*.05,s=view.spinner_scale;
        Object.assign(dot.style,{left:`${64+40*s*Math.cos(phase)-5*s}px`,top:`${64+40*s*Math.sin(phase)-5*s}px`,width:`${10*s}px`,height:`${10*s}px`,background:i<8?'#6cd780':'#966cd5',opacity:.35+.65*i/15});}
      if(view.focus && !view.disabled) input.focus();
      if(Object.hasOwn(view,'request')) {release();play(view.request,view.requested_quality);}
      return view;
    },
    dispose() {
      if(disposed)return;disposed=true;abort.abort();
      try {if(documentCurrent() && root.ownerDocument===document && root.parentNode===parent)root.remove();}
      finally {policy.free();}
    },
  };
}
