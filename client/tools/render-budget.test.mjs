import test from 'node:test';
import assert from 'node:assert/strict';
import { screen, alpha, heading, transforms } from './render-budget.mjs';

test('visual budgets admit small drift and reject visible geometry/opacity regressions', () => {
  screen([[0.3, 0.4]], [[0, 0]]);
  assert.throws(() => screen([[0.4, 0.4]], [[0, 0]]));
  screen(Array(19).fill([0, 0]).concat([[1, 0]]), Array(20).fill([0, 0]));
  assert.throws(() => screen(Array(19).fill([0, 0]).concat([[1.01, 0]]), Array(20).fill([0, 0])));
  alpha(0.505, 0.5);
  assert.throws(() => alpha(0.55, 0.5));
  assert.throws(() => alpha(0.001, 0));
  assert.throws(() => alpha(0.999, 1));
  heading(Math.PI * 2, 0);
  assert.throws(() => heading(0.01, 0));
  assert.throws(() => transforms([['rotate', 0], ['translate', 0, 0]], [['translate', 0, 0], ['rotate', 0]]));
});
