import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { NOT_A_NUMBER, parseStrictDecimal } from './numbers.ts';

test('strict decimals are read, correctly rounded', () => {
  for (const [text, want] of [
    ['0.5', 0.5],
    ['.5', 0.5],
    ['5.', 5],
    ['+1e-3', 0.001],
    ['-2.5E2', -250],
    [' 0.25 ', 0.25],
    ['0.39509836780726515', 0.39509836780726515],
  ] as const) {
    const r = parseStrictDecimal(text);
    assert.ok(r.ok, text);
    assert.equal(r.value, want, text);
  }
  const negZero = parseStrictDecimal('-0');
  assert.ok(negZero.ok && Object.is(negZero.value, -0));
});

test('anything else is NOT_A_NUMBER', () => {
  for (const text of ['0,5', 'NaN', 'Infinity', '-Infinity', '', ' ', '0x10', '1e', '.', '1.2.3', '1e400', '½']) {
    const r = parseStrictDecimal(text);
    assert.ok(!r.ok, text);
    assert.equal(r.code, NOT_A_NUMBER);
  }
});
