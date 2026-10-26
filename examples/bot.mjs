/** Control a local simulator with Node.js's standard library. */
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';

const protocol = 'slither-sim-json-v5';
const filename = process.platform === 'win32' ? 'slither-sim.exe' : 'slither-sim';
const [binary = fileURLToPath(new URL(`../target/release/${filename}`, import.meta.url)), count = '200'] = process.argv.slice(2);
const steps = Number(count);
if (!Number.isSafeInteger(steps) || steps < 1) {
  throw new Error('Usage: node examples/bot.mjs [binary] [positive step count]');
}

/** Steer to the nearest visible food, or back toward the center near the rim. */
function heading(frame, view) {
  const { snakes, food } = frame;
  const x = snakes.x[view.self];
  const y = snakes.y[view.self];
  let best = Math.atan2(-y, -x);
  if (Math.hypot(x, y) < frame.radius * 0.8) {
    let distance = Infinity;
    for (const i of view.food) {
      const candidate = (food.x[i] - x) ** 2 + (food.y[i] - y) ** 2;
      if (candidate < distance) {
        distance = candidate;
        best = Math.atan2(food.y[i] - y, food.x[i] - x);
      }
    }
  }
  return best;
}

function actions(frame) {
  return {
    heading: frame.views.map(view => (view === null ? null : heading(frame, view))),
    boost: frame.views.map(() => false),
  };
}

const child = spawn(binary, ['serve'], { stdio: ['pipe', 'pipe', 'inherit'] });
await once(child, 'spawn');
const finished = once(child, 'close');
const lines = createInterface({ input: child.stdout, crlfDelay: Infinity });
const replies = lines[Symbol.asyncIterator]();

async function request(message) {
  child.stdin.write(`${JSON.stringify(message)}\n`);
  const { value, done } = await replies.next();
  if (done) throw new Error('Simulator closed its output');
  const reply = JSON.parse(value);
  if (reply.protocol !== protocol) throw new Error('Incompatible simulator protocol');
  if ('error' in reply) throw new Error(reply.error.message);
  return reply.result;
}

try {
  // One world; pass worlds: N to step N independent worlds per request.
  let [frame] = (await request({ op: 'reset', seed: 7, agents: 4 })).worlds;
  let completed = 0;
  while (completed < steps && frame.living > 0) {
    [frame] = (await request({ op: 'step', actions: [actions(frame)] })).worlds;
    completed += 1;
  }
  console.log(`decisions=${completed} time=${frame.time.toFixed(3)}s living=${frame.living}`);
} finally {
  child.stdin.end();
  const timeout = setTimeout(() => child.kill(), 5000);
  try {
    await finished;
  } finally {
    clearTimeout(timeout);
    lines.close();
  }
}
if (child.exitCode !== 0) throw new Error(`Simulator exited with ${child.exitCode ?? child.signalCode}`);
