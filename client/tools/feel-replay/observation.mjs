// Flat Rust ABI: no JSON snapshot, draw preparation, rounding or interpolation.
export function observation([phase, token, cells], geometry) {
  const v = Array.from(cells); let i = 0;
  const n = () => { if (i >= v.length) throw new Error('Truncated core observation'); return v[i++]; };
  const point = () => ({ x: n(), y: n() });
  const cssPoint = p => ({ x: p.x * geometry.css.width / geometry.backing.width,
    y: p.y * geometry.css.height / geometry.backing.height });
  const delta = { raw: n(), lagged: n(), steps: n(), doubledSteps: n() }, multiplier = n();
  const camera = { ...point(), offset: point(), follow: !!n(), zoom: n(), targetZoom: 0 };
  camera.targetZoom = camera.zoom;
  let own = null;
  if (token !== null) {
    own = { token, expired: !!n(), deathProgress: n(), spawnProgress: n(), opacity: n(), rawHead: point(), correctedHead: point(), rawHeading: n(), correctedHeading: n(),
      wantedHeading: n(), gaze: n(), pupils: { left: point() } };
    // Both eyes share the retained eased pupil vector; their projected centers differ.
    own.pupils.right = { ...own.pupils.left };
    own.offsets = { x: n(), y: n(), fraction: n(), heading: n() };
    Object.assign(own, { speed: n(), visualSpeed: n(), boost: !!n(), boostPhase: n(),
      size: { segments: n(), fraction: n(), length: n() } });
    camera.targetZoom = n();
    const count = n();
    if (!Number.isSafeInteger(count) || count < 0 || count > 8193 || count > (v.length - i) / 2) throw new Error('Invalid body count');
    own.body = Array.from({ length: count }, () => cssPoint(point()));
    own.projected = null;
    if (!own.expired) {
      const backing = { head: point(), pupils: [point(), point()] };
      own.projected = { backing, css: { head: cssPoint(backing.head), pupils: backing.pupils.map(cssPoint) } };
    }
  }
  if (i !== v.length) throw new Error('Unknown core observation cells');
  return { phase, delta, multiplier, own, camera };
}
