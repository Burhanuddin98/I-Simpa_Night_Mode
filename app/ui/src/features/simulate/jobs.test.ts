// C40: the job list's rules (jobs.ts).
import { strict as assert } from 'node:assert';
import { test } from 'node:test';
import { addRefusal, type Job, jobsSummary, jobStatusText, nextJob, samePath, withJob } from './jobs.ts';

const job = (id: number, status: Job['status'], path = `C:\\p\\${id}.simpa`): Job => ({ id, path, name: `${id}`, solver: 'spps', device: 'cpu', status });

test('addRefusal: only a saved project with no unsaved changes, once per file, solver and device', () => {
  assert.match(addRefusal(null, 'spps', 'cpu', [])!, /Open a project/);
  assert.match(addRefusal({ path: null, dirty: true }, 'spps', 'cpu', [])!, /^Save the project first/);
  assert.match(addRefusal({ path: 'C:\\p\\1.simpa', dirty: true }, 'spps', 'cpu', [])!, /^Save the changes first/);
  assert.equal(addRefusal({ path: 'C:\\p\\1.simpa', dirty: false }, 'spps', 'cpu', []), null);
  const list = [job(1, 'waiting')];
  assert.match(addRefusal({ path: 'c:/P/1.SIMPA', dirty: false }, 'spps', 'cpu', list)!, /already wait/);
  assert.equal(addRefusal({ path: 'C:\\p\\1.simpa', dirty: false }, 'spps', 'gpu', list), null, 'another device');
  assert.equal(addRefusal({ path: 'C:\\p\\1.simpa', dirty: false }, 'tcr', 'gpu', list), null, 'another solver');
  assert.equal(addRefusal({ path: 'C:\\p\\1.simpa', dirty: false }, 'spps', 'cpu', [job(1, 'OK')]), null, 'run already: again is a new job');
});

test('nextJob, withJob: the first waiting one; a patch changes only its job', () => {
  const list = [job(1, 'OK'), job(2, 'refused'), job(3, 'waiting'), job(4, 'waiting')];
  assert.equal(nextJob(list)?.id, 3);
  assert.equal(nextJob([job(1, 'OK')]), null);
  const next = withJob(list, 3, { status: 'running', run: 'r' });
  assert.deepEqual(next.map((j) => j.status), ['OK', 'refused', 'running', 'waiting']);
  assert.equal(next[2].run, 'r');
  assert.equal(list[2].status, 'waiting', 'the list given is not changed');
});

test('samePath, jobStatusText, jobsSummary', () => {
  assert.equal(samePath('C:\\a\\b.simpa', 'c:/A/B.simpa'), true);
  assert.equal(samePath(null, 'x'), false);
  assert.equal(jobStatusText(job(1, 'CANCELLED')), 'Cancelled');
  assert.equal(jobsSummary([job(1, 'OK'), job(2, 'FAIL'), job(3, 'refused'), job(4, 'waiting')]), '2 jobs run: 1 OK, 1 not OK; 1 not run; 1 still waiting');
  assert.equal(jobsSummary([job(1, 'OK')]), '1 job run: 1 OK');
});
