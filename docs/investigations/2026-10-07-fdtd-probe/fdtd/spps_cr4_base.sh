#!/bin/bash
# CR4 on the BASE room (no "Absorbing panels" variant), the room BRAS measured. Waits for spps_s1.sh first.
until grep -q "=== DONE" /c/tmp/nm-fdtd-probe/spps_s1.log; do sleep 5; done
cd "/c/repos/Room-Acoustics-Engine/I-Simpa FDTD"
export SIMPA_SOLVERS_DIR='C:\tmp\nm-verified\I-Simpa-Night-Mode-2026-10-07-2e6345a\solvers'
for spec in "base app/src-tauri/examples/bras_cr4.simpa" "base_s1 C:/tmp/nm-spps-projects/bras_cr4_s1.simpa"; do
  set -- $spec
  echo "=== cr4 $1 $(date +%H:%M)"
  ./target/release/simpa.exe run "$2" --solver spps --device gpu --base --runs "C:/tmp/nm-spps-runs/cr4_$1" 2>&1 | grep -E "^OK - |^FAIL|^simpa: [^S]"
  d=$(ls -d /c/tmp/nm-spps-runs/cr4_$1/*/ | tail -1)
  ./target/release/simpa.exe results "$(cygpath -w "$d")" --json > "/c/tmp/nm-spps-runs/cr4_$1.json"; echo "results exit $?"
done
echo "=== DONE $(date +%H:%M)"
