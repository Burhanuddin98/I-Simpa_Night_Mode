// `npm test`: the TypeScript checksum (ui/src/checksum.ts, loaded by node's type stripping) against
// the known answers bench.rs is also held to (ui/src/checksum-kat.json). Exits 1 on any mismatch.
import { readFileSync } from 'node:fs';
import { knownAnswers } from '../src/checksum.ts';

const kat = JSON.parse(readFileSync(new URL('../src/checksum-kat.json', import.meta.url), 'utf8'));
const results = knownAnswers(kat.cases);
for (const r of results) {
  console.log(`${r.ok ? 'ok  ' : 'FAIL'} ${r.name} (${r.bytes} B): got ${r.got}, want ${r.want}`);
}
const failed = results.filter((r) => !r.ok).length;
console.log(`checksum known answers: ${results.length - failed} of ${results.length} passed`);
// An empty or truncated case list must not pass quietly.
if (failed > 0 || results.length < 6) process.exit(1);
