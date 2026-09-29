// The simulate package's pure logic (docs/investigations/2026-09-29-m11/PLAN.md 9.1): the
// Simulate step's sub, the Run label, the "Before running" rows and the elapsed m:ss. A stub
// from the M11 foundation: the package fills it in, with its own model.test.ts.
import type { RunsView } from '../../bindings/ipc';
import type { ActiveRun } from '../../store';

/**
 * The Simulate step's sub in the step bar: "<p> %" while running, else "run <n> <status>", else
 * empty. The foundation's stub is always empty.
 */
export function simulateSub(_run: ActiveRun | null, _runs: RunsView | null): string {
  return '';
}
