// Explicit build only. Replays never compile or connect to anything.
import { execFileSync } from 'node:child_process';
import { mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { sha256, stringify } from './feel-replay/io.mjs';
process.chdir(fileURLToPath(new URL('..', import.meta.url)));
const directory = path.resolve('dist/feel-native');
await mkdir(directory, { recursive: true });
await rm(path.join(directory, 'build.json'), { force: true });
const git = (...args) => execFileSync('git', args, { encoding: 'utf8' }).trim();
const source = () => {
  if (git('diff', '--name-only') || git('ls-files', '--others', '--exclude-standard')) throw new Error('Stage intended source before building');
  return { tree: git('write-tree'), core: git('write-tree', '--prefix=client/src/') };
};
const built = source();
const env = { ...process.env, CARGO_BUILD_JOBS: '2', CARGO_PROFILE_DEV_DEBUG: '0', CARGO_TARGET_DIR: path.resolve('apps/wgpu-slice/target') };
execFileSync('cargo', ['build', '--locked', '--features', 'feel-trace', '--manifest-path', 'apps/wgpu-slice/Cargo.toml', '--example', 'feel-replay'], { env, stdio: 'inherit' });
if (stringify(source()) !== stringify(built)) throw new Error('Source changed during build');
const executable = path.join(env.CARGO_TARGET_DIR, 'debug/examples/feel-replay' + (process.platform === 'win32' ? '.exe' : ''));
await writeFile(path.join(directory, 'build.json'), stringify({ ...built, executable, sha256: sha256(await readFile(executable)),
  rustc: execFileSync('rustc', ['--version'], { encoding: 'utf8' }).trim() }), { flag: 'wx' });
