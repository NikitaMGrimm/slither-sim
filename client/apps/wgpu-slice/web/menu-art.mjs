// Local resource policy only. Rust validates the same bounded manifest at decode.
export const MANIFEST_LIMIT = 4096;
export const DEFAULT_MENU_PATHS = ['menu/logo.png', 'menu/skin.png', 'menu/builder.png', 'menu/cosmetic.png', 'menu/quality-high.png', 'menu/quality-low.png'];
// Same public resource identifiers as Cosmetics::paths(); the real-Wasm login
// ABI check compares this inventory with Rust's, including every accessory.
export const COSMETIC_PATHS = [
  's/jmou3.png','s2/sest5.png',
  ...['kwk6','pewd','play','leaf','swiss','moldova','vietnam','argentina','mov2','bonkers2'].map(name=>`s/${name}.png`),
  ...'oakley graduation funkystar headphones eyebrows spikecollar disguise cape crown antlers unicorn angel bat dragon bear rabbit cat dreadlocks blonde ginger blackhair mohawk catglass swirly nerdglass 3dglass heartglass monocle deerstalker visor cap hardhat'.split(' ').map(name=>`s/a_${name}.png`),
];
export function menuPaths(bytes) {
  return menuEntries(bytes).map(entry => entry.path);
}
export function menuEntries(bytes, reservedPaths = COSMETIC_PATHS) {
  if (bytes.byteLength > MANIFEST_LIMIT) throw new Error('Menu manifest byte budget');
  const [version, ...lines] = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true }).decode(bytes).split(/\r?\n/);
  if (version !== 'menu-art-v1') throw new Error('Menu manifest version');
  const roles = new Set(), paths = new Set(), entries = [], reserved = new Set(reservedPaths);
  for (const line of lines.filter(Boolean)) {
    const fields = line.split(/[\t\f\r ]+/).filter(Boolean);
    if (fields.length !== 3 || roles.size === 37) throw new Error('Menu manifest fields/budget');
    const [role, revision, path] = fields;
    if(reserved.has(path))throw new Error('Menu manifest path collides with cosmetic');
    if (!/^(skin|builder|cosmetic|quality-high|quality-low|logo:(?:[0-9]|[12][0-9]|3[01]))$/.test(role)
      || !/^[0-9]+$/.test(revision) || Number(revision) < 1 || Number(revision) > 0xffff_ffff
      || path.length > 128 || !/^[a-zA-Z0-9_./-]+\.(png|jpg)$/.test(path)
      || path.split('/').some(p => !p || p === '.' || p === '..') || roles.has(role) || paths.has(path)) {
      throw new Error('Menu manifest invalid entry');
    }
    roles.add(role); paths.add(path);
    entries.push({ role, path });
  }
  return entries;
}
