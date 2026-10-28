// The dedicated bridge namespace is the attribution boundary. PID details are
// optional diagnostics; a closing socket need not retain an open process fd.
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

function address(hex) {
  if (!/^[0-9A-Fa-f]+$/.test(hex) || ![8, 32].includes(hex.length)) throw Error('invalid proc address');
  const bytes = Buffer.from(hex, 'hex');
  if (os.endianness() === 'LE') for (let i = 0; i < bytes.length; i += 4) bytes.subarray(i, i + 4).reverse();
  if (bytes.length === 4) return [...bytes].join('.');
  if (bytes.subarray(0, 10).every(b => b === 0) && bytes[10] === 255 && bytes[11] === 255)
    return [...bytes.subarray(12)].join('.');
  return null;
}
export function parseProc(text, endpoint) {
  const records = [];
  for (const line of text.split('\n').slice(1).filter(s => s.trim())) {
    const fields = line.trim().split(/\s+/);
    if (fields.length < 10) throw Error('malformed proc TCP census');
    const [hex, port] = fields[2].split(':');
    if (!port || !/^[0-9A-Fa-f]{4}$/.test(port)) throw Error('invalid proc port');
    const peer = address(hex);
    if (peer !== endpoint.ip || parseInt(port, 16) !== endpoint.port) continue;
    if (['06', '07', '0A'].includes(fields[3])) continue;
    records.push({ state: fields[3], local: fields[1], peer: fields[2], inode: fields[9], attribution: 'dedicated-container-namespace', owners: [] });
  }
  return records;
}
export function namespaceCensus(endpoint, proc = '/proc') {
  // Failure of either required census is fatal. Optional identity races are not.
  const connections = ['tcp', 'tcp6'].flatMap(file => parseProc(fs.readFileSync(path.join(proc, 'net', file), 'utf8'), endpoint));
  let pids = [];
  try { pids = fs.readdirSync(proc).filter(s => /^\d+$/.test(s)); } catch { /* optional identities */ }
  for (const pid of pids) {
    let descriptors;
    try { descriptors = fs.readdirSync(path.join(proc, pid, 'fd')); } catch { continue; }
    for (const fd of descriptors) {
      let link;
      try { link = fs.readlinkSync(path.join(proc, pid, 'fd', fd)); } catch { continue; }
      const inode = /^socket:\[(\d+)\]$/.exec(link)?.[1];
      if (!inode) continue;
      for (const connection of connections.filter(c => c.inode === inode && !c.owners.some(o => o.pid === pid))) {
        let argv = [];
        try { argv = fs.readFileSync(path.join(proc, pid, 'cmdline'), 'utf8').split('\0').filter(Boolean); } catch { /* optional */ }
        connection.owners.push({ pid, argv, commandLine: argv.join(' ') });
      }
    }
  }
  return { connections };
}
