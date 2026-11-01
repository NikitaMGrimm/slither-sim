// Offline hashing of typed wgpu-30 API actions, never a reconstruction of Draws.
import { createHash } from 'node:crypto';
import { open, realpath, writeFile } from 'node:fs/promises';
import { gunzipSync } from 'node:zlib';
import { resolve, relative, isAbsolute, basename, sep } from 'node:path';
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { pathToFileURL } from 'node:url';

const digest = bytes => createHash('sha256').update(bytes).digest('hex');
export const MAX_PAYLOAD_BYTES = 128 * 1024 * 1024;
const payloadBound = () => Error('GPU payload exceeds diagnostic bound');

async function confinedFile(root, name) {
  if (basename(name) !== name) throw Error('Payload outside trace directory');
  const path = await realpath(resolve(root, name)), rel = relative(root, path);
  if (isAbsolute(rel) || rel === '..' || rel.startsWith(`..${sep}`)) throw Error('Payload outside trace directory');
  return path;
}

async function readBounded(path) {
  const file = await open(path, 'r');
  try {
    if ((await file.stat()).size > MAX_PAYLOAD_BYTES) throw payloadBound();
    const chunks = [];
    let size = 0;
    // The stream bound also holds if a file grows after the initial size check.
    for await (const chunk of file.createReadStream({ autoClose: false })) {
      size += chunk.length;
      if (size > MAX_PAYLOAD_BYTES) throw payloadBound();
      chunks.push(chunk);
    }
    return Buffer.concat(chunks, size);
  } finally { await file.close(); }
}

export async function loadTracePayload(trace, name) {
  const root = await realpath(resolve(trace));
  try { return await readBounded(await confinedFile(root, name)); } catch (error) {
    if (error.code !== 'ENOENT') throw error;
  }
  const compressed = await readBounded(await confinedFile(root, `${name}.gz`));
  try { return gunzipSync(compressed, { maxOutputLength: MAX_PAYLOAD_BYTES }); } catch (error) {
    if (error.code === 'ERR_BUFFER_TOO_LARGE') throw payloadBound();
    throw error;
  }
}
const stable = value => JSON.stringify(value, (_, item) => item && typeof item === 'object' && !Array.isArray(item)
  ? Object.fromEntries(Object.entries(item).sort(([a], [b]) => a.localeCompare(b))) : item);
const idOf = value => {
  if (!value || Object.keys(value).length !== 1 || !Object.hasOwn(value, 'PointerId')) throw Error('Expected typed PointerId');
  // PointerId is a tuple variant with a skipped PhantomData field in wgpu.
  const pointer = value.PointerId;
  if (!Array.isArray(pointer) || pointer.length !== 1 || !Number.isSafeInteger(pointer[0]) || pointer[0] <= 0) throw Error('Unsafe trace pointer tuple');
  return pointer[0];
};
const isId = value => value && typeof value === 'object' && !Array.isArray(value) && Object.hasOwn(value, 'PointerId');

export class InputHasher {
  constructor(loadData, emit) {
    this.loadData = loadData; this.emit = emit; this.resources = new Map(); this.retired = new Map(); this.nextResource = 0; this.frame = null;
    this.payloads = []; this.frames = 0; this.cache = new Map();
  }
  async data(data) {
    let bytes;
    if (Object.hasOwn(data, 'File')) bytes = await this.loadData(data.File);
    else if (Object.hasOwn(data, 'Binary')) {
      if (data.Binary[1].length > MAX_PAYLOAD_BYTES) throw payloadBound();
      bytes = Buffer.from(data.Binary[1]);
    } else if (Object.hasOwn(data, 'String')) {
      if (Buffer.byteLength(data.String[1], 'utf8') > MAX_PAYLOAD_BYTES) throw payloadBound();
      bytes = Buffer.from(data.String[1], 'utf8');
    }
    else throw Error('Unknown GPU payload representation');
    if (bytes.length > MAX_PAYLOAD_BYTES) throw payloadBound();
    const record = { bytes: bytes.length, sha256: digest(bytes) };
    this.payloads.push({ file: data.File ?? null, ...record });
    return record;
  }
  resource(id) { return this.resources.get(id) ?? this.retired.get(id); }
  descriptor(id, stack = new Set()) {
    const existing = this.resource(id); if (!existing) throw Error(`Unknown resource pointer ${id}`);
    if (existing.descriptorHash) return existing.descriptorHash;
    if (this.cache.has(id)) return this.cache.get(id);
    const r = existing;
    if (stack.has(id)) throw Error('Cyclic resource descriptor');
    const next = new Set(stack).add(id);
    const walk = v => {
      if (isId(v)) {
        const child = this.resource(idOf(v)); if (!child) throw Error('Missing descriptor dependency');
        return { kind: child.kind, descriptor_sha256: this.descriptor(idOf(v), next), cpu_input: child.cpuInput ?? null };
      }
      if (Array.isArray(v)) return v.map(walk);
      if (v && typeof v === 'object') return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, walk(x)]));
      return v;
    };
    const hash = digest(stable({ kind: r.kind, descriptor: walk(r.desc), shader: r.shader ?? null }));
    this.cache.set(id, hash); return hash;
  }
  normalize(value) {
    if (isId(value)) {
      const id = idOf(value), r = this.resource(id); if (!r) throw Error(`Uncreated GPU resource reference ${id} in frame ${this.frame.label}`);
      if (!this.frame.aliases.has(r.token)) {
        const slot = this.frame.kindCounts.get(r.kind) ?? 0;
        this.frame.kindCounts.set(r.kind, slot + 1); this.frame.aliases.set(r.token, slot);
      }
      return { kind: r.kind, slot: this.frame.aliases.get(r.token), descriptor_sha256: this.descriptor(id), cpu_input: r.cpuInput ?? null };
    }
    if (Array.isArray(value)) return value.map(v => this.normalize(v));
    if (value && typeof value === 'object') return Object.fromEntries(Object.entries(value).map(([k, v]) => [k, this.normalize(v)]));
    return value;
  }
  event(kind, value) {
    if (!this.frame) return;
    const normalized = this.normalize(value), text = stable({ kind, value: normalized });
    this.frame.order.update(`${Buffer.byteLength(text)}:`).update(text);
    if (kind === 'command' || kind === 'submit') this.frame.commands.update(`${Buffer.byteLength(text)}:`).update(text);
    this.frame.events++;
  }
  begin(label) {
    if (this.frame) throw Error('Nested trace frame');
    this.cache.clear();
    this.frame = { label, aliases: new Map(), kindCounts: new Map(), order: createHash('sha256'), commands: createHash('sha256'), uploads: [], events: 0, passes: [] };
  }
  end(label) {
    if (!this.frame || this.frame.label !== label) throw Error('Unmatched trace frame marker');
    const frame = this.frame;
    const descriptors = [...this.resources.entries()].map(([id, r]) => ({ kind: r.kind, descriptor_sha256: this.descriptor(id), label: r.desc?.label ?? null, cpu_input: r.cpuInput ?? null,
      ...(r.kind === 'Buffer' ? { size: r.desc.size, usage: r.desc.usage } : {}) }));
    descriptors.sort((a, b) => stable(a).localeCompare(stable(b)));
    const device = structuredClone(this.init ?? {});
    if (device.desc) delete device.desc.trace; // Capture path is host metadata, not a GPU input.
    descriptors.push({ kind: 'Device', descriptor_sha256: digest(stable(device)) });
    const descriptorsHash = digest(stable(descriptors)), uploadsHash = digest(stable(frame.uploads));
    const commandsHash = frame.commands.digest('hex'), orderHash = frame.order.digest('hex');
    const report = { schema: 'gpu-api-inputs-v1', label, descriptor_sha256: descriptorsHash, uploads_sha256: uploadsHash,
      commands_sha256: commandsHash, order_sha256: orderHash,
      combined_sha256: digest(stable([descriptorsHash, uploadsHash, commandsHash, orderHash])),
      descriptors, uploads: frame.uploads, passes: frame.passes, events: frame.events,
      buffer_upload_bytes: frame.uploads.filter(x => x.kind === 'buffer').reduce((n, x) => n + x.bytes, 0),
      texture_upload_bytes: frame.uploads.filter(x => x.kind === 'texture').reduce((n, x) => n + x.bytes, 0) };
    this.emit(report); this.frames++; this.frame = null;
  }
  async action(action) {
    const names = Object.keys(action); if (names.length !== 1) throw Error('Expected one typed API action');
    const kind = names[0], value = action[kind];
    if (kind === 'Init') { this.init = value; return; }
    if (kind === 'FailedCommands' || kind === 'CreateTextureError') throw Error(`GPU trace contains ${kind}`);
    if (kind.startsWith('Create')) {
      let id, desc;
      if (Array.isArray(value)) { id = value[0]; desc = value[1]; }
      else { id = value.id; desc = value.desc; }
      if (!id || !desc) throw Error(`Unsupported creation action ${kind}`);
      if (kind === 'CreateTextureView') desc = { parent: value.parent, ...desc };
      const r = { kind: kind.slice(6), desc, token: this.nextResource++ };
      if (kind === 'CreateShaderModule') r.shader = await this.data(value.data);
      if (kind === 'CreateRenderBundle') r.desc = { ...desc, base: value.base };
      this.resources.set(idOf(id), r); this.retired.delete(idOf(id)); this.cache.clear();
      // Keep immutable dependency fingerprints even after the public shader or
      // layout handle drops, or its opaque address is reused by another object.
      r.descriptorHash = this.descriptor(idOf(id));
      this.event('create', id); return;
    }
    if (kind.startsWith('Get') && kind.endsWith('BindGroupLayout')) {
      this.resources.set(idOf(value.id), { kind: 'BindGroupLayout', desc: { pipeline: value.pipeline, index: value.index }, token: this.nextResource++ });
      this.retired.delete(idOf(value.id));
      this.resources.get(idOf(value.id)).descriptorHash = this.descriptor(idOf(value.id));
      this.cache.clear(); this.event('create', value.id); return;
    }
    if (kind === 'WriteBuffer' || kind === 'WriteTexture') {
      const id = kind === 'WriteBuffer' ? value.id : value.to.texture;
      const r = this.resources.get(idOf(id)); if (!r) throw Error('Write to uncreated resource');
      const data = await this.data(value.data);
      if (kind === 'WriteBuffer' && data.bytes !== value.size) throw Error('Buffer payload length does not match actual write size');
      const details = kind === 'WriteBuffer' ? { offset: value.offset, size: value.size, queued: value.queued } : { to: value.to, layout: value.layout, size: value.size };
      // This renderer writes complete vertex prefixes at offset zero and
      // immutable texture revisions. Reject unsupported state instead of
      // claiming equality from an incomplete reconstruction.
      if (kind === 'WriteBuffer' && value.offset !== 0) throw Error('Partial buffer write requires explicit state composition');
      if (kind === 'WriteTexture' && r.cpuInput) throw Error('Mutable texture requires explicit state composition');
      const textureTarget = kind === 'WriteTexture' ? Object.fromEntries(Object.entries(value.to).filter(([key]) => key !== 'texture')) : null;
      r.cpuInput = { ...data, ...(kind === 'WriteBuffer' ? { offset: value.offset } : { target: textureTarget, layout: value.layout, size: value.size }) };
      this.cache.clear();
      if (this.frame) {
        const upload = { kind: kind === 'WriteBuffer' ? 'buffer' : 'texture', resource: this.normalize(id), ...data, usage: r.desc.usage ?? null, details: this.normalize(details) };
        this.frame.uploads.push(upload); this.event('upload', upload);
      }
      return;
    }
    if (kind === 'Submit') {
      const commands = value[1];
      if (commands.length === 1 && commands[0].InsertDebugMarker?.startsWith('frame-perf:')) {
        const marker = commands[0].InsertDebugMarker;
        if (marker.endsWith(':begin')) this.begin(marker.slice(0, -6));
        else if (marker.endsWith(':end')) this.end(marker.slice(0, -4));
        else throw Error('Unknown frame marker');
        return;
      }
      this.event('submit', 'begin');
      for (const command of commands) {
        const pass = command.RunRenderPass;
        if (pass && this.frame) this.frame.passes.push({ label: pass.pass.label, sha256: digest(stable(this.normalize(command))) });
        this.event('command', command);
      }
      this.event('submit', 'end'); return;
    }
    if (kind.startsWith('Drop')) {
      // A public handle may drop before Submit: encoders keep the core Arc alive
      // (notably TextureBlitter's temporary bind group). Preserve its descriptor
      // for queued commands, while excluding it from the public live inventory.
      const id = idOf(value), resource = this.resources.get(id);
      if (!resource) throw Error('Duplicate or uncreated resource drop');
      this.event('drop', value); this.retired.set(id, resource); this.resources.delete(id); this.cache.clear(); return;
    }
    if (kind.startsWith('Destroy')) { this.event('destroy', value); return; }
    throw Error(`Unsupported API action ${kind}`);
  }
  finish() { if (this.frame) throw Error('Unclosed frame'); if (!this.frames) throw Error('No input frames recorded'); }
}

export async function hashTrace(converter, trace, output) {
  const root = await realpath(resolve(trace)), reports = [];
  const load = name => loadTracePayload(root, name);
  const process = spawn(converter, [await confinedFile(root, 'trace.ron')], { windowsHide: true, stdio: ['ignore', 'pipe', 'inherit'] });
  const completion = new Promise((ok, fail) => { process.once('error', fail); process.once('exit', code => code === 0 ? ok() : fail(Error(`Trace converter exit ${code}`))); });
  completion.catch(() => {});
  const hasher = new InputHasher(load, row => reports.push(row));
  try {
    for await (const line of createInterface({ input: process.stdout, crlfDelay: Infinity })) if (line.trim()) await hasher.action(JSON.parse(line));
    await completion; hasher.finish();
  } catch (error) { process.kill(); throw error; }
  await writeFile(output, reports.map(stable).join('\n') + '\n');
  await writeFile(`${output}.payloads.json`, JSON.stringify(hasher.payloads, null, 2) + '\n');
  return { frames: hasher.frames, payloads: hasher.payloads.length };
}
if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const [converter, trace, output] = process.argv.slice(2);
  if (!converter || !trace || !output) throw Error('usage: gpu-trace-hashes.mjs CONVERTER TRACE_DIRECTORY NEW_OUTPUT_JSONL');
  console.log(JSON.stringify(await hashTrace(converter, trace, output)));
}
