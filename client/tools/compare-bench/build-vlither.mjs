// Build adapter only. Requires explicitly supplied user-space tool locations.
// Does not install SDKs, change PATH globally, or alter upstream source files.
import fs from 'node:fs/promises';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';

const here = path.dirname(fileURLToPath(import.meta.url));
const c = JSON.parse(await fs.readFile(process.argv[2], 'utf8'));
const checkout = path.join(path.resolve(c.output), 'vlither');
const t = c.vlitherTools;
if (!t?.premake || !t?.make || !t?.gccDirectory || !t?.slangc || !t?.vulkanHeaders || !t?.vulkanImportLibrary) throw Error('Need vlitherTools: premake, make, gccDirectory, slangc, vulkanHeaders, vulkanImportLibrary');
const env = { ...process.env, VULKAN_SDK: path.resolve(t.vulkanHeaders).replaceAll('\\', '/'), PATH: t.gccDirectory + path.delimiter + path.dirname(t.make) + path.delimiter + process.env.PATH };
const log = [];
function run(command, args) {
  log.push({ command, args });
  try { const out = execFileSync(command, args, { cwd: checkout, env, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 }); log.push(out); }
  catch (e) { log.push(String(e.stdout), String(e.stderr)); throw e; }
}
try {
  await fs.mkdir(path.join(t.vulkanHeaders, 'lib'), { recursive: true });
  // GCC's -static search needs the .a name, even though Vulkan loads via DLL.
  await fs.copyFile(t.vulkanImportLibrary, path.join(t.vulkanHeaders, 'lib/libvulkan-1.a'));
  run(t.premake, ['--file=project.lua', 'gmake']);
  run(t.make, ['-C', 'build/makefiles', 'config=release_windows', '-j2']);
  const shaders = path.join(checkout, 'app/res/shaders'); await fs.mkdir(path.join(shaders, 'bin'), { recursive: true });
  for (const name of (await fs.readdir(path.join(shaders, 'src'))).filter(f => f.endsWith('.slang'))) {
    for (const [stage, suffix, entry] of [['vertex', 'v', 'vs_main'], ['fragment', 'f', 'fs_main']]) {
      run(t.slangc, [path.join(shaders, 'src', name), '-profile', 'spirv_1_0', '-target', 'spirv', '-capability', 'GLSL_330', '-o', path.join(shaders, 'bin', name.replace('.slang', suffix + '.spv')), '-stage', stage, '-entry', entry, '-emit-spirv-via-glsl']);
    }
  }
  const wrapper = path.join(checkout, 'build/compare-wrapper.c');
  await fs.copyFile(path.join(here, 'vlither-wrapper.c'), wrapper);
  const project = await fs.readFile(path.join(checkout, 'project.lua'), 'utf8');
  const appVersion = project.match(/APP_VERSION=\\"([^"\\]+)\\"/), settingsVersion = project.match(/SETTINGS_VERSION=\\"([^"\\]+)\\"/);
  if (!appVersion || !settingsVersion) throw Error('Vlither project version declaration changed; update build adapter after inspection');
  // Replace only the compiled main object. Upstream sources remain untouched.
  run(path.join(t.gccDirectory, 'gcc.exe'), ['-c', wrapper, '-o', 'build/bin-int/windows_x86_64_release/app/main.o', '-O3', '-m64',
    '-I.', '-Iglfw/include', '-Ithermite/include', '-Ithermite/src', `-I${path.join(t.vulkanHeaders, 'include')}`,
    '-DGLFW_INCLUDE_NONE', '-DGLFW_INCLUDE_VULKAN', '-DCGLM_FORCE_DEPTH_ZERO_TO_ONE', '-DCIMGUI_NO_EXPORT', `-DAPP_VERSION="${appVersion[1]}"`, `-DSETTINGS_VERSION="${settingsVersion[1]}"`]);
  run(t.make, ['-C', 'build/makefiles', 'config=release_windows', '-j2']);
  const binary = path.join(checkout, 'build/bin/windows_x86_64_release/app.exe');
  const sha = async p => createHash('sha256').update(await fs.readFile(p)).digest('hex');
  await fs.writeFile(path.join(checkout, 'build/compare-build.json'), JSON.stringify({ instrumented: true,
    commit: execFileSync('git', ['rev-parse', 'HEAD'], { cwd: checkout, encoding: 'utf8' }).trim(),
    binary, binarySha256: await sha(binary), wrapperSha256: await sha(wrapper),
    declarations: 'automatic loopback join, vsync true, stock wheel input, 0.5s view census; renderer unchanged',
    tools: t, log }, null, 2));
  console.log(binary);
} finally { await fs.writeFile(path.join(checkout, 'build/compare-build-log.json'), JSON.stringify(log, null, 2)); }
