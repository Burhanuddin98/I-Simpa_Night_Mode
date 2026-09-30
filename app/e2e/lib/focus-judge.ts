// The judge's command line, for tools/gates/m11.ps1 (PLAN.md 4.4 step 8):
//   node app/e2e/lib/focus-judge.ts <focus.jsonl> [--min-sessions <n>] [--expect-steal <title>]
// Prints the verdict: every failure, every excused change, the notes. Exits 0 when the log
// passes, 1 when it fails, 2 on a usage error. With --expect-steal (the -FocusSayNo live proof)
// the sense is inverted: it exits 0 only when the judge failed, and on a foreground change to a
// window with that title, so a watcher that saw nothing cannot pass it.
import { readFileSync } from 'node:fs';
import { judge, parseLog } from './focus.ts';

const args = process.argv.slice(2);
const file = args[0];
const opt = (name: string): string | undefined => {
  const i = args.indexOf(name);
  return i > 0 ? args[i + 1] : undefined;
};
if (!file || file.startsWith('--')) {
  console.error('usage: node focus-judge.ts <focus.jsonl> [--min-sessions <n>] [--expect-steal <title>]');
  process.exit(2);
}
const min = Number(opt('--min-sessions') ?? '0');
const expectSteal = opt('--expect-steal');
const v = judge(parseLog(readFileSync(file, 'utf8')), { minSessions: min });
for (const n of v.notes) console.log(`note     ${n}`);
for (const e of v.excused) console.log(`excused  ${e}`);
for (const f of v.failures) console.log(`FAIL     ${f}`);
if (expectSteal !== undefined) {
  const caught = v.failures.some((f) => f.startsWith('FOCUS TAKEN:') && f.includes(`'${expectSteal}'`));
  console.log(caught ? `say-NO   the judge flagged '${expectSteal}' taking the foreground` : `say-NO   the judge did NOT flag '${expectSteal}'`);
  process.exit(caught ? 0 : 1);
}
console.log(v.ok ? 'm11-focus: PASS' : `m11-focus: FAIL (${v.failures.length})`);
process.exit(v.ok ? 0 : 1);
