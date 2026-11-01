import { execFileSync } from 'node:child_process';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

process.chdir(fileURLToPath(new URL('..', import.meta.url)));
function run(command, args, env = process.env) {
  console.log(`\n> ${command} ${args.join(' ')}`);
  execFileSync(command, args, { stdio: 'inherit', env });
}
const node = (...args) => run(process.execPath, args);
const cargo = (...args) => run('cargo', args);
const manifest = ['--manifest-path', 'apps/wgpu-slice/Cargo.toml'];
// Root workspace profiles keep debug info; the wgpu builds skip it to limit time and memory.
const gpuEnv = { ...process.env, CARGO_BUILD_JOBS: process.env.CARGO_BUILD_JOBS || '1', RUST_TEST_THREADS: '1', CARGO_PROFILE_DEV_DEBUG: '0',
  CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR || path.resolve('target/wgpu-slice-check') };
const gpu = (...args) => run('cargo', [args[0], ...manifest, ...args.slice(1)], gpuEnv);

try {
  node('tools/check-repository.mjs');
  node('tools/check-repository.mjs', '--history');
  node('--test', 'tools/*.test.mjs');
  // Same app tests and concurrency as CI; several start loopback servers.
  node('--test', '--test-concurrency=1', 'apps/wgpu-slice/tests/*.test.mjs');
  cargo('fmt', '-p', 'slither-client', '--check');
  cargo('test', '--all-targets', '--locked', '--features', 'latency-rig');
  cargo('test', '--doc', '--locked');
  cargo('clippy', '--all-targets', '--locked', '--features', 'latency-rig', '--', '-D', 'warnings');
  node('tools/build-web.mjs');
  node('tools/wasm-smoke.mjs');
  gpu('fmt', '--check');
  if (process.platform === 'win32') {
    gpu('test', '--locked', '--features', 'latency-rig', '--lib', '--tests', '--bin', 'slither-client-latency');
    gpu('clippy', '--locked', '--features', 'latency-rig', '--all-targets', '--', '-D', 'warnings');
    gpu('test', '--locked', '--features', 'frame-profile', '--test', 'render_profile', '--test', 'body_shadow_scale', '--test', 'frame_perf_output');
    gpu('clippy', '--locked', '--features', 'frame-profile', '--all-targets', '--', '-D', 'warnings');
    gpu('build', '--locked', '--bin', 'slither-wgpu-desktop');
  } else console.log('Windows desktop checks run on Windows CI; checking shared Wasm here.');
  gpu('clippy', '--locked', '--lib', '--target', 'wasm32-unknown-unknown', '--', '-D', 'warnings');
  gpu('build', '--locked', '--lib', '--target', 'wasm32-unknown-unknown');
  run(process.env.WASM_BINDGEN || 'wasm-bindgen', ['--target', 'web', '--out-dir', 'dist/wgpu-slice/pkg',
    '--out-name', 'slither_wgpu_slice', path.join(gpuEnv.CARGO_TARGET_DIR, 'wasm32-unknown-unknown/debug/slither_wgpu_slice.wasm')]);
  console.log('\nAll checks passed.');
} catch (error) { console.error(error.message); process.exitCode = 1; }
