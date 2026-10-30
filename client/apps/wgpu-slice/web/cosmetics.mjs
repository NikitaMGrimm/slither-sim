import { menuEntries } from './menu-art.mjs';
// Optional startup resources. Each path is attempted once; failures leave Play available.
export async function loadCosmetics({ owner, CosmeticImages, baseUrl, accessory = -1, signal, includeMenu = true,
  isCurrent = () => true, fetch: request = globalThis.fetch, warn = message => console.warn(message) }) {
  let images = new CosmeticImages();
  try {
    const localBase = new URL(baseUrl);
    if (!['http:', 'https:'].includes(localBase.protocol) || localBase.username || localBase.password
      || (globalThis.location && localBase.origin !== globalThis.location.origin)) throw new Error('Local served cosmetics required');
    const read = async (path, limit) => {
        const response = await request(new URL(path, baseUrl), { credentials: 'omit', redirect: 'error', signal });
        if (!response.ok) throw new Error(`HTTP ${response.status}`);
        const reader = response.body.getReader();
        const chunks = []; let size = 0;
        try {
          for (;;) {
            const { value, done } = await reader.read(); if (done) break;
            size += value.byteLength;
            if (size > limit) throw new Error('Image budget');
            chunks.push(value);
          }
        } finally { await reader.cancel(); reader.releaseLock(); }
        const bytes = new Uint8Array(size); let offset = 0;
        for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.byteLength; }
        return bytes;
    };
    const cosmeticPaths = CosmeticImages.paths();
    try {
      const manifest = await read('menu/manifest.txt', 4096);
      menuEntries(manifest,cosmeticPaths);
      images.manifest(manifest);
    }
    catch (error) {
      if (signal?.aborted || !isCurrent()) return;
      warn(`Menu manifest unavailable: ${String(error)}`);
    }
    for (const path of [...cosmeticPaths, ...(includeMenu ? images.menu_paths() : [])]) {
      try {
        const bytes=await read(path,4*1024*1024);
        if(signal?.aborted || !isCurrent())return;
        images.add(path,bytes);
      } catch (error) {
        if (signal?.aborted || !isCurrent()) return;
        warn(`Cosmetic ${path} unavailable: ${String(error)}`);
      }
    }
    if(signal?.aborted || !isCurrent())return;
    const pixelBytes=includeMenu ? undefined : images.pixel_bytes();
    const ready = images; images = undefined;
    owner.cosmetics(ready, accessory);
    return pixelBytes;
  } finally { images?.free(); }
}
