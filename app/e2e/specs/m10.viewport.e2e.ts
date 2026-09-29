// The viewport package's gate ids (PLAN.md 3 and 6.1). Shell from the M10 foundation: each id
// fails as pending until the package builds it, so nothing passes by being absent. The package
// owns this file from here.
//   m10-a-highlight  raw hall: __m10.highlightedFaceCount() > 0 and the chip "FAIL · n faces"
//                    visible; teaching room: 0
//   m10-d            the box: double-click the ceiling (aimAtFace) selects faces [10, 11], group
//                    Ceiling; a wall double-click selects 2 faces, not the group's 8
//   m10-g            every step, both view tabs, all dock tabs, two models: one canvas
const PENDING = 'PENDING: the viewport package builds this check (PLAN.md 6.1)';

describe('M10 viewport', () => {
  it('m10-a-highlight: the refused faces of the raw hall are highlighted', () => {
    throw new Error(PENDING);
  });
  it('m10-d: a double-click on the box ceiling selects its 2 faces, group Ceiling', () => {
    throw new Error(PENDING);
  });
  it('m10-g: document.querySelectorAll("canvas").length == 1 throughout', () => {
    throw new Error(PENDING);
  });
});
