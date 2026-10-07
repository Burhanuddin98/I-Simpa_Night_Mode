"""spps-gpu bed (b, c): run the cases for the arms and seeds given, one after the other, logging one
JSON line per run to <OUT>/bed-<tag>.jsonl.

usage: python run_bed.py <tag> <arms comma-separated> <seeds comma-separated> [case ...]
"""
import json, os, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
FIX = os.path.join(REPO, "tests", "fixtures", "results")
CR4 = r"B:\repos\I-Simpa_Night_Mode\.out\ui\cr4-27\runs\20261006-193146-930-spps\solve"
OUT = os.environ.get("BED_OUT", r"B:\repos\I-Simpa_Night_Mode\.out\spps-gpu\bed")   # A6: .out\a6\a2bed

# case: (source solve folder, simulation attributes). Particle counts raised above the fixtures' own
# (2,000 and 50,000) so the parameters leave the noise model's "uncalibrated" region; equal in both arms.
CASES = {
    "seats": (os.path.join(FIX, "seats_spps", "solve"), {"nbparticules": "200000"}),
    "seats3s": (os.path.join(FIX, "seats_spps", "solve"), {"nbparticules": "200000", "duree_simulation": "3"}),
    "energetic": (os.path.join(FIX, "energetic_spps", "solve"), {"nbparticules": "200000"}),
    "outputs": (os.path.join(FIX, "outputs_spps", "solve"), {"nbparticules": "200000"}),
    "sources2": (os.path.join(FIX, "sources2_spps", "solve"), {"nbparticules": "200000"}),
    "cr4-1k": (CR4, {"only_band": "1000"}),
    # rule variants on the energetic box (energetic mode, 1 s): reflection laws, direct field only,
    # source types, surface map in "SPL" mode, single-sided materials, no air absorption; and the
    # random mode with diffusion on the seats box
    "v-lambert": (os.path.join(FIX, "energetic_spps", "solve"), {"nbparticules": "200000", "mat.diffusion": "1", "mat.loi": "2"}),
    "v-uniform": (os.path.join(FIX, "energetic_spps", "solve"), {"nbparticules": "200000", "mat.diffusion": "0.5", "mat.loi": "1"}),
    "v-w2": (os.path.join(FIX, "energetic_spps", "solve"), {"nbparticules": "200000", "mat.diffusion": "0.7", "mat.loi": "3"}),
    "v-w4": (os.path.join(FIX, "energetic_spps", "solve"), {"nbparticules": "200000", "mat.diffusion": "1", "mat.loi": "5"}),
    "v-direct": (os.path.join(FIX, "energetic_spps", "solve"), {"nbparticules": "200000", "direct_calc": "1"}),
    "v-srcxy": (os.path.join(FIX, "energetic_spps", "solve"), {"nbparticules": "200000", "src.directivite": "2"}),
    "v-srcyz": (os.path.join(FIX, "energetic_spps", "solve"), {"nbparticules": "200000", "src.directivite": "3"}),
    "v-srcuni": (os.path.join(FIX, "energetic_spps", "solve"), {"nbparticules": "200000", "src.directivite": "1", "src.u": "0", "src.v": "1", "src.w": "0.2"}),
    "v-surfspl": (os.path.join(FIX, "energetic_spps", "solve"), {"nbparticules": "200000", "surf_receiv_method": "1"}),
    "v-onesided": (os.path.join(FIX, "energetic_spps", "solve"), {"nbparticules": "200000", "side.material": "0"}),
    "v-noatmo": (os.path.join(FIX, "energetic_spps", "solve"), {"nbparticules": "200000", "abs_atmo_calc": "0"}),
    "v-random-diffuse": (os.path.join(FIX, "seats_spps", "solve"), {"nbparticules": "200000", "duree_simulation": "3", "mat.diffusion": "0.6", "mat.loi": "2"}),
}

if __name__ == "__main__":
    if len(sys.argv) < 4:
        sys.exit(__doc__)
    tag, arms, seeds = sys.argv[1], sys.argv[2].split(","), [int(s) for s in sys.argv[3].split(",")]
    cases = sys.argv[4:] or list(CASES)
    os.makedirs(OUT, exist_ok=True)
    log = os.path.join(OUT, f"bed-{tag}.jsonl")
    for case in cases:
        src, attrs = CASES[case]
        for seed in seeds:
            for arm in arms:
                cmd = [sys.executable, os.path.join(HERE, "vs_spps.py"), case, src, arm, str(seed)] + [f"{k}={v}" for k, v in attrs.items()]
                p = subprocess.run(cmd, capture_output=True, text=True)
                lines = [l for l in p.stdout.splitlines() if l.startswith("{")]
                rec = json.loads(lines[0]) if lines else {"case": case, "arm": arm, "seed": seed, "error": p.stdout[-1500:] + p.stderr[-1500:]}
                with open(log, "a", encoding="utf-8") as fh:
                    fh.write(json.dumps(rec) + "\n")
                print(json.dumps(rec), flush=True)
