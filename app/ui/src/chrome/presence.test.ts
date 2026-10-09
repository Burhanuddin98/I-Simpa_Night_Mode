// presence.ts under `node --test` (Burhan's 10-05 UI list, item 14).
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { CLOSE_MS, closeAnimates, presenceAfter } from './presence.ts';

test('a menu that closes plays its way out, then the caller makes it gone', () => {
  assert.equal(presenceAfter('gone', true, true), 'shown');
  assert.equal(presenceAfter('shown', true, true), 'shown');
  assert.equal(presenceAfter('shown', false, true), 'closing');
  assert.equal(presenceAfter('closing', false, true), 'closing', 'it stays until its time is up');
  assert.equal(presenceAfter('gone', false, true), 'gone');
});

test('a closing menu opened again turns back', () => {
  assert.equal(presenceAfter('closing', true, true), 'shown');
});

test('with no motion a closed menu goes at once, as before', () => {
  assert.equal(presenceAfter('shown', false, false), 'gone');
  assert.equal(presenceAfter('closing', false, false), 'gone', 'motion switched off mid-close');
  assert.equal(presenceAfter('gone', true, false), 'shown');
});

test('nothing animates under reduced motion or WebDriver', () => {
  assert.equal(closeAnimates(false, false), true);
  assert.equal(closeAnimates(true, false), false);
  assert.equal(closeAnimates(false, true), false);
  assert.equal(closeAnimates(true, true), false);
});

test('a menu stays mounted as long as its longest transition', () => {
  // motion.css: opacity 150 ms, transform 170 ms.
  assert.equal(CLOSE_MS, 170);
});
