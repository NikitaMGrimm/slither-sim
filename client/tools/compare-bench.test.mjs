import test from 'node:test';
import assert from 'node:assert/strict';
import { csv, presentation, hostStats, validity, wholeMap, schedule, netlogProof, comparison } from './compare-bench/analysis.mjs';
import { endpointFrom } from './compare-bench/run.mjs';
import { adapterFlags } from './compare-bench/browser.mjs';

test('CSV quoting, BOM and malformed rows', () => {
  assert.deepEqual(csv('\uFEFFa,b\r\n"a,b","x""y"\r\n'), [{ a: 'a,b', b: 'x"y' }]);
  assert.throws(() => csv('a,b\n1\n'));
  assert.throws(() => csv('a\n"unterminated'));
});
test('display timestamps include latency and select exactly one PID/surface', () => {
  const header = 'ProcessID,SwapChainAddress,CPUStartQPCTime,DisplayLatency,DisplayedTime,PresentMode,SyncInterval\n';
  const text = header + ['7,A,100,10,10,Flip,1', '7,A,110,20,10,Flip,1', '7,A,120,100,NA,Flip,1', '7,A,200,10,10,Flip,1', '8,A,101,1,2,Flip,0', '7,B,102,1,2,Flip,0'].join('\n');
  const m = presentation(text, { pid: 7, swapchain: 'A', seconds: .2, startMs: 100, endMs: 300 });
  assert.equal(m.longestStallMs, 80);
  assert.equal(m.displayedFrames, 3);
  assert.equal(m.stallsOver50Ms, 1);
  assert.deepEqual(m.syncIntervals, ['1']);
  assert.throws(() => presentation('ProcessID,MsBetweenPresents\n7,10', {}), /no timing fallback/);
});
test('invalid/unknown validity cannot silently pass', () => {
  assert.equal(validity({}).valid, false);
  const run = { metrics: { coveredMs: 1000, boundaryGapMs: [0, 0] }, seconds: 1, dt: .007, bots: 100,
    scenario: 'battledome', networkVerified: true, width: 960, height: 540, display: 'D', dpi: 96,
    adapter: 'synthetic', renderAdapters: [{ name: 'Synthetic GPU', luid: 'luid1' }], refreshHz: 60, vsync: 'Fifo',
    windowSamples: [{ foreground: true, occluded: false, minimized: false, visible: true, width: 960, height: 540, display: 'D', dpi: 96 }],
    stats: [{ ticksPerSecond: 142.85, deferredTicks: 0, lostSeconds: 0, bots: 100, players: 1 }],
    census: [{ wholeMapFood: true, admittedBotSlots: 100, totalAliveSnakes: 100, players: [{ alive: true, setup: true, protocol: 19, wireLivingSnakes: 100 }] }],
    views: [{ wholeMap: true, visiblePellets: 40, retainedPellets: 40 }] };
  assert.equal(validity(run).valid, true);
  for (const mutation of [r => r.windowSamples[0].foreground = false, r => r.windowSamples[0].occluded = true,
    r => r.windowSamples[0].buildRunning = true, r => r.stats[0].deferredTicks = 1,
    r => r.census[0].players[0].alive = false, r => r.views[0].wholeMap = false,
    r => r.census[0].wholeMapFood = false, r => r.networkVerified = false,
    r => r.metrics.boundaryGapMs = [1000, 0]]) {
    const copy = structuredClone(run); mutation(copy); assert.equal(validity(copy).valid, false);
  }
});
test('adapter and refresh mismatches are labelled, never direct comparisons', () => {
  const run = { validity: { valid: true }, renderAdapters: [{ luid: 'A' }], display: 'D', refreshHz: 240, width: 1280, height: 720, dpi: 120, scenario: 'normal', experiment: 'primary', vsyncCondition: 'display-paced' };
  assert.equal(comparison(run, structuredClone(run)).classification, 'direct comparison');
  for (const change of [r => r.renderAdapters = [{ luid: 'B' }], r => r.refreshHz = 60, r => r.validity.valid = false]) {
    const other = structuredClone(run); change(other); assert.equal(comparison(run, other).classification, 'labelled comparison only');
  }
});
test('Chrome adapter selection is explicit, with no silent malformed fallback', () => {
  assert.deepEqual(adapterFlags('default'), []);
  assert.deepEqual(adapterFlags('luid_0x00000000_0x00000123'), ['--use-gl=angle', '--use-angle=d3d11', '--use-adapter-luid=0,291']);
  assert.throws(() => adapterFlags('NVIDIA'), /exact DXGI/);
});
test('host stats retain overload; camera accounts for offset', () => {
  assert.equal(hostStats('stats: ticks/s=100.00 deferred_ticks=3 lost_seconds=0.025000 simulation_load=41.2% players=1 bot_slots=500')[0].deferredTicks, 3);
  assert.equal(wholeMap({ camera: [0, 0], center: 0, radius: 100, zoom: 1, viewport: [200, 200] }), true);
  assert.equal(wholeMap({ camera: [1, 0], center: 0, radius: 100, zoom: 1, viewport: [200, 200] }), false);
});
test('round order reverses and keeps every variant', () => {
  assert.deepEqual(schedule(3, ['a', 'b']).map(x => x.client), ['a', 'b', 'b', 'a', 'a', 'b']);
});
test('network evidence rejects remote peers and missing game connection', () => {
  const make = addresses => ({ constants: { logEventTypes: { TCP_CONNECT_ATTEMPT: 5 } }, events: addresses.map(remote_address => ({ type: 5, params: { remote_address } })) });
  assert.equal(netlogProof(make(['127.0.0.1:12']), '127.0.0.1:12').verified, true);
  assert.equal(netlogProof(make(['127.0.0.1:12', '203.0.113.1:80']), '127.0.0.1:12').verified, false);
  assert.equal(netlogProof(make(['127.0.0.1:13']), '127.0.0.1:12').verified, false);
});
test('readiness admits only complete allowed literal-loopback announcements', () => {
  assert.equal(endpointFrom('Listening on ws://127.0.0.1:49152/slither\n'), 'ws://127.0.0.1:49152/slither');
  assert.equal(endpointFrom('Listening on ws://127.0.0.1:49'), null);
  assert.equal(endpointFrom('Listening on ws://203.0.113.1:49152/slither\n'), null);
  for (const port of [0, 8766, 18884, 18885, 65536]) assert.throws(() => endpointFrom(`Listening on ws://127.0.0.1:${port}/slither\n`));
});
