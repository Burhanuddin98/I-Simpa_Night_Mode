# Tutorial 3 (the industrial hall): why our pipeline refuses it (2026-09-24)

This investigation was run at Burhan's request ("i think we should investigate tutorial 3"). It
had four lenses (scene, preprocess, refusal-validity, fittings), a synthesis and a challenge.

**Read `synthesise.md` first, then `challenge.md`.** The challenge found two cracks:
- **The claim that "TetGen `-d` lists exactly our 20 pairs" is wrong.** The log shows 93 pair
  reports and exit 0, so the "self_intersections is right" verdict must be re-derived from the
  unique pair set.
- **"Our preprocess reproduces upstream's `.poly` byte for byte" is overstated.** Our Part-5
  variant still differs in its 2 region lines.

The scratch evidence is in `target/investigate/tutorial3/`, which is gitignored.
