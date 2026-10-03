// No UI string says "validated" (MQ2, decision-log row 39: the public wording is "checked against
// reference solutions to within the just-noticeable difference", never "validated"). The M12 assay
// found "EDT ... marked not validated" in the Simulate step's hint, outside the Acoustics tab
// that m12-a's page check reads; this test holds every string the UI's source can put on a
// screen, on every step, to the same rule.
//
// What is read, under ui/src and ui/index.html: every string literal, template text and JSX text
// and attribute of the .ts/.tsx/.mjs sources (the *.test.ts suites are not UI), and the text of
// the .css files and the page; comments are not read. A snake_case token (`edt_validated`,
// `validated_by_bed`) is a JSON field's name, not a word on screen, and is removed before the
// search. The JSON files under src (the IPC schemas) are imported by no UI module and are not
// read.
import { strict as assert } from 'node:assert';
import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { test } from 'node:test';
import { fileURLToPath } from 'node:url';
import ts from 'typescript';

const SRC = path.dirname(fileURLToPath(import.meta.url));
const WORD = /validated/i;

/** A snake_case field name is not a word on screen. */
const words = (s: string) => s.replace(/[A-Za-z0-9]*_[A-Za-z0-9_]*/g, ' ');

/** Every string a script can show (literals, template text, JSX text), with its line. */
export function scriptStrings(file: string, text: string): { line: number; text: string }[] {
  const kind = file.endsWith('.tsx') ? ts.ScriptKind.TSX : file.endsWith('.mjs') ? ts.ScriptKind.JS : ts.ScriptKind.TS;
  const sf = ts.createSourceFile(file, text, ts.ScriptTarget.Latest, true, kind);
  const out: { line: number; text: string }[] = [];
  const visit = (n: ts.Node) => {
    if (ts.isStringLiteral(n) || ts.isNoSubstitutionTemplateLiteral(n) || ts.isTemplateHead(n) || ts.isTemplateMiddle(n) || ts.isTemplateTail(n) || ts.isJsxText(n)) {
      out.push({ line: sf.getLineAndCharacterOfPosition(n.getStart(sf)).line + 1, text: n.text });
    }
    ts.forEachChild(n, visit);
  };
  visit(sf);
  return out;
}

/** The hits of the word in one file's shown strings. */
export function hits(file: string, text: string): string[] {
  if (/\.(tsx?|mjs)$/.test(file)) return scriptStrings(file, text).filter((s) => WORD.test(words(s.text))).map((s) => `${file}:${s.line}: ${JSON.stringify(s.text)}`);
  const bare = text.replace(/\/\*[\s\S]*?\*\//g, '').replace(/<!--[\s\S]*?-->/g, '');
  return bare
    .split('\n')
    .map((l, i) => ({ l, i }))
    .filter(({ l }) => WORD.test(words(l)))
    .map(({ l, i }) => `${file}:${i + 1}: ${l.trim()}`);
}

function uiFiles(dir: string): string[] {
  const out: string[] = [];
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) out.push(...uiFiles(p));
    else if (/\.(tsx?|mjs|css)$/.test(e.name) && !/\.test\.ts$/.test(e.name)) out.push(p);
  }
  return out;
}

test('wording: no string the UI can show says "validated", on any step', () => {
  const files = [...uiFiles(SRC), path.join(SRC, '..', 'index.html')];
  assert.ok(files.length > 50, `only ${files.length} UI files found under ${SRC}`);
  const found = files.flatMap((f) => hits(path.relative(SRC, f), readFileSync(f, 'utf8')));
  assert.deepEqual(found, []);
});

test('wording: the search says no (a shown string), and yes to comments and field names', () => {
  // Say-NO: a hint, a JSX text, a template and a CSS content string are each found.
  assert.equal(hits('a.ts', 'const h = "EDT is marked not validated";').length, 1);
  assert.equal(hits('a.tsx', 'const e = <div className="x">Not Validated here</div>;').length, 1);
  assert.equal(hits('a.ts', 'const t = `${n} validated`;').length, 1);
  assert.equal(hits('a.css', '.x::after { content: "validated"; }').length, 1);
  // Not words on screen: comments, and JSON field names in a path.
  assert.equal(hits('a.ts', '// not validated\n/** validated */\nconst p = `${x}.edt_validated`;').length, 0);
  assert.equal(hits('a.ts', "const k = 'validated_by_bed';").length, 0);
  assert.equal(hits('a.css', '/* validated */ .x {}').length, 0);
});
