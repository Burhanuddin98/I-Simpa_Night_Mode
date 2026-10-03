# KEEL brief: scope row 15 piece (1), G19 + M37 (2026-10-03, rebuild @ 1a76770)

PRIOR
- G19: parity-matrix.md:133,400 - Deferred, M, "Before v1", no regroup op (ops.rs:124-127 SetGeometry only); G20 (:134,451) and G30 (:144) reuse the op.
- M37: parity-matrix.md:204,448 - Absent, import side before v1: "Flatten them, or keep a group path as sources do". GUI groups and M39 are v1.x.
- decision-log.md:33 (row 21, 2026-09-29 09:03) v1 scope; :34 (row 22, 2026-09-29 11:59) gives G19 + M37 their own piece, outside M11. Ruling by Jarvis, not Burhan.
- Built? No. git log has no commit for either; scope.md:15 says "(1) not started". proj.rs:921-927 still returns ImportError::unsupported for any non-`recepteurp` child of `recepteursp` (a group). Matrix cited :910-916; the line has moved.
- Sources already keep a group path (proj.rs:1051 source_elements, Source::group at ~:906) - the model for M37. Point receivers have no group field.
- extract_upstream_scene.py:29-41 is a RELATED but different problem: the corrected mesh in the .proj has one group "model", so the layer split is lost; solved by a nearest-original-face-centroid heuristic from the layered .ply. It is a workaround for lost data, not a regroup op; it proves why G19 matters (a layer mixing materials is stuck).
- proj.rs import itself maps material groups from .finfo (proj.rs:40-52), so a .proj import does keep face groups; only mesh-group "model" collapse is the issue.

PROBE
- Upstream ships 10 .proj under I-Simpa-upstream/src/isimpa/resources/doc (tutorials 1-3, Industrial, validation). Parse each: list children tags of `recepteursp` and count distinct .finfo material groups vs mesh groups. Any nested group => our importer refuses today. Some files failed plain XML parse (token error at line 1): check whether they are zipped/other before trusting a count. Then run our import-proj on each and record refuse/accept.
- Not run to completion here (budget).

LOAD-BEARING
- M37: flatten vs keep a group path. Flatten silently loses which group a receiver belonged to; if the group drives any selection (M39 rotate-all, or a run's receiver set) a wrong set enters a run unannounced. Keeping Source::group's shape is the safe analogue.
- G19: the regroup op must be invertible and must move the material with the face set; a face landing in a group with another material silently takes that material. Surface-group key is (material group, receiver, zones) per proj.rs:45 - regroup must keep receivers/zones coherent or the "face listed by two" refusal fires.

RULED-NOT-BUILT
- decision-log row 22 (:34, 2026-09-29): G19 + M37 own piece - unbuilt. Row 21 (:33) puts them in v1. G20 / M39 are v1.x, need the same op / M37.

TRAPS
- Matrix line refs stale (proj.rs:910-916 now ~921). Row 22 is Jarvis's call, not Burhan's. Do not copy the centroid heuristic as a regroup op. Corrected mesh has one "model" group - test on testdata/elmia_corrected.ply.
