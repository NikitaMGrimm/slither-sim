// Explicit opt-in build. The replay command itself never builds or discovers peers.
import { execFileSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { assertBuildBinding, rebuildArtifacts } from './feel-replay/artifacts.mjs';
process.chdir(fileURLToPath(new URL('..', import.meta.url)));
const env = { ...process.env, CARGO_BUILD_JOBS: '2', CARGO_TARGET_DIR: path.resolve('target/feel-replay') };
const bindgen = process.env.WASM_BINDGEN || 'wasm-bindgen';
const git = (...args) => execFileSync('git', args, { encoding: 'utf8' }).trim();
const source = () => {
  git('diff', '--exit-code', '--', '.');
  if (git('ls-files', '--others', '--exclude-standard')) throw new Error('Stage replay source files before building');
  return { tree: git('write-tree'), core: git('write-tree', '--prefix=client/src/') };
};
await rebuildArtifacts('dist/feel-replay', async () => {
  const built = source();
  if (execFileSync(bindgen, ['--version'], { encoding: 'utf8' }).trim() !== 'wasm-bindgen 0.2.129') throw new Error('wasm-bindgen 0.2.129 required');
  // Only wgpu builds without debug info, as its pre-workspace profile did.
  for (const [name, manifest, dir, debug] of [['slither_client', 'Cargo.toml', 'canvas', {}], ['slither_wgpu_slice', 'apps/wgpu-slice/Cargo.toml', 'gpu', { CARGO_PROFILE_DEV_DEBUG: '0' }]]) {
    execFileSync('cargo', ['build', '--locked', '--lib', '--features', 'feel-trace', '--target', 'wasm32-unknown-unknown', '--manifest-path', manifest], { env: { ...env, ...debug }, stdio: 'inherit' });
    execFileSync(bindgen, [path.join(env.CARGO_TARGET_DIR, `wasm32-unknown-unknown/debug/${name}.wasm`),
      '--target', 'web', '--out-dir', `dist/feel-replay/${dir}`, '--out-name', name, '--no-typescript'], { stdio: 'inherit' });
  }
  assertBuildBinding(built, source());
  return { ...built, rustc: execFileSync('rustc', ['--version'], { encoding: 'utf8' }).trim(), bindgen: '0.2.129' };
});
