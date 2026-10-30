// Local resource policy only. Rust validates the same bounded manifest at decode.
export const MANIFEST_LIMIT = 4096;
export const DEFAULT_MENU_PATHS = ['menu/logo.png', 'menu/skin.png', 'menu/builder.png', 'menu/cosmetic.png'];
export function menuPaths(bytes) {
  if (bytes.byteLength > MANIFEST_LIMIT) throw new Error('Menu manifest byte budget');
  const [version, ...lines] = new TextDecoder('utf-8', { fatal: true }).decode(bytes).split(/\r?\n/);
  if (version !== 'menu-art-v1') throw new Error('Menu manifest version');
  const roles = new Set(), paths = new Set();
  for (const line of lines.filter(Boolean)) {
    const fields = line.trim().split(/[\t\n\r ]+/);
    if (fields.length !== 3 || roles.size === 35) throw new Error('Menu manifest fields/budget');
    const [role, revision, path] = fields;
    if (!/^(skin|builder|cosmetic|logo:(?:[0-9]|[12][0-9]|3[01]))$/.test(role)
      || !/^[0-9]+$/.test(revision) || Number(revision) < 1 || Number(revision) > 0xffff_ffff
      || path.length > 128 || !/^[a-zA-Z0-9_./-]+\.(png|jpg)$/.test(path)
      || path.split('/').some(p => !p || p === '.' || p === '..') || roles.has(role) || paths.has(path)) {
      throw new Error('Menu manifest invalid entry');
    }
    roles.add(role); paths.add(path);
  }
  return [...paths];
}
