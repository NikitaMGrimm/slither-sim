// Optional startup resources. Each path is attempted once; failures leave Play available.
export async function loadCosmetics({ owner, CosmeticImages, baseUrl, accessory = -1, signal,
  fetch: request = globalThis.fetch, warn = message => console.warn(message) }) {
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
    try { images.manifest(await read('menu/manifest.txt', 4096)); }
    catch (error) {
      if (signal?.aborted) return;
      warn(`Menu manifest unavailable: ${String(error)}`);
    }
    for (const path of [...CosmeticImages.paths(), ...images.menu_paths()]) {
      try {
        images.add(path, await read(path, 4 * 1024 * 1024));
      } catch (error) {
        if (signal?.aborted) return;
        warn(`Cosmetic ${path} unavailable: ${String(error)}`);
      }
    }
    const ready = images; images = undefined;
    owner.cosmetics(ready, accessory);
  } finally { images?.free(); }
}
