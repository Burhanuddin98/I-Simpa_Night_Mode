// The M10 e2e harness (docs/investigations/2026-09-29-m10/PLAN.md, section 4). Run by
// tools/gates/m10.ps1 only, which builds the release app, installs the pinned WebdriverIO under
// C:\tmp\nm-e2e\node (never on B:, exFAT), and passes the environment below.
//
// No bare-specifier runtime import: wdio resolves its runner, framework and reporters from its
// own install; this file and the specs import only `node:` built-ins and relative files. The
// globals (browser, $, $$) are injected.
//
// Each spec file gets its own worker and session: the app is relaunched with `--e2e` for each,
// so every package's spec starts from a fresh app.
import { spawn, type ChildProcess } from 'node:child_process';
import { connect } from 'node:net';
import path from 'node:path';

const need = (k: string): string => {
  const v = process.env[k];
  if (!v) throw new Error(`${k} unset: run tools/gates/m10.ps1`);
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
const specs = (process.env.M10_SPEC ?? '*')
  .split(',')
  .map((s) => `${specDir}/m10.${s.trim()}.e2e.ts`);

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
      // msedgedriver's own capabilities, passed through tauri-driver untouched, so that
      // --enable-logging can be excluded: with it the WebView2 browser allocates a console window
      // that Windows activates, taking focus from whatever Burhan is using (M11 FOUNDATION.md
      // F-21). The window itself is unchanged: visible, and built unfocused by app.exe.
      browserName: 'webview2',
      'ms:edgeChromium': true,
      'ms:edgeOptions': { binary: need('M10_APP'), args: ['--e2e'], excludeSwitches: ['enable-logging'] },
    },
  ],
  logLevel: 'warn',
  outputDir: need('M10_WORK'),
  connectionRetryCount: 3,
  waitforTimeout: 30_000,
  framework: 'mocha',
  mochaOpts: { ui: 'bdd', timeout: 180_000 },
  reporters: [
    'spec',
    ['junit', { outputDir: need('M10_WORK'), outputFileFormat: (o: { cid: string }) => `junit-${o.cid}.xml` }],
  ],
  beforeSession: async () => {
    driver = spawn(need('M10_TAURI_DRIVER'), ['--port', String(PORT), '--native-driver', need('M10_NATIVE_DRIVER')], {
      stdio: ['ignore', 'inherit', 'inherit'],
    });
    await listening(PORT, 30_000);
  },
  afterSession: () => {
    driver?.kill();
  },
};
