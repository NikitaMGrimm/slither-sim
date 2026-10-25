// A single bounded handshake diagnostic on the same frozen official endpoint.
import fs from 'node:fs';
import dc from 'node:diagnostics_channel';
import { record, parseArgs, runCollectorCli } from '../../../tools/fidelity/slither-headless.mjs';
for (const name of ['undici:request:headers', 'undici:request:error', 'undici:client:connectError']) {
  dc.channel(name).subscribe(value => console.log(JSON.stringify({ channel: name,
    statusCode: value.response?.statusCode, error: value.error ? String(value.error) : null,
    code: value.error?.code, headers: value.response?.headers?.map(x => String(x)) })));
}
runCollectorCli(async signal => {
  const [plan, out] = process.argv.slice(2);
  if (fs.existsSync(out)) throw new Error('existing probe output');
  fs.mkdirSync(out);
  const opts = parseArgs(['--out', out, '--server', '57.129.37.42:444', '--origin', 'http://slither.io',
    '--bots', '1', '--seconds', '10', '--scenario', 'forage', '--plan', plan]);
  return record(opts, { ip: '57.129.37.42', port: 444 }, 'probe', 1, signal);
});
