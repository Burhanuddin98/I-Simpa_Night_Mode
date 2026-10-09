// node --test suite for Help › About's model (aboutModel.ts).
import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import type { AboutInfo } from '../bindings/ipc.ts';
import { buildLine, LICENCES, patchNumbers, runtimeLine, solversLines } from './aboutModel.ts';

const INFO: AboutInfo = {
  app_version: '0.1.0',
  commit: '0123456789ab',
  profile: 'release',
  tauri_version: '2.11.6',
  webview: { engine: 'WebView2', version: '141.0.3537.71' },
  solvers: {
    upstream_commit: '929a5c8e5f590b189f29155faeff8670f3699479',
    patches: ['0001-surface-receiver-time-bin.patch', '0002-sparse-surface-receiver-series.patch'],
    tetgen_version: '1.5.0',
    spps_gpu_commit: '4b426540e5d1a1d1ef26e8769512b1a763753573',
  },
};

test('about: the build, the runtime and the solvers in one line each', () => {
  assert.equal(buildLine(INFO), 'Version 0.1.0 · build 0123456789ab (release)');
  assert.equal(runtimeLine(INFO), 'Tauri 2.11.6 · WebView2 141.0.3537.71');
  assert.equal(runtimeLine({ ...INFO, webview: { engine: 'WebView2', version: null } }), 'Tauri 2.11.6 · WebView2 (version not read)');
  assert.equal(patchNumbers(INFO.solvers.patches), '0001, 0002');
  assert.deepEqual(solversLines(INFO), [
    'SPPS, TCR and preprocess: upstream I-Simpa commit 929a5c8, with this repository’s patches 0001, 0002',
    'TetGen 1.5.0',
    'SPPS on the GPU: this repository’s commit 4b42654',
  ]);
  assert.match(solversLines({ ...INFO, solvers: { ...INFO.solvers, patches: [] } })[0], /no patches$/);
});

test('about: the licences a release must name (CLAUDE.md: TetGen is AGPL-3.0), and no claim on the patches', () => {
  const by = (what: string) => LICENCES.find((l) => l.what.startsWith(what));
  assert.equal(by('Night Mode')?.licence, 'GPL-3.0');
  assert.equal(by('I-Simpa solvers')?.licence, 'GPL-3.0-or-later');
  assert.equal(by('TetGen')?.licence, 'AGPL-3.0');
  const all = LICENCES.map((l) => `${l.what} ${l.note}`).join(' ');
  // The repository's rule: nothing says the patches are correct or upstream wrong.
  assert.doesNotMatch(all, /\b(fix|fixes|fixed|correct|bug|wrong)\b/i);
  assert.doesNotMatch(all, /https?:\/\//);
});
