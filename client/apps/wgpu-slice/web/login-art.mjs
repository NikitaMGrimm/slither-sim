import { menuEntries, DEFAULT_MENU_PATHS } from './menu-art.mjs';

// Inspect dimensions before asking the browser to allocate decoded pixels.
export function menuImageSize(bytes) {
  const data=new DataView(bytes.buffer,bytes.byteOffset,bytes.byteLength);
  let size;
  if(bytes.length>=24 && data.getUint32(0)===0x89504e47 && data.getUint32(4)===0x0d0a1a0a
    && data.getUint32(12)===0x49484452) size=[data.getUint32(16),data.getUint32(20)];
  else if(bytes[0]===0xff&&bytes[1]===0xd8) {
    for(let i=2;i+4<=bytes.length;) {
      if(bytes[i++]!==0xff)throw new Error('Malformed JPEG marker');
      while(bytes[i]===0xff)i++;
      const marker=bytes[i++];
      if(marker===0xda||marker===0xd9)break;
      if(marker===0x01||(marker>=0xd0&&marker<=0xd7))continue;
      if(i+2>bytes.length)break;
      const length=data.getUint16(i);
      if(length<2||i+length>bytes.length)break;
      if([0xc0,0xc1,0xc2,0xc3,0xc5,0xc6,0xc7,0xc9,0xca,0xcb,0xcd,0xce,0xcf].includes(marker)&&length>=7){size=[data.getUint16(i+5),data.getUint16(i+3)];break;}
      i+=length;
    }
  }
  if(!size||size.some(n=>n<1||n>1024))throw new Error('Menu art dimensions/format');
  return size;
}

// Optional local art only. Failed reads/decode leave the authored controls intact.
// Callers retain this lease before awaiting and retire it on generation teardown.
export function loginArt({ realm = globalThis, current, signal, base = '/cosmetics/', pixelBudget = 16*1024*1024 }) {
  const urls = [], art = {};
  let retired = false, pixelBytes = 0;
  const read = async (path, limit) => {
    const url = new URL(base + path, realm.location.href);
    if (url.origin !== realm.location.origin) throw new Error('Same-origin menu art required');
    const response = await realm.fetch(url,{credentials:'omit',redirect:'error',signal});
    if(!response.ok)throw new Error('Optional menu art unavailable');
    const reader=response.body.getReader(), chunks=[];let size=0;
    try {for(;;){const {value,done}=await reader.read();if(done)break;size+=value.byteLength;if(size>limit)throw new Error('Menu art byte budget');chunks.push(value);}}
    finally {await reader.cancel();reader.releaseLock();}
    const bytes=new Uint8Array(size);let offset=0;
    for(const chunk of chunks){bytes.set(chunk,offset);offset+=chunk.byteLength;}
    return bytes;
  };
  const ready=(async()=>{
    let entries=DEFAULT_MENU_PATHS.map((path,i)=>({role:['logo:0','skin','builder','cosmetic','quality-high','quality-low'][i],path}));
    try{entries=menuEntries(await read('menu/manifest.txt',4096));}catch{ /* Defaults match native. */ }
    for(const {role,path} of entries.filter(e=>['logo:0','skin','quality-high','quality-low'].includes(e.role))){
      if(retired||!current())break;
      try {
        const bytes=await read(path,4*1024*1024);
        if(retired||!current())break;
        const [width,height]=menuImageSize(bytes),charge=width*height*4;
        if(pixelBytes+charge>pixelBudget)throw new Error('Menu art pixel budget');
        const url=realm.URL.createObjectURL(new realm.Blob([bytes]));urls.push(url);
        const image=new realm.Image();image.src=url;await image.decode();
        if(image.naturalWidth>1024||image.naturalHeight>1024||!image.naturalWidth||!image.naturalHeight)throw new Error('Menu art dimensions');
        if(!retired&&current()){art[role]=url;pixelBytes+=charge;}
      }catch{ /* Missing/malformed art retains the fallback. */ }
    }
    return art;
  })();
  return {art,ready,dispose(){retired=true;for(const url of urls)realm.URL.revokeObjectURL(url);urls.length=0;}};
}
