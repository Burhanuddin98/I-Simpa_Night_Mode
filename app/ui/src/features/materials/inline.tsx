// Inline messages of the Materials step: validator issues, refusals of the checked apply, and the
// UI's own refusals (NOT_A_NUMBER, PASTE_SHAPE, PASTE_HEADER), each shown as "FAIL <CODE>:
// message". Red is also the brand colour, so a failure always carries its label as text.
import type { UiIssue } from '../../bindings/ipc';

export interface Line {
  label: 'FAIL' | 'WARN';
  code: string;
  message: string;
}

/** A validator issue or a refusal as a line. */
export function lineOf(issue: UiIssue): Line {
  return { label: issue.severity === 'warning' ? 'WARN' : 'FAIL', code: issue.code, message: issue.message };
}

export function IssueLines({ lines, part }: { lines: readonly Line[]; part: string }) {
  if (lines.length === 0) return null;
  return (
    <div className="mat-issues" role="status" data-part={part}>
      {lines.map((l, i) => (
        <div key={i} className={l.label === 'WARN' ? 'issue warning' : 'issue'} data-issue-code={l.code}>
          <span className="state">{l.label === 'WARN' ? 'Warning:' : 'Error:'}</span>
          <span className="msg"> {l.message} </span>
          <span className="code">{l.code}</span>
        </div>
      ))}
    </div>
  );
}

// ---- refusals ---------------------------------------------------------------------------------
// `refusalStore` keeps a field's refusals until the next accepted edit under the same key. The
// messages under the grid and the material list are the latest attempt's: a new attempt
// dismisses what was shown before it. An entry is dismissed by identity, so a new refusal under
// the same key (a new array) shows again. Module scope, so a remount does not bring old
// messages back.

const dismissed = new WeakSet<readonly UiIssue[]>();

/** Dismisses every refusal now filed under a key `match` accepts. */
export function dismiss(refusals: ReadonlyMap<string, UiIssue[]>, match: (key: string) => boolean): void {
  for (const [key, list] of refusals) if (match(key)) dismissed.add(list);
}

/** The refusals filed under keys `match` accepts that no later attempt has dismissed. */
export function visibleRefusals(refusals: ReadonlyMap<string, UiIssue[]>, match: (key: string) => boolean): UiIssue[] {
  const out: UiIssue[] = [];
  for (const [key, list] of refusals) if (match(key) && !dismissed.has(list)) out.push(...list);
  return out;
}

/** A rejected command's `{code, message}` (the Console already has it as a FAIL line): the
 * backend wrapper's own reading, which actions.ts passes on. */
export { asCmdError as errorOf } from '../../actions';
