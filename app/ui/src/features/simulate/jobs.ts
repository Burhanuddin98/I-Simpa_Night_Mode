// Parity C40, upstream's job list (SystemScript/job_tool: "Add calculation in the job list", "Run job
// list", "Clear job list"; Docs/code_configuration_job.rst): several runs queued, run one after
// another while nobody watches. Upstream's job is a project file and a solver; it opens the file,
// runs, and saves. Here a job is a saved project file, a solver and SPPS's device; the list opens
// each file (the save prompt first, as Open), runs it, and waits for it to end before the next.
// Pure: actions.ts runs the list, JobList.tsx draws it, jobs.test.ts holds the rules.
//
// Upstream's warning holds here too: a job runs the project's file as saved, so two jobs of one
// project with nothing changed between them repeat the same calculation. A job is therefore added
// only from a saved project with no unsaved changes, and the same file, solver and device waits in
// the list once.
import type { RunStatusUi } from '../../bindings/ipc';
import type { SolverName, SppsDevice } from '../../store';

export type JobStatus = 'waiting' | 'running' | 'refused' | Exclude<RunStatusUi, 'RUNNING'>;

export interface Job {
  id: number;
  /** The project file the job opens. */
  path: string;
  name: string;
  solver: SolverName;
  device: SppsDevice;
  status: JobStatus;
  /** The run folder the job made, once it has. */
  run?: string;
  /** Why it was refused, or what stopped it, in words. */
  note?: string;
}

/** Windows paths name one file whatever their case or slashes. */
export const samePath = (a: string | null | undefined, b: string | null | undefined): boolean =>
  !!a && !!b && a.replace(/\//g, '\\').toLowerCase() === b.replace(/\//g, '\\').toLowerCase();

/** Why the open project cannot be added as a job now, or null when it can. */
export function addRefusal(
  info: { path?: string | null; dirty: boolean } | null,
  solver: SolverName,
  device: SppsDevice,
  list: readonly Job[],
): string | null {
  if (!info) return 'Open a project first';
  if (!info.path) return 'Save the project first: a job runs the project file as saved';
  if (info.dirty) return 'Save the changes first: a job runs the project file as saved, not what is on screen';
  const dev = solver === 'spps' ? device : 'cpu';
  if (list.some((j) => j.status === 'waiting' && samePath(j.path, info.path) && j.solver === solver && j.device === dev)) {
    return 'This project and solver already wait in the job list; change the project and save it under another name for another calculation';
  }
  return null;
}

/** The first job still waiting, or null. */
export function nextJob(list: readonly Job[]): Job | null {
  return list.find((j) => j.status === 'waiting') ?? null;
}

/** `list` with job `id` changed by `patch`. */
export function withJob(list: readonly Job[], id: number, patch: Partial<Job>): Job[] {
  return list.map((j) => (j.id === id ? { ...j, ...patch } : j));
}

/** What a job's status reads as. */
export function jobStatusText(j: Job): string {
  switch (j.status) {
    case 'waiting':
      return 'Waiting';
    case 'running':
      return 'Running';
    case 'refused':
      return 'Not run';
    case 'OK':
      return 'Ended OK';
    case 'FAIL':
      return 'Failed';
    case 'CRASH':
      return 'Solver crashed';
    case 'CANCELLED':
      return 'Cancelled';
    case 'INTERRUPTED':
      return 'Interrupted';
  }
}

/** The list's tally once it stops: `3 jobs run: 2 OK, 1 failed; 1 not run`. */
export function jobsSummary(list: readonly Job[]): string {
  const ran = list.filter((j) => j.status !== 'waiting' && j.status !== 'refused' && j.status !== 'running');
  const ok = ran.filter((j) => j.status === 'OK').length;
  const notRun = list.filter((j) => j.status === 'refused').length;
  const waiting = list.filter((j) => j.status === 'waiting').length;
  const parts = [`${ran.length} ${ran.length === 1 ? 'job' : 'jobs'} run: ${ok} OK${ran.length - ok > 0 ? `, ${ran.length - ok} not OK` : ''}`];
  if (notRun > 0) parts.push(`${notRun} not run`);
  if (waiting > 0) parts.push(`${waiting} still waiting`);
  return parts.join('; ');
}
