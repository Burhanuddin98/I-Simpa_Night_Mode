// node --test suite for m11-focus's judge (PLAN.md 4.2, item 3). Each synthetic log gets its
// verdict: a steal with no input fails; a click inside the window 200 ms before passes; a click
// elsewhere 200 ms before fails; a key 200 ms before passes; a minimised sample fails; plus the
// watcher's own liveness, closing windows and sessions never shown.
import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { type FocusRecord, judge, parseLog, type WindowSample } from './focus.ts';

const START: FocusRecord = {
  kind: 'start',
  t: 0,
  wall: '2026-09-29T21:00:00+02:00',
  root: 100,
  sample_ms: 250,
  hooks: { foreground: true, mouse: true, keyboard: true },
};
const STOP: FocusRecord = { kind: 'stop', t: 60_000, reason: 'stop_file', fg: 1, mouse: 1, key: 1, samples: 3 };
const APP = { hwnd: '0x10', pid: 200, title: 'I-Simpa Night Mode', cls: 'Tauri Window' };
const win = (over: Partial<WindowSample> = {}): WindowSample => ({ ...APP, visible: true, iconic: false, monitor: true, ...over });
const sample = (t: number, windows: WindowSample[]): FocusRecord => ({ kind: 'sample', t, apps: [200], windows });
const steal = (t: number, gate = true): FocusRecord => ({
  kind: 'fg',
  t,
  hwnd: '0x10',
  pid: 200,
  name: 'app.exe',
  title: 'I-Simpa Night Mode',
  cls: 'Tauri Window',
  rect: [100, 100, 1300, 900],
  gate,
  chain: ['app.exe:200', 'msedgedriver.exe:150', 'powershell.exe:100'],
});
const log = (...rs: FocusRecord[]) => [START, sample(250, [win()]), ...rs, STOP];

test('a quiet, visible session passes', () => {
  const v = judge(log(sample(500, [win()])), { minSessions: 1 });
  assert.deepEqual(v.failures, []);
  assert.equal(v.ok, true);
  assert.equal(v.sessions, 1);
});

test('a steal with no input fails', () => {
  const v = judge(log(steal(5_000)));
  assert.equal(v.ok, false);
  assert.match(v.failures[0], /^FOCUS TAKEN: app\.exe 'I-Simpa Night Mode'/);
});

test('a click inside the window 200 ms before excuses it', () => {
  const v = judge(log({ kind: 'mouse', t: 4_800, injected: false, x: 500, y: 500 }, steal(5_000)));
  assert.equal(v.ok, true, v.failures.join('\n'));
  assert.match(v.excused[0], /a real click inside it 200 ms before/);
});

test('a click elsewhere 200 ms before does not', () => {
  const v = judge(log({ kind: 'mouse', t: 4_800, injected: false, x: 50, y: 50 }, steal(5_000)));
  assert.equal(v.ok, false);
});

test('a key 200 ms before excuses it', () => {
  const v = judge(log({ kind: 'key', t: 4_800, injected: false }, steal(5_000)));
  assert.equal(v.ok, true, v.failures.join('\n'));
  assert.match(v.excused[0], /a real key 200 ms before/);
});

test('injected input, input too early, and input after the change excuse nothing', () => {
  assert.equal(judge(log({ kind: 'key', t: 4_800, injected: true }, steal(5_000))).ok, false);
  assert.equal(judge(log({ kind: 'mouse', t: 4_800, injected: true, x: 500, y: 500 }, steal(5_000))).ok, false);
  assert.equal(judge(log({ kind: 'key', t: 3_900, injected: false }, steal(5_000))).ok, false);
  assert.equal(judge(log(steal(5_000), { kind: 'key', t: 5_100, injected: false })).ok, false);
});

test("a foreground change outside the gate's tree is not the gate's", () => {
  assert.equal(judge(log(steal(5_000, false))).ok, true);
});

test('a minimised sample fails', () => {
  const v = judge(log(sample(500, [win({ iconic: true })]), sample(750, [win()])));
  assert.equal(v.ok, false);
  assert.match(v.failures[0], /^WINDOW MINIMISED/);
});

test('a window on no monitor fails', () => {
  const v = judge(log(sample(500, [win({ monitor: false })])));
  assert.match(v.failures[0], /^WINDOW ON NO MONITOR/);
});

test('a hidden window that stays fails; one hidden while it closes does not', () => {
  const hidden = judge(log(sample(500, [win({ visible: false })]), sample(750, [win({ visible: false })]), sample(1_000, [win({ visible: false })]), sample(2_000, [win({ visible: false })])));
  assert.equal(hidden.ok, false);
  assert.match(hidden.failures[0], /^WINDOW INVISIBLE/);
  assert.equal(hidden.failures.length, 1, 'one report per window');
  const closing = judge(log(sample(500, [win({ visible: false })]), sample(750, [])));
  assert.deepEqual(closing.failures, []);
  assert.match(closing.notes.join('\n'), /closing/);
  // Gone, but only after the grace: hidden for too long first.
  const slow = judge(log(sample(500, [win({ visible: false })]), sample(1_750, [])));
  assert.equal(slow.ok, false);
});

test('a window not yet shown is not judged hidden; a session never shown is', () => {
  const late = judge([START, sample(250, [win({ visible: false })]), sample(500, [win()]), STOP]);
  assert.deepEqual(late.failures, []);
  const never = judge([START, sample(250, [win({ visible: false })]), sample(500, [win({ visible: false })]), STOP]);
  assert.equal(never.ok, false);
  assert.match(never.failures.join('\n'), /none was ever seen visible/);
  // IME windows and untitled helpers are not the app's windows.
  const ime = judge([START, sample(250, [win(), win({ hwnd: '0x20', title: 'Default IME', cls: 'IME', visible: false })]), STOP]);
  assert.deepEqual(ime.failures, []);
});

test('fewer sessions than the gate opened fails', () => {
  assert.equal(judge([START, STOP], { minSessions: 1 }).ok, false);
  assert.equal(judge([START, sample(250, [win()]), STOP], { minSessions: 1 }).ok, true);
});

test('a watcher that did not run, did not stop, or lost a hook fails', () => {
  assert.equal(judge([STOP]).ok, false);
  assert.equal(judge([START]).ok, false);
  assert.equal(judge([{ ...START, hooks: { foreground: true, mouse: false, keyboard: true } } as FocusRecord, STOP]).ok, false);
  assert.equal(judge([START, { kind: 'error', t: 5, message: 'SetWinEventHook failed' }, STOP]).ok, false);
});

test('the log parses line by line; a broken line is an error', () => {
  const text = `${JSON.stringify(START)}\r\n${JSON.stringify(sample(250, [win()]))}\n{broken\n${JSON.stringify(STOP)}\n`;
  const rs = parseLog(text);
  assert.equal(rs.length, 4);
  assert.equal(rs[2].kind, 'error');
  assert.equal(judge(rs).ok, false);
});
