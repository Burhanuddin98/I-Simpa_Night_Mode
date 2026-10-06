// The landing page's model (Burhan, 2026-10-06 01:24: "a landing page with the default or a start
// a new project type deal"): shown while no project is open, it offers the example projects that
// ship inside the app (src-tauri/src/examples.rs, the files in src-tauri/examples/), New project
// and Open. Pure: Landing.tsx draws it, landingModel.test.ts holds every card to its file.
//
// The cards say what a room is and what is already set, in counts and words only: no number a
// solver computes or a unit is ever shown here (M10 rule 6, the self-test's no_acoustic_numbers).
// To add a room (BRAS CR1 and CR3 are next), add its row here and in examples.rs.
import type { SceneState } from '../bindings/ipc';

export interface ExampleCard {
  /** The id `example_open` takes (examples.rs `EXAMPLES`). */
  id: string;
  name: string;
  /** What the room is, in plain words. */
  what: string;
  /** The shipped file under src-tauri/examples/, for the test. */
  file: string;
  sources: number;
  /** Point receivers. */
  receivers: number;
  /** Surface receivers: the Sound-level planes section's planes. */
  planes: number;
  variants: number;
}

export const LANDING_TITLE = 'Start with a room';
export const LANDING_LEAD = 'Open an example that is ready to run, or start a project of your own.';
export const EXAMPLES_HEAD = 'Examples';
export const EXAMPLES_NOTE = 'Each opens as your own copy in Documents, Night Mode, Examples.';

export const EXAMPLES: readonly ExampleCard[] = [
  {
    id: 'elmia',
    name: 'Elmia hall',
    what: 'A concert hall, I-Simpa’s own tutorial room',
    file: 'elmia_hall.simpa',
    sources: 3,
    receivers: 6,
    planes: 2,
    variants: 0,
  },
  {
    id: 'industrial',
    name: 'Industrial hall',
    what: 'A factory hall, I-Simpa’s third tutorial room, with two milling machines among the machinery',
    file: 'industrial_hall.simpa',
    sources: 6,
    receivers: 5,
    planes: 1,
    variants: 0,
  },
  {
    id: 'bras-cr1',
    name: 'BRAS CR1',
    what: 'Coupled rooms from the BRAS benchmark, a laboratory opening onto a reverberation chamber',
    file: 'bras_cr1.simpa',
    sources: 2,
    receivers: 2,
    planes: 0,
    variants: 0,
  },
  {
    id: 'bras-cr2',
    name: 'BRAS CR2',
    what: 'A seminar room from the BRAS benchmark',
    file: 'bras_cr2.simpa',
    sources: 2,
    receivers: 5,
    planes: 0,
    variants: 0,
  },
  {
    id: 'bras-cr3',
    name: 'BRAS CR3',
    what: 'A chamber music hall from the BRAS benchmark',
    file: 'bras_cr3.simpa',
    sources: 2,
    receivers: 5,
    planes: 0,
    variants: 0,
  },
  {
    id: 'bras-cr4',
    name: 'BRAS CR4',
    what: 'An auditorium from the BRAS benchmark',
    file: 'bras_cr4.simpa',
    sources: 2,
    receivers: 5,
    planes: 1,
    variants: 1,
  },
];

const WORDS = ['no', 'one', 'two', 'three', 'four', 'five', 'six', 'seven', 'eight', 'nine'];

/** `2 sources`, `a sound-level plane`: counts, never a unit. */
export function counted(n: number, one: string, many: string, article = false): string {
  if (n === 1) return article ? `a ${one}` : `1 ${one}`;
  return `${n} ${many}`;
}

/** `a, b and c`. */
export function listed(parts: readonly string[]): string {
  if (parts.length <= 1) return parts.join('');
  return `${parts.slice(0, -1).join(', ')} and ${parts[parts.length - 1]}`;
}

/** The card's one line: what the room is and what is already set. */
export function exampleLine(e: ExampleCard): string {
  const set = [counted(e.sources, 'source', 'sources'), counted(e.receivers, 'receiver', 'receivers')];
  if (e.planes > 0) set.push(counted(e.planes, 'sound-level plane', 'sound-level planes', true));
  set.push('every surface’s material');
  if (e.variants > 0) set.push(`${WORDS[e.variants] ?? e.variants} variant ${e.variants === 1 ? 'set' : 'sets'}`);
  return `${e.what}: ${listed(set)} — ready to run.`;
}

/** The landing page stands in for the empty window: shown exactly while no project is open. */
export function landingShown(scene: SceneState | null): boolean {
  return scene === null;
}
