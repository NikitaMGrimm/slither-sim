import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, open, rm, symlink, writeFile } from 'node:fs/promises';
import { createWriteStream } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Readable } from 'node:stream';
import { pipeline } from 'node:stream/promises';
import { createGzip, gzipSync } from 'node:zlib';
import { InputHasher, loadTracePayload, MAX_PAYLOAD_BYTES } from './gpu-trace-hashes.mjs';
const id = pointer => ({ PointerId: [pointer] });
function actions({ pointer = 1, byte = 1, size = 512, reverse = false, path = 'first' } = {}) {
  const pass = label => ({ RunRenderPass: { pass: { label, commands: [
    { SetVertexBuffer: { slot: 0, buffer: id(pointer), offset: 0, size: null } },
    { Draw: { vertex_count: 1, instance_count: 1, first_vertex: 0, first_instance: 0 } },
  ], immediates_data: [] }, color_attachments: [] } });
  return [
    { Init: { desc: { trace: { Directory: path }, required_features: 'TIMESTAMP_QUERY' }, backend: 'Dx12' } },
    { CreateBuffer: [id(pointer), { label: 'ordered vertex chunk', size, usage: 'VERTEX | COPY_DST', mapped_at_creation: false }] },
    { Submit: [3, [{ InsertDebugMarker: 'frame-perf:control:begin' }]] },
    { WriteBuffer: { id: id(pointer), data: { Binary: ['bin', [byte, ...Array(19).fill(0)]] }, offset: 0, size: 20, queued: true } },
    { Submit: [4, reverse ? [pass('overlay'), pass('world')] : [pass('world'), pass('overlay')]] },
    { Submit: [5, [{ InsertDebugMarker: 'frame-perf:control:end' }]] },
  ];
}
async function collect(input) {
  const rows = [];
  const hasher = new InputHasher(async () => { throw Error('missing payload'); }, row => rows.push(row));
  for (const action of input) await hasher.action(action);
  hasher.finish(); return rows[0];
}
test('actual bytes/descriptors/order are hashed while opaque pointers and capture paths normalize', async () => {
  const stock = await collect(actions());
  const same = await collect(actions({ pointer: 739, path: 'second' }));
  assert.equal(stock.combined_sha256, same.combined_sha256);
  assert.equal(stock.buffer_upload_bytes, 20);
  assert.equal(stock.texture_upload_bytes, 0);
  assert.equal(stock.passes.length, 2);
  assert.notEqual(stock.uploads_sha256, (await collect(actions({ byte: 2 }))).uploads_sha256);
  assert.notEqual(stock.descriptor_sha256, (await collect(actions({ size: 1024 }))).descriptor_sha256);
  assert.notEqual(stock.commands_sha256, (await collect(actions({ reverse: true }))).commands_sha256);
});
test('immutable dependency fingerprints survive a shader handle drop', async () => {
  const input = actions();
  input.splice(1, 0,
    { CreateShaderModule: { id: id(50), desc: { label: 'shader' }, data: { String: ['wgsl', '@vertex fn main() {}'] } } },
    { CreateGeneralRenderPipeline: { id: id(51), desc: { vertex: { module: id(50) } } } },
    { DropShaderModule: id(50) });
  input[7].Submit[1][0].RunRenderPass.pass.commands.unshift({ SetPipeline: id(51) });
  assert.equal((await collect(input)).passes.length, 2);
});
test('missing bytes, failed GPU commands and unsupported partial writes fail closed', async () => {
  const missing = actions(); missing[3].WriteBuffer.data = { File: 'data1.bin' };
  await assert.rejects(collect(missing), /missing payload/);
  const partial = actions(); partial[3].WriteBuffer.offset = 4;
  await assert.rejects(collect(partial), /Partial buffer write/);
  await assert.rejects(collect([{ FailedCommands: { error: 'validation' } }]), /FailedCommands/);
});

test('retained CPU inputs uploaded before the frame remain part of its fingerprint', async () => {
  const input = byte => {
    const stream = actions();
    stream.splice(2, 0,
      { CreateBuffer: [id(400), { label: 'immutable uniform', size: 4, usage: 'UNIFORM | COPY_DST', mapped_at_creation: false }] },
      { WriteBuffer: { id: id(400), data: { Binary: ['bin', [byte, 0, 0, 0]] }, offset: 0, size: 4, queued: true } });
    return stream;
  };
  const a = await collect(input(1)), b = await collect(input(2));
  assert.equal(a.uploads_sha256, b.uploads_sha256);
  assert.notEqual(a.descriptor_sha256, b.descriptor_sha256);
});

test('queued commands retain a descriptor after its public handle drops', async () => {
  const input = actions();
  input.splice(4, 0, { DropBuffer: id(1) });
  assert.equal((await collect(input)).passes.length, 2);
});

test('opaque address reuse creates a new resource identity within the frame', async () => {
  const input = pointer => {
    const stream = actions();
    const commands = structuredClone(stream[4].Submit[1]);
    for (const command of commands) command.RunRenderPass.pass.commands[0].SetVertexBuffer.buffer = id(pointer);
    stream.splice(5, 0,
      { DropBuffer: id(1) },
      { CreateBuffer: [id(pointer), { label: 'ordered vertex chunk', size: 512, usage: 'VERTEX | COPY_DST', mapped_at_creation: false }] },
      { WriteBuffer: { id: id(pointer), data: { Binary: ['bin', Array(20).fill(2)] }, offset: 0, size: 20, queued: true } },
      { Submit: [9, commands] });
    return stream;
  };
  assert.equal((await collect(input(1))).combined_sha256, (await collect(input(600))).combined_sha256);
});

async function traceFixture(t) {
  const directory = await mkdtemp(join(tmpdir(), 'gpu-trace-hash-'));
  t.after(() => rm(directory, { recursive: true, force: true }));
  const root = join(directory, 'trace');
  await mkdir(root);
  return { root, directory };
}

test('oversized raw and compressed files fail the size check before reading', async t => {
  const { root } = await traceFixture(t);
  for (const name of ['raw.bin', 'compressed.bin.gz']) {
    const file = await open(join(root, name), 'wx');
    try { await file.truncate(MAX_PAYLOAD_BYTES + 1); } finally { await file.close(); }
  }
  await assert.rejects(loadTracePayload(root, 'raw.bin'), /GPU payload exceeds diagnostic bound/);
  await assert.rejects(loadTracePayload(root, 'compressed.bin'), /GPU payload exceeds diagnostic bound/);
  await writeFile(join(root, 'ok.bin.gz'), gzipSync(Buffer.from('authored payload')));
  assert.deepEqual(await loadTracePayload(root, 'ok.bin'), Buffer.from('authored payload'));
});

test('decompression enforces the output bound and retains the ENOENT-only fallback', async t => {
  const { root } = await traceFixture(t);
  const block = Buffer.alloc(1024 * 1024);
  async function* oversized() {
    for (let size = 0; size < MAX_PAYLOAD_BYTES; size += block.length) yield block;
    yield Buffer.from([0]);
  }
  await pipeline(Readable.from(oversized()), createGzip(), createWriteStream(join(root, 'large.bin.gz')));
  await assert.rejects(loadTracePayload(root, 'large.bin'), /GPU payload exceeds diagnostic bound/);
  await writeFile(join(root, 'broken.bin.gz'), Buffer.from('invalid gzip'));
  await assert.rejects(loadTracePayload(root, 'broken.bin'), { code: 'Z_DATA_ERROR' });
});

test('real paths confine raw and gzip symlinks to the trace root', async t => {
  const { root, directory } = await traceFixture(t);
  const outside = join(directory, 'outside.bin');
  await writeFile(outside, Buffer.from('outside trace'));
  try { await symlink(outside, join(root, 'escape.bin'), 'file'); } catch (error) {
    if (error.code === 'EPERM' || error.code === 'ENOSYS') {
      t.skip('File symlinks are unavailable to this account');
      return;
    }
    throw error;
  }
  await assert.rejects(loadTracePayload(root, 'escape.bin'), /outside trace directory/);
  await symlink(outside, join(root, 'escape-gzip.bin.gz'), 'file');
  await assert.rejects(loadTracePayload(root, 'escape-gzip.bin'), /outside trace directory/);
  await writeFile(join(root, 'inside.bin'), Buffer.from('inside trace'));
  await symlink(join(root, 'inside.bin'), join(root, 'inside-link.bin'), 'file');
  assert.deepEqual(await loadTracePayload(root, 'inside-link.bin'), Buffer.from('inside trace'));
  await assert.rejects(loadTracePayload(root, '../outside.bin'), /outside trace directory/);
});
