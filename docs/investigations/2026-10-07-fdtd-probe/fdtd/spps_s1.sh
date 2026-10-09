#!/bin/bash
cd "/c/repos/Room-Acoustics-Engine/I-Simpa FDTD"
export SIMPA_SOLVERS_DIR='C:\tmp\nm-verified\I-Simpa-Night-Mode-2026-10-07-2e6345a\solvers'
for r in cr2 cr4; do
  echo "=== $r s1 $(date +%H:%M)"
  ./target/release/simpa.exe run "C:/tmp/nm-spps-projects/bras_${r}_s1.simpa" --solver spps --device gpu --runs "C:/tmp/nm-spps-runs/${r}_s1" 2>&1 | grep -E "^OK - |^FAIL|^simpa: [^S]"
  d=$(ls -d /c/tmp/nm-spps-runs/${r}_s1/*/ | tail -1)
  ./target/release/simpa.exe results "$(cygpath -w "$d")" --json > "/c/tmp/nm-spps-runs/${r}_s1.json"; echo "$r results exit $?"
done
echo "=== DONE $(date +%H:%M)"
