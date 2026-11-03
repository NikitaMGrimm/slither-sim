// The only NTL-specific adapter: visible UI controls, never game globals or
// minified functions. Updated mods must preserve these labels or fail preflight.
import { setTimeout as delay } from 'node:timers/promises';
export async function configureNTL(page, { enableZoom }) {
  const opened = await page.evaluate(`(()=>{
    const button=[...document.querySelectorAll('button')].find(e=>e.textContent.trim()==='Open Settings');
    if(!button)return false;button.click();return true;
  })()`);
  if (!opened) throw Error('NTL UI preflight: missing Open Settings button in local login; no settings or view inferred');
  for (let i=0;i<50;i++) {
    if (await page.evaluate(`Boolean([...document.querySelectorAll('input[type=checkbox]')].find(e=>/enable.*disable zoom mod/i.test(e.title)))`)) break;
    await delay(100);
  }
  const settings = await page.evaluate(`(()=>{
    const zoom=[...document.querySelectorAll('input[type=checkbox]')].find(e=>/enable.*disable zoom mod/i.test(e.title));
    if(!zoom)throw Error('Missing Enable zoom checkbox');
    const before=zoom.checked;
    if(${Boolean(enableZoom)}&&!zoom.checked)zoom.click();
    const release=[...document.querySelectorAll('input[type=checkbox]')].find(e=>/always get back slowly/i.test(e.title));
    if(${Boolean(enableZoom)}&&!release)throw Error('Missing slow zoom return checkbox');
    if(${Boolean(enableZoom)}&&release?.checked)release.click();
    return {zoomEnabledBefore:before,zoomEnabled:zoom.checked,slowReturnEnabled:release?.checked??null,
      controls:[...document.querySelectorAll('button,input,select')].filter(e=>e.getBoundingClientRect().width>0).map(e=>({tag:e.tagName,type:e.type,id:e.id,text:e.textContent,title:e.title,value:e.value,checked:e.checked}))};
  })()`);
  const closed=await page.evaluate(`(()=>{const button=[...document.querySelectorAll('#settpage button')].find(e=>e.textContent.trim()==='Close');if(!button)return false;button.click();return true;})()`);
  if(!closed)throw Error('NTL UI preflight: missing Close settings button');
  settings.zoomSettings={zoomEnabled:settings.zoomEnabled,slowReturnEnabled:settings.slowReturnEnabled,controls:settings.controls.filter(c=>/zoom/i.test(c.title??''))
    .map(({id,type,value,checked})=>({id,type,value,checked})).sort((a,b)=>a.id.localeCompare(b.id))};
  await delay(500);
  return settings;
}
