// The landing page (Burhan, 2026-10-06 01:24): while no project is open it stands in for the
// empty window, a card over the 3D view's background with the shipped examples (landingModel.ts), New
// project, New box room (G11, BoxRoomDialog.tsx) and Open. Each action is the one File › New project, File › Open… and the examples
// command run (actions.ts), so the save prompt and the run guard hold here too. A failure is
// shown on the card, since the Console is behind it. The menu bar stays above it and the status
// bar beside it; App.tsx makes the regions behind it inert, so Tab moves only through the card.
import { useState } from 'react';
import * as actions from '../actions';
import { boxRoomRequestStore, runStore, sceneStore, useStore } from '../store';
import { EXAMPLES, EXAMPLES_HEAD, EXAMPLES_NOTE, exampleLine, LANDING_LEAD, LANDING_TITLE, landingShown } from './landingModel';
import { RUN_ACTIVE_TITLE } from './MenuBar';

export function Landing() {
  const scene = useStore(sceneStore);
  const running = useStore(runStore) !== null;
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  if (!landingShown(scene)) return null;

  const go = (key: string, act: () => Promise<unknown>) => {
    setBusy(key);
    setError(null);
    act()
      .catch((e: unknown) => {
        const err = actions.asCmdError(e);
        setError(`${err.message} (${err.code})`);
      })
      .finally(() => setBusy(null));
  };
  const blocked = busy !== null || running;
  const title = running ? RUN_ACTIVE_TITLE : undefined;

  return (
    <section className="landing" data-part="landing" aria-labelledby="landing-title">
      <div className="landing-card">
        <h1 id="landing-title" className="landing-title">
          {LANDING_TITLE}
        </h1>
        <p className="landing-lead">{LANDING_LEAD}</p>

        <h2 className="landing-head">{EXAMPLES_HEAD}</h2>
        <ul className="landing-examples">
          {EXAMPLES.map((e) => (
            <li key={e.id}>
              <button
                className="landing-example"
                data-example={e.id}
                disabled={blocked}
                title={title}
                aria-busy={busy === e.id}
                onClick={() => go(e.id, () => actions.openExample(e.id))}
              >
                <span className="name">{e.name}</span>
                <span className="line">{exampleLine(e)}</span>
                {busy === e.id && <span className="opening">Opening…</span>}
              </button>
            </li>
          ))}
        </ul>
        <p className="landing-note">{EXAMPLES_NOTE}</p>

        <div className="landing-actions">
          <button
            className="primary"
            data-landing-action="new-project"
            disabled={blocked}
            title={title}
            onClick={() => go('new', () => actions.newProject())}
          >
            New project
          </button>
          <button
            data-landing-action="new-box-room"
            disabled={blocked}
            title={title ?? 'A new project whose model is a closed box of the width, length and height you give'}
            onClick={() => boxRoomRequestStore.set(true)}
          >
            New box room…
          </button>
          <button data-landing-action="open" disabled={blocked} title={title ?? 'Open a project or a room model'} onClick={() => go('open', actions.openDialog)}>
            Open…
          </button>
        </div>

        {error && (
          <div className="issue landing-error" role="alert" data-part="landing-error">
            <span className="state">Error</span>
            <span>{error}</span>
          </div>
        )}
      </div>
    </section>
  );
}
