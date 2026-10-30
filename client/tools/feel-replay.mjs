import { pathToFileURL } from 'node:url';
import { readSchedule, readTrace, stringify } from './feel-replay/io.mjs';
import { compareTraces } from './feel-replay/compare.mjs';

export async function main(args) {
  if (args.length < 3 || args.length > 4) throw new Error('Usage: node tools/feel-replay.mjs SCHEDULE LEFT RIGHT [same-core|cross-target]');
  const [scheduleFile, leftFile, rightFile, mode = 'same-core'] = args;
  const schedule = await readSchedule(scheduleFile);
  const left = await readTrace(leftFile, schedule);
  const right = await readTrace(rightFile, schedule);
  const report = compareTraces(left, right, schedule, { mode });
  // Reports also preserve float bits (including signed-zero deltas); no row cap.
  process.stdout.write(stringify(report));
  return report.passed ? 0 : 1;
}
if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  try { process.exitCode = await main(process.argv.slice(2)); }
  catch (error) { process.stderr.write(`${error.message}\n`); process.exitCode = 1; }
}
