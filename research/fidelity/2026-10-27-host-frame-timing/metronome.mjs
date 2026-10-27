// Synthetic loopback-only cadence probe: one outstanding game ping, stock bytes.
// The natural headless runs remain separate; this intentionally prevents silence.
import fs from 'node:fs';
import path from 'node:path';
import { Recorder, loginPacket, splitFrame } from '../../../tools/fidelity/slither-headless.mjs';

const [out, portText, secondsText] = process.argv.slice(2);
const port = Number(portText), seconds = Number(secondsText);
if (!Number.isInteger(port) || port < 8790 || port > 8899 || !(seconds > 0 && seconds <= 120))
  throw new Error('use study loopback ports and 1..120 seconds');
fs.mkdirSync(out, { recursive: false });
const rec = new Recorder(path.join(out, 'bot0.jsonl.gz'), {
  schema: 1, dataOrigin: 'local-simulator', scenario: 'synthetic-pong-clock',
  server: { ip: '127.0.0.1', port }, t: performance.now(), wall: Date.now(),
});
let failure = null, setup = false, pending = false, pings = 0, pongs = 0;
const ws = new WebSocket(`ws://127.0.0.1:${port}/slither`);
ws.binaryType = 'arraybuffer';
const send = bytes => { rec.frame('o', bytes); ws.send(bytes); };
const ping = () => { if (!pending) { pending = true; pings++; send(Uint8Array.of(251)); } };
ws.onopen = () => { rec.meta({ event: 'connect' }); send(loginPacket('clock')); };
ws.onmessage = event => {
  const bytes = new Uint8Array(event.data);
  rec.frame('i', bytes);
  for (const packet of splitFrame(bytes)) {
    if (packet[0] === 97) { setup = true; ping(); }
    if (packet[0] === 112) { pongs++; pending = false; ping(); }
  }
};
ws.onerror = () => { failure = 'websocket error'; };
await new Promise(resolve => setTimeout(resolve, seconds * 1000));
ws.onmessage = null;
ws.close();
await new Promise(resolve => { ws.onclose = resolve; setTimeout(resolve, 1000); });
rec.finish({ event: 'capture-complete', setup, pings, pongs, failure });
await rec.close();
if (!setup || failure || pongs < seconds * 50) throw new Error('probe failed; retained');
