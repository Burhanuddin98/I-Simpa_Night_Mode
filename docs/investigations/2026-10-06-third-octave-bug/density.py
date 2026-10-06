"""How sparse the stored sound maps are in the real 05:13 CR4-third run (300,000 particles a source, 18 bands)."""
import os
import subprocess

SIMPA = r"C:\tmp\nm-target\release\simpa.exe"
RUN = r"B:\repos\I-Simpa_Night_Mode\.out\ui\cr4-third\runs\20261006-051350-113-spps\solve\Surface receiver"

for band in ["100 Hz", "500 Hz", "1000 Hz", "5000 Hz", "Global"]:
    p = os.path.join(RUN, band, "rs_cut.csbin")
    if not os.path.isfile(p):
        continue
    out = subprocess.run([SIMPA, "dump", "csbin", p], capture_output=True, text=True).stdout
    steps = faces = recs = 0
    hit_faces = 0
    for line in out.splitlines():
        if line.startswith("timesteps "):
            steps = int(line.split()[1])
        elif line.startswith("face "):
            faces += 1
        elif line.startswith("records "):
            n = int(line.split()[1])
            recs += n
            if n:
                hit_faces += 1
    cells = faces // 2
    slots = cells * steps
    dense_mb = slots * 4 / 2**20
    print(f"{band:8s} cells {cells:,} steps {steps:,} slots {slots:,} | records {recs:,} ({100*recs/max(1,2*slots):.1f}% of face-steps) "
          f"| faces with any energy {hit_faces:,} of {faces:,} | dense RAM {dense_mb:,.0f} MB | sparse 8 B a record {recs*8/2**20:,.0f} MB | file {os.path.getsize(p)/2**20:,.0f} MB")
