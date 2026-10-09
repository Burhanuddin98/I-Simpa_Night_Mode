// Parity A23, Help › About Night Mode: the version, commit and profile of this build and the solvers'
// build, as the core reports them (`app_about`), and the licences of what the app carries
// (aboutModel.ts). The GPL-3.0 and the third-party notices open in full through Help (help.rs
// `PAGES`), in the default browser or editor. In the import dialog's frame; Esc or Close shuts it.
// Registers the e2e hook `openAbout()`.
import { useEffect, useRef, useState } from 'react';
import * as actions from '../actions';
import type { AboutInfo } from '../bindings/ipc';
import { aboutOpenStore, useStore } from '../store';
import { registerHook } from '../testhooks';
import { ABOUT_LEAD, ABOUT_TITLE, buildLine, LICENCES, runtimeLine, solversLines } from './aboutModel';
import { HELP_LICENCE, HELP_NOTICES } from './helpModel';

function Dialog() {
  const [info, setInfo] = useState<AboutInfo | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const closeButton = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    closeButton.current?.focus();
    actions
      .loadAbout()
      .then(setInfo)
      .catch((e: unknown) => {
        const err = actions.asCmdError(e);
        setProblem(`${err.message} (${err.code})`);
      });
  }, []);
  const close = () => aboutOpenStore.set(false);
  return (
    <div className="dialog-backdrop" role="presentation">
      <div
        className="dialog about"
        role="dialog"
        aria-modal="true"
        aria-labelledby="about-title"
        data-part="about-dialog"
        onKeyDown={(e) => {
          if (e.key === 'Escape') {
            e.preventDefault();
            close();
          }
        }}
      >
        <div className="dialog-title" id="about-title">
          {ABOUT_TITLE}
        </div>
        <p className="about-lead">{ABOUT_LEAD}</p>
        {info && (
          <div className="about-build" data-part="about-build">
            <div className="mono" data-field="about.build">
              {buildLine(info)}
            </div>
            <div className="mono" data-field="about.runtime">
              {runtimeLine(info)}
            </div>
            <div className="about-head">Solvers</div>
            {solversLines(info).map((l) => (
              <div key={l} className="mono" data-field="about.solver">
                {l}
              </div>
            ))}
          </div>
        )}
        {!info && !problem && <div className="dialog-note empty">Reading the build…</div>}
        {problem && (
          <div className="issue" role="alert" data-part="about-problem">
            <span className="state">Error</span>
            <span className="msg">{problem}</span>
          </div>
        )}
        <div className="about-head">Licences</div>
        <table className="about-licences" data-part="about-licences">
          <tbody>
            {LICENCES.map((l) => (
              <tr key={l.what}>
                <th scope="row">
                  {l.what}
                  <span className="about-licence mono">{l.licence}</span>
                </th>
                <td>{l.note}</td>
              </tr>
            ))}
          </tbody>
        </table>
        <div className="dialog-actions">
          {[HELP_LICENCE, HELP_NOTICES].map((l) => (
            <button key={l.id} data-part={l.id} title={l.title} onClick={() => actions.fire(actions.openHelp(l.topic, l.what))}>
              {l.label}
            </button>
          ))}
          <button ref={closeButton} className="primary" data-part="about-close" onClick={close}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}

export function AboutDialog() {
  const open = useStore(aboutOpenStore);
  useEffect(
    () =>
      registerHook('openAbout', () => {
        aboutOpenStore.set(true);
        return true;
      }),
    [],
  );
  return open ? <Dialog /> : null;
}
