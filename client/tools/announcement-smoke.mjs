// Authored synthetic actual-Wasm announcement witnesses; no network or reference.
export function announcementSmoke({ fixture, setup, spawn, food, prey, plan, expectedScore, eq, ok, rejects }) {
  const f = fixture();
  f.scene.enable_announcement_tracking();
  const packet = (name, message, count = 1) => {
    const bytes = new Uint8Array(8 + name.length + message.length);
    bytes.set([109, count >>> 16 & 255, count >>> 8 & 255, count & 255, 255, 255, 255, name.length]);
    for (let i = 0; i < name.length; i++) bytes[8 + i] = name.charCodeAt(i);
    for (let i = 0; i < message.length; i++) bytes[8 + name.length + i] = message.charCodeAt(i);
    return bytes;
  };
  const first = [setup(1), packet('First', '<tag> &  text'), setup(29)];
  f.hud(first, plan(first));
  let view = f.scene.announcement_view();
  eq(view.lines.map(line => line.text), ['"<tag> &  text"', "- First, today's longest", 'with a length of 10']);
  eq(view.opacity, 0); ok(Object.isFrozen(view) && Object.isFrozen(view.lines) && Object.isFrozen(view.lines[0]));
  f.scene.reset(); eq(f.scene.announcement_view(), view, 'reset retains text/opacity');
  f.hud([setup(1), packet('123456-7', 'x'.repeat(600))]);
  view = f.scene.announcement_view(); eq(view.lines[0].text.length, 602); eq(view.lines[1].text, "- today's longest");
  for (const [name, message, expected, alpha] of [
    ['N', 'M', ['"M"', "- N, today's longest", 'with a length of 10'], [.5, 1, .5, .5, .75, .5, .5, .65]],
    ['', 'M', ['"M"', "- today's longest", 'with a length of 10'], [.5, 1, .5, .5, .5, .65]],
    ['N', '', ["Today's longest was N", 'with a length of 10'], [.5, .75, .5, .65]],
    ['', '', ["Today's longest: 10"], [.5, .75]],
    ['ÿ\0\t\r\n', '<>&\0\rA\nB\x7f', ['"<>&\nA\nB\x7f"', "- ÿ\t\n, today's longest", 'with a length of 10'], [.5, 1, .5, .5, .75, .5, .5, .65]],
    ['123456ÿ7', 'okay', ['"okay"', "- today's longest", 'with a length of 10'], [.5, 1, .5, .5, .5, .65]],
    ['123456A7', '123456 7', ["Today's longest was 123456A7", 'with a length of 10'], [.5, .75, .5, .65]],
  ]) {
    f.hud([packet(name, message)]); view = f.scene.announcement_view();
    eq(view.lines.map(line => line.text), expected);
    ok(Object.isFrozen(view.blocks));
    for (const block of view.blocks) {
      ok(Object.isFrozen(block) && Object.isFrozen(block.fragments));
      for (const fragment of block.fragments) ok(Object.isFrozen(fragment));
    }
    eq(view.blocks.flatMap(block => block.fragments.filter(fragment => fragment.kind === 'text').map(fragment => fragment.alpha)), alpha);
  }
  const max = packet('', 'ÿ'.repeat((1 << 20) - 8));
  f.hud([max], plan([max]), { bytes: max }); view = f.scene.announcement_view();
  eq(view.blocks[0].fragments[1].text.length, (1 << 20) - 8);
  eq(view.lines[0].text.length, (1 << 20) - 6, 'maximum bare command retains complete text');
  const tooLarge = new Uint8Array(max.length + 1); tooLarge.set(max);
  f.atomic(() => f.hud([tooLarge], plan([tooLarge]), { bytes: tooLarge }));
  const before = view;
  f.atomic(() => f.hud([setup(17), packet('new', 'new'), [109]]));
  eq(f.scene.announcement_view(), before);
  f.atomic(() => f.hud([packet('new', 'new'), packet('outside', '', 0xffffff)]));
  eq(f.scene.announcement_view(), before);
  for (const commands of [
    [setup(1), packet('outside', '', 0xffffff)],
    [setup(29), spawn(3), food, prey(31), setup(1), packet('outside', '', 2050)],
    [setup(1), packet('outside', '', 2050), setup(29)],
    [setup(29), packet('outside', '', 2078), setup(1)],
  ]) {
    const callbacks = { ...f.calls };
    f.atomic(() => f.hud(commands));
    eq(f.calls, callbacks, 'all source resolvers remain uncalled on late invalid announcement');
    eq(f.scene.announcement_view(), before);
  }
  const fresh = fixture(); fresh.scene.enable_announcement_tracking();
  fresh.atomic(() => fresh.hud([packet('missing-table', ''), setup(1)]));
  eq(fresh.scene.announcement_view().present, false);
  f.hud([packet('zero', 'ignored', 0)]); eq(f.scene.announcement_view(), before, 'nonpositive score retains content');
  const inclusive = fixture(); inclusive.scene.enable_announcement_tracking();
  inclusive.hud([setup(1), packet('', '', 2049), setup(29), packet('', '', 2077), setup(1)]);
  eq(inclusive.scene.announcement_view().lines[0].text,
    `Today's longest: ${expectedScore(29, 2077, 1)}`, 'inclusive bounds use each ordered table, not the final Setup');
  const ordered = fixture(); ordered.scene.enable_announcement_tracking();
  const alive = () => ({ announcementLifecycle: {
    schema: 1, deathPending: false, returnActive: false, returnCompleted: false, returnFraction: 1,
  } });
  const board = [108, 0, 0, 1, 0, 1];
  ordered.hud([setup(1), packet('N', 'M'), board]);
  let tick = ordered.scene.advance_with_control(20, false, alive); tick.free();
  ok(ordered.scene.announcement_view().opacity > 0, 'Setup/m/l starts fade');
  ordered.hud([board, setup(1), packet('N', 'M')]);
  tick = ordered.scene.advance_with_control(40, false, alive); tick.free();
  eq(ordered.scene.announcement_view().opacity, 0, 'l/Setup/m hides until a later board');
  ordered.hud([board]);
  tick = ordered.scene.advance_with_control(60, false, alive); tick.free();
  const held = ordered.scene.announcement_view();
  tick = ordered.scene.advance_with_control(80, false, () => ({ announcementLifecycle: {
    schema: 1, deathPending: true, returnActive: false, returnCompleted: false, returnFraction: 1,
  } })); tick.free();
  eq(ordered.scene.announcement_view(), held, 'death hold freezes rich projection and opacity');
  const life = { schema: 1, deathPending: true, returnActive: true, returnCompleted: false, returnFraction: .6 };
  const step = f.scene.advance_with_control(20, false, () => ({ announcementLifecycle: life })); step.free();
  eq(f.scene.announcement_view().opacity, .6);
  const state = f.state(); let getters = 0;
  rejects(() => f.scene.advance_with_control(40, false, () => ({ announcementLifecycle: {
    ...life, get returnFraction() { getters++; return .5; },
  } })));
  eq(getters, 0); eq(f.state(), state); eq(f.scene.announcement_view().opacity, .6);
  rejects(() => f.scene.advance_with_control(40, false, () => ({ announcementLifecycle: { ...life, returnFraction: NaN } })));
  eq(f.state(), state);
  const completed = f.scene.advance_with_control(40, false, () => ({ announcementLifecycle: {
    schema: 1, deathPending: false, returnActive: true, returnCompleted: true, returnFraction: 0,
  } })); completed.free();
  eq(f.scene.announcement_view().opacity, -1);
  const observer = f.scene.announcement_view();
  eq(f.scene.announcement_view(), observer, 'observations consume no time');
}
