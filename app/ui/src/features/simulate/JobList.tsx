// Parity C40: the job list in the Simulate step (jobs.ts, actions.ts `runJobList`): the runs queued,
// each a saved project file, a solver and SPPS's device, run one after another. "Add this project"
// takes the open project as saved; "Run job list" opens each file in turn and runs it; "Stop" cancels
// the job running and starts no other. Kept for the session, across the projects it opens.
import * as actions from '../../actions';
import { runStore, sceneStore, solverStore, deviceStore, useStore } from '../../store';
import { addRefusal, jobStatusText } from './jobs';

export function JobList() {
  const jobs = useStore(actions.jobsStore);
  const { running, stopping } = useStore(actions.jobRunStore);
  const scene = useStore(sceneStore);
  const solver = useStore(solverStore);
  const device = useStore(deviceStore);
  const active = useStore(runStore) !== null;
  const why = addRefusal(scene?.info ?? null, solver, device, jobs);
  const waiting = jobs.filter((j) => j.status === 'waiting').length;
  return (
    <div className="sim-jobs" data-part="job-list" data-running={running ? 'true' : 'false'}>
      {jobs.length === 0 ? (
        <div className="empty" data-part="job-list-empty">
          No jobs. Add saved projects, each with the solver chosen now, and run them one after another.
        </div>
      ) : (
        <ol className="sim-job-rows">
          {jobs.map((j) => (
            <li key={j.id} className="sim-job" data-job={j.id} data-job-status={j.status}>
              <span className="name" title={j.path}>
                {j.name}
              </span>
              <span className="solver mono">
                {j.solver.toUpperCase()}
                {j.solver === 'spps' ? ` ${j.device.toUpperCase()}` : ''}
              </span>
              <span className={`state ${j.status === 'OK' ? 'ok' : j.status === 'waiting' || j.status === 'running' ? '' : 'bad'}`} title={j.note ?? j.run ?? undefined}>
                {jobStatusText(j)}
              </span>
              {j.status === 'waiting' && !running ? (
                <button className="sim-job-remove" data-action="job-remove" aria-label={`Take ${j.name} off the job list`} onClick={() => actions.removeJob(j.id)}>
                  ×
                </button>
              ) : (
                <span className="sim-job-remove" />
              )}
              {j.note && (
                <span className="note" data-part="job-note">
                  {j.note}
                </span>
              )}
            </li>
          ))}
        </ol>
      )}
      <div className="sim-job-buttons">
        <button className="small-button" data-action="job-add" disabled={why !== null || running} title={why ?? 'Add the open project, as saved, with the solver chosen now'} onClick={() => actions.addJob()}>
          Add this project
        </button>
        {running ? (
          <button className="small-button" data-action="job-stop" disabled={stopping} title="Cancel the job running and start no other; the ones not reached keep waiting" onClick={() => actions.stopJobList()}>
            {stopping ? 'Stopping…' : 'Stop'}
          </button>
        ) : (
          <button
            className="small-button"
            data-action="job-run"
            disabled={waiting === 0 || active}
            title={waiting === 0 ? 'Add a job first' : active ? 'A run is active: wait for it or cancel it' : `Run the ${waiting} waiting ${waiting === 1 ? 'job' : 'jobs'} one after another`}
            onClick={() => actions.fire(actions.runJobList())}
          >
            Run job list
          </button>
        )}
        <button className="small-button" data-action="job-clear" disabled={running || jobs.length === 0} onClick={() => actions.clearJobs()}>
          Clear
        </button>
      </div>
    </div>
  );
}
