// node --test support (not app code, never bundled): module resolution hooks that let a suite run
// actions.ts under node. The UI imports its own modules without an extension (Vite resolves
// them), and the two Tauri modules exist only inside the app's webview; here each extensionless
// relative import resolves to its .ts or .tsx file, and the Tauri modules to the stand-ins beside
// this file. A suite registers it first: `register('./testing/hooks.mjs', import.meta.url)`.
import { existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const here = new URL('.', import.meta.url);
const STANDINS = {
  '@tauri-apps/api/core': 'tauri-core.mjs',
  '@tauri-apps/plugin-dialog': 'tauri-dialog.mjs',
};

export async function resolve(specifier, context, next) {
  const standin = STANDINS[specifier];
  if (standin) return { url: new URL(standin, here).href, shortCircuit: true };
  const relative = specifier.startsWith('./') || specifier.startsWith('../');
  if (relative && context.parentURL?.startsWith('file:') && !/\.[cm]?[jt]sx?$|\.json$/.test(specifier)) {
    for (const ext of ['.ts', '.tsx']) {
      const url = new URL(specifier + ext, context.parentURL);
      if (existsSync(fileURLToPath(url))) return { url: url.href, shortCircuit: true };
    }
  }
  return next(specifier, context);
}
