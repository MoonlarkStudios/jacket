import assert from 'node:assert/strict';
import test from 'node:test';
import { validateTitle } from './pr-title.mjs';

test('Conventional Commit titles allow scopes and breaking changes', () => {
  for (const title of [
    'feat: add album search',
    'fix(linux): retain unfinished work',
    'feat!: change exported manifest',
    'chore(main): release 0.1.0-beta.1',
  ])
    validateTitle(title);
  for (const title of [
    'Add search',
    'feat: ',
    'fix: bad\nextra',
    'feat scope: wrong',
  ]) {
    assert.throws(() => validateTitle(title), /Conventional Commit/);
  }
});
