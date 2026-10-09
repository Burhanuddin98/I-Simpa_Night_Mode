// A38, upstream's file drop (`ISimpaApp::OnFileDrop`, i_simpa_main.h:605-613: the first file
// dropped is opened, after the save prompt): what a drop on the window opens. The window sends the
// paths (src-tauri main.rs, `AppEvent::FilesDropped`); actions.ts opens the one chosen here as
// File › Open… would. Pure, tested by drop.test.ts.

/** What File › Open… takes: a Night Mode project, an I-Simpa project, a room model. */
export const DROP_EXTENSIONS = ['simpa', 'proj', 'ply', 'obj', 'stl'] as const;

export type DropChoice = { path: string; ignored: number } | { problem: string };

/** The file a drop opens: the first one dropped, as upstream, if Open takes it; otherwise why not. */
export function dropChoice(paths: readonly string[]): DropChoice {
  const first = paths[0];
  if (first === undefined) return { problem: 'Nothing was dropped that can be opened' };
  const ext = first.split('.').pop()?.toLowerCase() ?? '';
  const name = first.replace(/^.*[\\/]/, '');
  if (!(DROP_EXTENSIONS as readonly string[]).includes(ext)) {
    return { problem: `${name} is not a file Night Mode opens: drop a project (.simpa, .proj) or a room model (PLY, OBJ, STL)` };
  }
  return { path: first, ignored: paths.length - 1 };
}
