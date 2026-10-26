/**
 * Closed-loop Node.js reference controller for tools/perf_transport.py (built-ins only).
 *
 * Usage: node tools/perf_transport_client.mjs BINARY REQUESTS [SERVE_ARG ...]
 *
 * Sends each pre-encoded request line from REQUESTS to `BINARY serve SERVE_ARG...`, waits for its
 * reply, parses it and runs a small decision over every living slot. The decision
 * is not sent; requests stay scripted. Prints one JSON line with the summed timings.
 */
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { readFileSync } from 'node:fs';
import { createInterface } from 'node:readline';

// Protocol slither-sim-json-v5: reply decoding and the decision step. Another
// protocol replaces these two functions and leaves the timing loop unchanged.
const protocol = 'slither-sim-json-v5';

function decodeReply(line) {
  const reply = JSON.parse(line);
  if (reply.protocol !== protocol) throw new Error(`Unexpected protocol ${reply.protocol}`);
  if ('error' in reply) throw new Error(`serve error: ${reply.error.message}`);
  return reply.result;
}

/** Same fold as the Python client; the runner requires equal checksums. */
function decide(result, checksum) {
  let living = 0;
  let foodSeen = 0;
  for (const frame of result.worlds) {
    const { x: snakeX, y: snakeY } = frame.snakes;
    const { x: foodX, y: foodY } = frame.food;
    for (const view of frame.views) {
      if (view === null) continue;
      const x = snakeX[view.self];
      const y = snakeY[view.self];
      const food = view.food;
      let best = -1;
      let nearest = Infinity;
      for (const row of food) {
        const dx = foodX[row] - x;
        const dy = foodY[row] - y;
        const distance = dx * dx + dy * dy;
        if (distance < nearest) {
          best = row;
          nearest = distance;
        }
      }
      living += 1;
      foodSeen += food.length;
      checksum = (Math.imul(checksum, 1000003) + best + 1 + 7 * food.length
        + 13 * view.snakes.length) >>> 0;
    }
  }
  return { checksum, living, foodSeen };
}

const [binary, requestsPath, ...serveArgs] = process.argv.slice(2);
if (!binary || !requestsPath) {
  throw new Error('Usage: node tools/perf_transport_client.mjs BINARY REQUESTS [SERVE_ARG ...]');
}
const requests = readFileSync(requestsPath, 'utf8').split('\n').filter(line => line !== '').map(line => `${line}\n`);

const child = spawn(binary, ['serve', ...serveArgs], { stdio: ['pipe', 'pipe', 'inherit'] });
await once(child, 'spawn');
const finished = once(child, 'close');
const lines = createInterface({ input: child.stdout, crlfDelay: Infinity });
const replies = lines[Symbol.asyncIterator]();
const clock = process.hrtime.bigint;
const totals = { steps: 0, replyBytes: 0, waitNs: 0, parseNs: 0, decideNs: 0, living: 0, foodSeen: 0 };
let reset = null;
let checksum = 0;
try {
  for (const [index, request] of requests.entries()) {
    const start = clock();
    child.stdin.write(request);
    const { value: line, done } = await replies.next();
    const received = clock();
    if (done) throw new Error('serve closed its output');
    const result = decodeReply(line);
    const parsed = clock();
    const decision = decide(result, checksum);
    const decided = clock();
    checksum = decision.checksum;
    const sample = {
      replyBytes: Buffer.byteLength(line) + 1,
      waitNs: Number(received - start),
      parseNs: Number(parsed - received),
      decideNs: Number(decided - parsed),
      living: decision.living,
      foodSeen: decision.foodSeen,
    };
    if (index === 0) {
      reset = sample; // The reset reply is reported separately from steps.
      continue;
    }
    totals.steps += 1;
    for (const [key, value] of Object.entries(sample)) totals[key] += value;
  }
} finally {
  child.stdin.end();
  const timeout = setTimeout(() => child.kill(), 30000);
  try {
    await finished;
  } finally {
    clearTimeout(timeout);
    lines.close();
  }
}
if (child.exitCode !== 0) throw new Error(`serve exited with ${child.exitCode ?? child.signalCode}`);
console.log(JSON.stringify({ client: 'node', runtime: process.version, checksum, reset, ...totals }));
