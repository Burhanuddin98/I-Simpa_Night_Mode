// The M12 e2e harness (docs/investigations/2026-10-03-m12/PLAN.md). Run by tools/gates/m12.ps1
// only, which builds the release app, stages a private copy of the verified solvers on C:, uses
// the pinned WebdriverIO under C:\tmp\nm-e2e\node (never on B:, exFAT), and passes the
// environment below. M11's config with specs m12.<name>.e2e.ts (M12_SPEC); the M11_* variables
// keep their meaning, so the shared libraries (lib/m11.ts, lib/hooks.ts) read them unchanged.
//
// The app windows stay visible and on screen, and open without taking keyboard focus: app.exe
// builds its window with focused(false) under --e2e (src-tauri/src/main.rs). Nothing here
// resizes, moves, minimises, hides or switches windows.
//
// No bare-specifier runtime import: wdio resolves its runner, framework and reporters from its
// own install; this file and the specs import only `node:` built-ins and relative files.
import { spawn, type ChildProcess } from 'node:child_process';
import { connect } from 'node:net';
import path from 'node:path';

const need = (k: string): string => {
  const v = process.env[k];
  if (!v) throw new Error(`${k} unset: run tools/gates/m12.ps1`);
  return v;
};

const PORT = 4444;
let driver: ChildProcess | undefined;

/** Resolves once something listens on `port`, or rejects after `ms`. */
function listening(port: number, ms: number): Promise<void> {
  const t0 = Date.now();
  return new Promise((resolve, reject) => {
    const attempt = () => {
      const s = connect(port, '127.0.0.1');
      s.once('connect', () => {
        s.end();
        resolve();
      });
      s.once('error', () => {
        s.destroy();
        if (Date.now() - t0 > ms) reject(new Error(`nothing listens on ${port} after ${ms} ms`));
        else setTimeout(attempt, 100);
      });
    };
    attempt();
  });
}

// Forward slashes: wdio globs the spec pattern, and a backslash is an escape in a glob.
const specDir = path.join(import.meta.dirname, 'specs').replace(/\\/g, '/');
const specs = (process.env.M12_SPEC ?? 'acoustics')
  .split(',')
  .map((s) => `${specDir}/m12.${s.trim()}.e2e.ts`);

export const config = {
  runner: 'local',
  maxInstances: 1,
  specs,
  hostname: '127.0.0.1',
  port: PORT,
  capabilities: [
    {
      maxInstances: 1,
      // WebView2 through msedgedriver speaks WebDriver classic; no BiDi session.
      'wdio:enforceWebDriverClassic': true,
      // msedgedriver's own capabilities, which tauri-driver passes through untouched. Not
      // `tauri:options`: tauri-driver rebuilds ms:edgeOptions from it and cannot carry
      // excludeSwitches. The driver adds --enable-logging to WebView2's switches by default, and
      // with it the WebView2 browser allocates a console window of its own (its parent, app.exe,
      // has none), which Windows activates: that console took the foreground in every session
      // (the focus watcher, 2026-09-29 21:29; FOUNDATION.md F-21). Excluding the one switch keeps
      // the test windows from taking focus; nothing else changes.
      browserName: 'webview2',
      'ms:edgeChromium': true,
      'ms:edgeOptions': { binary: need('M11_APP'), args: ['--e2e'], excludeSwitches: ['enable-logging'] },
    },
  ],
  logLevel: 'warn',
  outputDir: need('M11_WORK'),
  connectionRetryCount: 3,
  waitforTimeout: 30_000,
  framework: 'mocha',
  mochaOpts: { ui: 'bdd', timeout: 300_000 },
  reporters: [
    'spec',
    ['junit', { outputDir: need('M11_WORK'), outputFileFormat: (o: { cid: string }) => `junit-${o.cid}.xml` }],
  ],
  beforeSession: async () => {
    driver = spawn(need('M11_TAURI_DRIVER'), ['--port', String(PORT), '--native-driver', need('M11_NATIVE_DRIVER')], {
      stdio: ['ignore', 'inherit', 'inherit'],
    });
    await listening(PORT, 30_000);
  },
  // The close and kill specs end their app on purpose: the session may already be gone.
  afterSession: () => {
    driver?.kill();
  },
};
