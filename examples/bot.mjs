/** Control a local simulator with Node.js's standard library. */
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { createInterface } from 'node:readline';
import { fileURLToPath } from 'node:url';

const protocol = 'slither-sim-json-v1';
const filename = process.platform === 'win32' ? 'slither-sim.exe' : 'slither-sim';
const [binary = fileURLToPath(new URL(`../target/release/${filename}`, import.meta.url)), count = '200'] = process.argv.slice(2);
const steps = Number(count);
if (!Number.isSafeInteger(steps) || steps < 1) {
  throw new Error('Usage: node examples/bot.mjs [binary] [positive step count]');
}

function action(observation) {
  if (observation === null) return null;
  const { x, y } = observation.self.head;
  let heading = Math.atan2(-y, -x);
  if (Math.hypot(x, y) < observation.arena.radius * 0.8) {
    let distance = Infinity;
    for (const food of observation.food) {
      const candidate = (food.x - x) ** 2 + (food.y - y) ** 2;
      if (candidate < distance) {
        distance = candidate;
        heading = Math.atan2(food.y - y, food.x - x);
      }
    }
  }
  return { heading, boost: false };
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
  if ('error' in reply) throw new Error(reply.error);
  return reply.result;
}

try {
  let result = await request({ op: 'reset', seed: 7, agents: 4 });
  let completed = 0;
  while (completed < steps && result.observations.some(item => item !== null)) {
    result = await request({ op: 'step', actions: result.observations.map(action) });
    completed += 1;
  }
  const living = result.observations.filter(item => item !== null).length;
  console.log(`decisions=${completed} time=${result.time.toFixed(3)}s living=${living}`);
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
