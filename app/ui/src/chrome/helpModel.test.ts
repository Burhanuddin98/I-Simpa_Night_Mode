// node --test suite for the Help menu's model (helpModel.ts): every topic the menu sends is one the
// core opens (src-tauri/src/help.rs), and no web address is written on the UI's side.
import { strict as assert } from 'node:assert';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { HELP_LINKS, helpLink } from './helpModel.ts';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const HELP_RS = path.join(HERE, '..', '..', '..', 'src-tauri', 'src', 'help.rs');

test('help: the menu’s web topics are exactly the core’s LINKS, in order', () => {
  const rs = readFileSync(HELP_RS, 'utf8');
  const links = rs.slice(rs.indexOf('pub const LINKS'), rs.indexOf('];', rs.indexOf('pub const LINKS')));
  const ids = [...links.matchAll(/\(\s*"([a-z-]+)",\s*"https:\/\/[^"]+",?\s*\)/g)].map((m) => m[1]);
  assert.deepEqual(ids, HELP_LINKS.map((l) => l.topic));
});

test('help: ids and labels are unique, and no address is written here', () => {
  assert.equal(new Set(HELP_LINKS.map((l) => l.id)).size, HELP_LINKS.length);
  assert.equal(new Set(HELP_LINKS.map((l) => l.label)).size, HELP_LINKS.length);
  const ts = readFileSync(path.join(HERE, 'helpModel.ts'), 'utf8');
  assert.doesNotMatch(ts, /https?:\/\//);
  assert.equal(helpLink('source').label, 'Night Mode source code');
  assert.throws(() => helpLink('nope' as never));
});
