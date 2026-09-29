// The materials package's gate id (PLAN.md 3 and 6.2). Shell from the M10 foundation: it fails
// as pending until the package builds it, so nothing passes by being absent. The package owns
// this file from here.
//   m10-c  teaching room, Materials step, focus [data-grid-cell="0:0"], a synthetic paste of
//          tests/fixtures/ui/materials_6x6.tsv (bytes as read by Node), saveAs: the saved bytes
//          equal tests/fixtures/ui/materials_6x6.expected.json; the committed start file differs
//          from it (asserted first)
const PENDING = 'PENDING: the materials package builds this check (PLAN.md 6.2)';

describe('M10 materials', () => {
  it('m10-c: the pasted 6x6 block saves byte-identical to materials_6x6.expected.json', () => {
    throw new Error(PENDING);
  });
});
