@echo off
rem A stand-in for tetgen.exe, for the M11 gate's mesh-failure run (gate (e)) only:
rem it plays TetGen 1.6.0 skipping two self-intersecting facets. The verified
rem TetGen 1.5.0 never writes this file (docs/investigations/2026-09-29-m11/PLAN.md, 5).
(echo 2 1& echo 1 1 2 3 8& echo 2 1 3 4 9)> scene_mesh_skipped.face
exit /b 3
