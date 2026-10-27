// Inspect a separately retained stock client; never fetch it or run its app.
import { readFileSync } from 'node:fs';
import { createHash } from 'node:crypto';
import { runInNewContext } from 'node:vm';
import assert from 'node:assert/strict';

const bytes = readFileSync(process.argv[2]);
const sha256 = createHash('sha256').update(bytes).digest('hex');
assert.equal(sha256, '5ad8aa8f06d1e71bad2945001bab077f8d4d279c093a046d19b6f26ac6b62095');
const source = bytes.toString('utf8');
const start = source.indexOf('function newFood(');
const definition = source.slice(start, source.indexOf('function newPrey(', start));
const fadeStart = source.indexOf('if(fo.fr!=1)');
const fade = source.slice(fadeStart, source.indexOf('fo.rx=fo.xx', fadeStart));
assert.ok(definition.startsWith('function newFood(') && fade.includes('fo.rsp*vfr/150'));
assert.ok(source.includes('vfr=(ctm-ltm)/8'));
// Sprite references are unused by the fade update; no canvas or DOM is needed.
const sprite = new Proxy({ ic: 1 }, { get: (o, key) => key in o ? o[key] : [0] });
const context = { gsc: 1, foods: [], foods_c: 0, per_color_imgs: Array(10).fill(sprite) };
runInNewContext(definition + '\nthis.make = newFood;', context);
const duration = rapid => {
  context.fo = context.make(1, 0, 0, 6.2, rapid, 8);
  context.vfr = 20 / 8;
  let elapsed = 0;
  while (context.fo.fr < 1 && elapsed < 2000) {
    runInNewContext(fade, context);
    elapsed += 20;
  }
  return elapsed;
};
const normalMs = duration(false);
const rapidMs = duration(true);
assert.ok(normalMs >= 1200 && normalMs <= 1220);
assert.ok(rapidMs >= 400 && rapidMs <= 420);
console.log(JSON.stringify({ stockClientSha256: sha256, normalMs, rapidMs }));
