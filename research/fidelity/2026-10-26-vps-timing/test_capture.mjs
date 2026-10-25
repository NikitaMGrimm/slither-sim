import assert from 'node:assert/strict';
import { test } from 'node:test';
import { failureClose } from './capture.mjs';

test('unexpected close fails; a death can rejoin only after close; shutdown is not a retry', () => {
  assert.equal(failureClose({ stopped:false, deathObservation:null }), true);
  assert.equal(failureClose({ stopped:false, deathObservation:{} }), false);
  assert.equal(failureClose({ stopped:true, deathObservation:null }), false);
});
