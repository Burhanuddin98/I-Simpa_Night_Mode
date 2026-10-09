// node --test suite for the Help menu's model (helpModel.ts): every topic the menu sends is one the
// core opens (src-tauri/src/help.rs), and no web address is written on the UI's side.
import { strict as assert } from 'node:assert';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import { HELP_LINKS, HELP_PAGES, helpLink, TUTORIALS } from './helpModel.ts';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const HELP_RS = path.join(HERE, '..', '..', '..', 'src-tauri', 'src', 'help.rs');

test('help: the menu’s web topics are exactly the core’s LINKS, in order', () => {
  const rs = readFileSync(HELP_RS, 'utf8');
  const links = rs.slice(rs.indexOf('pub const LINKS'), rs.indexOf('];', rs.indexOf('pub const LINKS')));
  const ids = [...links.matchAll(/\(\s*"([a-z-]+)",\s*"https:\/\/[^"]+",?\s*\)/g)].map((m) => m[1]);
  assert.deepEqual(ids, HELP_LINKS.map((l) => l.topic));
});

test('help: the pages shipped in the app are exactly the core’s PAGES, in order (A22)', () => {
  const rs = readFileSync(HELP_RS, 'utf8');
  const pages = rs.slice(rs.indexOf('pub const PAGES'), rs.indexOf('];', rs.indexOf('pub const PAGES')));
  const topics = [...pages.matchAll(/topic: "([a-z0-9-]+)"/g)].map((m) => m[1]);
  assert.deepEqual(topics, HELP_PAGES.map((l) => l.topic));
  assert.equal(HELP_PAGES[0].label, 'User manual');
});

test('help: each tutorial opens an example examples.rs ships, and its own page (A43)', () => {
  const rs = readFileSync(path.join(HERE, '..', '..', '..', 'src-tauri', 'src', 'examples.rs'), 'utf8');
  const ids = [...rs.matchAll(/id: "([^"]+)"/g)].map((m) => m[1]);
  assert.deepEqual(TUTORIALS.map((t) => t.example), ['tutorial-1', 'tutorial-2', 'tutorial-3']);
  for (const t of TUTORIALS) {
    assert.ok(ids.includes(t.example), t.example);
    assert.equal(t.topic, t.example);
  }
  assert.equal(TUTORIALS[1].label, 'Tutorial 2: the Elmia hall');
});

test('help: ids and labels are unique, and no address is written here', () => {
  const all = [...HELP_PAGES, ...HELP_LINKS];
  assert.equal(new Set(all.map((l) => l.id)).size, all.length);
  assert.equal(new Set(all.map((l) => l.label)).size, all.length);
  const ts = readFileSync(path.join(HERE, 'helpModel.ts'), 'utf8');
  assert.doesNotMatch(ts, /https?:\/\//);
  assert.equal(helpLink('source').label, 'Night Mode source code');
  assert.throws(() => helpLink('nope' as never));
});
