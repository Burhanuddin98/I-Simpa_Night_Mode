// The scene package's gate ids (PLAN.md 3 and 6.3). Shell from the M10 foundation: each id
// fails as pending until the package builds it, so nothing passes by being absent. The package
// owns this file from here.
//   m10-a-run        raw hall: [data-part=run] disabled, data-blockers holds GEOMETRY_REFUSED;
//                    teaching room: data-blockers is exactly M11_PENDING
//   m10-b-materials  corrected hall: [data-step="materials"] [data-part="sub"] reads "0 / 10";
//                    tutorial1_box.simpa reads "3 / 3"
//   m10-e-outside    R1's position.x = 20 + Enter: [data-issue-code="RECEIVER_OUTSIDE"] shown,
//                    projectJson() and undoDepth() unchanged; x = 4 accepted
//   m10-e-label      R1's name "a/b" + Enter: [data-issue-code="LABEL_UNSAFE"] shown, project
//                    unchanged; "Front row" accepted
const PENDING = 'PENDING: the scene package builds this check (PLAN.md 6.3)';

describe('M10 scene', () => {
  it('m10-a-run: Run is disabled, and why is named', () => {
    throw new Error(PENDING);
  });
  it("m10-b-materials: the step bar reads Materials '0 / 10' on the corrected hall", () => {
    throw new Error(PENDING);
  });
  it('m10-e-outside: a receiver placed outside shows RECEIVER_OUTSIDE and changes nothing', () => {
    throw new Error(PENDING);
  });
  it("m10-e-label: the label 'a/b' is refused with LABEL_UNSAFE", () => {
    throw new Error(PENDING);
  });
});
