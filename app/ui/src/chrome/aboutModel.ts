// Help › About's model (parity A23): the build as the core reports it (`app_about`, about.rs), and the
// licences of what the app carries, in words. The full texts are opened through Help (help.rs
// `PAGES`: the GPL-3.0 and THIRD-PARTY-NOTICES.txt), never written here.
//
// Pure: AboutDialog.tsx draws it; aboutModel.test.ts holds the lines.
import type { AboutInfo } from '../bindings/ipc';

export const ABOUT_TITLE = 'I-Simpa Night Mode';
export const ABOUT_LEAD =
  'A graphical interface for room-acoustics simulation over the solvers of I-Simpa, the open-source project of Université Gustave Eiffel. It is not upstream I-Simpa’s own interface.';

export interface LicenceRow {
  what: string;
  licence: string;
  note: string;
}

/** What the app and its solvers carry, and under which licence (the notices hold the receipts). */
export const LICENCES: readonly LicenceRow[] = [
  {
    what: 'Night Mode',
    licence: 'GPL-3.0',
    note: 'This app’s own code. You may run, study, change and pass it on under the GNU General Public License, version 3; Help › Night Mode source code has its source.',
  },
  {
    what: 'I-Simpa solvers: SPPS, TCR, preprocess',
    licence: 'GPL-3.0-or-later',
    note: '© 2007–2022 Université Gustave Eiffel, Judicaël Picaut, Nicolas Fortin. Built from upstream’s source with this repository’s patches, listed above.',
  },
  {
    what: 'TetGen, the volume mesher',
    licence: 'AGPL-3.0',
    note: '© Hang Si, Weierstrass Institute (WIAS). Dual-licensed: the GNU Affero GPL, version 3, or a paid licence from WIAS. This free release uses it under the AGPL-3.0.',
  },
  {
    what: 'SPPS on the GPU',
    licence: 'GPL-3.0',
    note: 'This repository’s port of SPPS. It includes TinyXML-2 (zlib licence).',
  },
  {
    what: 'Fonts: Inter, JetBrains Mono',
    licence: 'OFL-1.1',
    note: 'The SIL Open Font License, shipped with the fonts.',
  },
  {
    what: 'Example rooms',
    licence: 'GPL-3.0 · CC BY-SA 4.0',
    note: 'The Elmia hall, the industrial hall and Tutorials 1 to 3 come from upstream I-Simpa’s tutorials (GPL-3.0); BRAS CR1 to CR4 are adapted from the BRAS benchmark (CC BY-SA 4.0).',
  },
  {
    what: 'Libraries',
    licence: 'MIT, Apache-2.0 and others',
    note: 'The Rust crates and JavaScript packages compiled into the app, each under its own licence: Third-party notices lists every one, with its licence text.',
  },
];

/** `Version 0.1.0 · build 0123456789ab (release)`. */
export function buildLine(a: AboutInfo): string {
  return `Version ${a.app_version} · build ${a.commit} (${a.profile})`;
}

/** `Tauri 2.11.6 · WebView2 141.0…`, or the webview unread. */
export function runtimeLine(a: AboutInfo): string {
  return `Tauri ${a.tauri_version} · ${a.webview.engine} ${a.webview.version ?? '(version not read)'}`;
}

/** The patches by their number: `0001-surface-receiver-time-bin.patch` → `0001`. */
export function patchNumbers(patches: readonly string[]): string {
  return patches.map((p) => p.split('-')[0]).join(', ');
}

/** The solvers' build, as solvers/manifest.json records it. */
export function solversLines(a: AboutInfo): string[] {
  const s = a.solvers;
  const patches = s.patches.length === 0 ? 'no patches' : `${s.patches.length === 1 ? 'patch' : 'patches'} ${patchNumbers(s.patches)}`;
  return [
    `SPPS, TCR and preprocess: upstream I-Simpa commit ${s.upstream_commit.slice(0, 7)}, with this repository’s ${patches}`,
    `TetGen ${s.tetgen_version}`,
    `SPPS on the GPU: this repository’s commit ${s.spps_gpu_commit.slice(0, 7)}`,
  ];
}
