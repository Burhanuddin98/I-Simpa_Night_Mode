#!/bin/bash
# Phase 3: chamber closure check, then Test B (chamber-calibrated boundary) on CR2, CR1, CR3, CR4. Sequential.
cd /c/tmp/nm-fdtd-probe
PY=./.venv/Scripts/python
echo "=== closure $(date +%H:%M)"; VC_MAP=curve $PY vchamber.py run 2>&1 | grep -E "^\[|SystemExit"; VC_MAP=curve $PY vchamber.py analyze
for spec in "CR2 1000 2.5" "CR1 400 8.0" "CR3 400 5.0" "CR4 400 4.0"; do
  set -- $spec
  echo "=== $1 curve $(date +%H:%M)"
  PROBE_BC=curve PROBE_ROOM=$1 PROBE_FMAX=$2 PROBE_DUR=$3 $PY probe.py run LS1 LS2 2>&1 | grep -E "^\[LS|SystemExit"
  PROBE_BC=curve PROBE_ROOM=$1 PROBE_FMAX=$2 PROBE_DUR=$3 $PY probe.py analyze 2>&1 | grep -v -i wavfile
done
echo "=== CHAIN DONE $(date +%H:%M)"
