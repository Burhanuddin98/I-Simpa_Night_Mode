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

## Both cracks re-derived (2026-09-24 08:00, workflow `wf_0b309fb2-d17`, agent "cracks")

Both cracks are errors in how the synthesis was worded, and neither changes its conclusions.
Scripts and outputs are in `target/agents/t3-cracks-scratch/`.

- **Crack 1: `self_intersections` is right, and it matches TetGen 1.5.0 exactly.**
  - The `-d` log's 93 hits are **20 distinct facet pairs**, the same set as our check's
    (`map_pairs.out`: equal sets, none on either side only). The welded arm also gives 20, and
    the arm without the box bottom gives 14 on both sides.
  - Why 93: `interecursive()` tests a pair once in every leaf box both triangles fall into, and
    counts every hit (`tetgen.cxx:14373-14531`, printed at `:14574`).
  - Why the exit codes differ:
    - `-d` always exits 0; its `diagnose` branch returns without `terminatetetgen`
      (`tetgen.cxx:30893-30908`). So never gate on `-d`'s exit code.
    - The exit 3 belongs to the meshing run with `-pq2 -A -n`.
  - The synthesis merged the two runs into one sentence (`synthesise.md:4`, `:61`).
  - After upstream's repair, TetGen 1.5.0 finds no intersecting faces and meshes to the stored
    `.1.ele`. Our check also finds 0 pairs and 5 cells there: 106.895, 93.445, 755.648, 4.352
    and 18.0 m³.
- **Crack 2: the 2 region lines are upstream's ids and seeds, now derived from source.**
  - Upstream's ids and seeds:
    - The id is each active zone's `wxid` (1930, 2083; `Objet3D_maillage.cpp:932` at `fa978fb`,
      `element.cpp:232-235`).
    - The Model zone's seed is its `volpos`.
    - The box's seed is `hc − (hc − ba)·1e-4f`, taken from the raw, unsorted corners
      (`encombrement_cuboide.h:336-338`).
    - The volume constraint is −1.
  - With those two lines, our Part-5 input goes through `preprocess.exe` to the stored
    `scene_mesh.poly` byte for byte: 4,889 B, sha256 `74b8f831…`.
  - The only thing the source leaves open is the order of the lines, which is a wx hash map's
    iteration order. The stored file has 1930 first.
  - One caveat: "our input" here is the lens's Python model of our writer, not our Rust writer's
    own bytes. The tutorial 3 piece measures the real writer.

## Settled since (2026-09-24, the tutorial-3 follow-ups, piece B)

Two of the synthesis's UNKNOWNs are now measured, in committed tests the parity gate runs; the
findings are in `docs/upstream-findings.md` (internal):
- **Whether upstream's shipped 1.3.4 and 1.4.0 `spps.exe` loop the same way on tutorial 3:**
  they do. On the parity mesh (upstream's `.mbin`), the stored run with transmission on and seed
  1, 1.4.0 writes every file as ours (20.12 % of records lost to loops), and 1.3.4 loses 20.13 %
  (`parity_tutorials.rs::shipped_solvers_on_tutorial_3_parity_mesh`).
- **The effect on receiver levels:** at the stored configuration, 3 seeds on each mesh, the
  default mesh (markers restored) and the parity mesh agree within 0.12 dB at every receiver;
  only Receiver 3's difference is resolved by the seeds
  (`parity_tutorials.rs::tutorial_3_receiver_levels_default_against_parity`).
- The loss to loops itself, and the run manager's refusal of the parity mesh, are
  `tutorial_3_loops_parity_against_default` (`docs/m5-m6-design.md`, decision 12).
