// Run from client/: node tools/generate-camera-weights.mjs <headless Edge executable>
// Evaluate in Chromium, never in Node's host Math implementation.
import { launchBrowser } from './feel-replay/browser.mjs';

if (process.argv.length !== 3) throw new Error('Expected a headless Edge executable');
const browser = await launchBrowser(process.argv[2]);
try {
  const version = await browser.call('Browser.getVersion');
  const { targetId } = await browser.call('Target.createTarget', { url: 'about:blank', background: true });
  const { sessionId } = await browser.call('Target.attachToTarget', { targetId, flatten: true });
  const result = await browser.call('Runtime.evaluate', {
    expression: `Array.from({ length: 62 }, (_, index) => {
      let weight = 0.5 * (1 - Math.cos(Math.PI * (61 - index) / 61));
      weight += (0.5 * (1 - Math.cos(Math.PI * weight)) - weight) * 0.5;
      const view = new DataView(new ArrayBuffer(8));
      view.setFloat64(0, weight, false);
      return view.getBigUint64(0, false).toString(16).padStart(16, '0');
    })`,
    returnByValue: true,
  }, sessionId);
  if (result.exceptionDetails) throw new Error(result.exceptionDetails.text);
  console.log(JSON.stringify({ browser: version.product, javascript: version.jsVersion, bits: result.result.value }, null, 2));
} finally {
  await browser.close();
}
