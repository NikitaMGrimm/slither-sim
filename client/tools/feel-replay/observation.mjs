// Flat Rust ABI: no JSON snapshot, draw preparation, rounding or interpolation.
export function observation([phase, token, cells], geometry) {
  const v = Array.from(cells); let i = 0;
  const n = () => { if (i >= v.length) throw new Error('Truncated core observation'); return v[i++]; };
  const point = () => ({ x: n(), y: n() });
  const list = count => Array.from({ length: count }, n);
  const delta = { raw: n(), lagged: n(), steps: n(), doubledSteps: n() }, multiplier = n();
  const camera = { ...point(), offset: point(), index: n(), pending: n(), follow: !!n(), zoom: n(), ring: list(124), targetZoom: 0 };
  camera.targetZoom = camera.zoom;
  let own = null;
  if (token !== null) {
    own = { token, expired: !!n(), deathProgress: n(), rawHead: point(), correctedHead: point(), rawHeading: n(), correctedHeading: n(),
      wantedHeading: n(), gaze: n(), pupils: { left: point() } };
    // Both eyes share the retained eased pupil vector; their projected centers differ.
    own.pupils.right = { ...own.pupils.left };
    own.offsets = { x: n(), y: n(), fraction: n(), heading: n() };
    Object.assign(own, { speed: n(), visualSpeed: n(), boost: !!n(), boostPhase: n(),
      size: { segments: n(), fraction: n(), length: n() } });
    own.rings = { head: { index: n(), pending: n() }, angle: { index: n(), pending: n() }, length: { index: n(), pending: n() } };
    camera.targetZoom = n();
    for (const [name, count] of [['head', 159], ['angle', 26], ['length', 53]]) own.rings[name].values = list(count);
    own.projected = null;
    if (!own.expired) {
      const backing = { head: point(), pupils: [point(), point()] };
      const cssPoint = p => ({ x: p.x * geometry.css.width / geometry.backing.width,
        y: p.y * geometry.css.height / geometry.backing.height });
      own.projected = { backing, css: { head: cssPoint(backing.head), pupils: backing.pupils.map(cssPoint) } };
    }
  }
  if (i !== v.length) throw new Error('Unknown core observation cells');
  return { phase, delta, multiplier, own, camera };
}
