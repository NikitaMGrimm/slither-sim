import { readdir, readFile, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { sha256, stringify } from './io.mjs';

// Contract: browser-peers.md and README.md; authored browser-synthetic.json.
export function assertBuildBinding(built, product) {
  if (built.tree !== product.tree) throw new Error('Replay artifacts are stale; stage the candidate and rebuild explicitly');
}

async function artifactHashes(directory) {
  const hashes = [];
  async function visit(relative) {
    for (const entry of (await readdir(path.join(directory, relative), { withFileTypes: true })).sort((a, b) => a.name.localeCompare(b.name))) {
      const name = `${relative}/${entry.name}`;
      if (entry.isDirectory()) await visit(name);
      else if (entry.isFile()) hashes.push({ name, sha256: sha256(await readFile(path.join(directory, name))) });
      else throw new Error('Regular generated artifacts required');
    }
  }
  await visit('canvas'); await visit('gpu');
  if (!hashes.length) throw new Error('Generated artifacts required');
  return hashes;
}

export async function rebuildArtifacts(directory, build) {
  // Invalidate BEFORE source/tool checks or any artifact mutation. An interrupted
  // rebuild can never authenticate its partial output with the preceding receipt.
  const receipt = path.join(directory, 'build.json');
  await rm(receipt, { force: true });
  const fields = await build();
  const artifacts = await artifactHashes(directory);
  await writeFile(receipt, stringify({ ...fields, artifacts }), { flag: 'wx' });
}

export async function verifyArtifacts(directory, built, product) {
  assertBuildBinding(built, product);
  if (!Array.isArray(built.artifacts) || !built.artifacts.length
    || stringify(built.artifacts) !== stringify(await artifactHashes(directory))) throw new Error('Generated artifact authentication failed; rebuild explicitly');
}
