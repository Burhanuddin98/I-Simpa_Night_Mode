"""spps-gpu bed: the refusals A2 specifies, each on a staged copy of the seats box, and --probe.
Prints exit code and stderr per case.

usage: python refusals.py <root>
"""
import json, os, re, struct, subprocess, sys
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import stage  # noqa: E402

EXE = r"C:\tmp\nm-spps-gpu\bin\spps-gpu.exe"
REPO = os.path.abspath(os.path.join(HERE, "..", "..", ".."))
SRC = os.path.join(REPO, "tests", "fixtures", "results", "seats_spps", "solve")


def volume_ids(mbin):
    b = open(mbin, "rb").read()
    T, N = struct.unpack_from("<II", b, 0)
    o = 8 + 12 * N
    return sorted({struct.unpack_from("<i", b, o + 100 * t + 16)[0] for t in range(T)})


def fitting(cfg_path, vid):
    s = open(cfg_path, encoding="utf-8").read()
    enc = ('<encombrement_enum><encombrement id="%d"><bfreq freq="500" alpha="0.1" lambda="2" loi_diff="0"/>'
           '<bfreq freq="1000" alpha="0.1" lambda="2" loi_diff="0"/></encombrement></encombrement_enum>' % vid)
    s = re.sub(r"<encombrement_enum\s*/>|<encombrement_enum>.*?</encombrement_enum>", enc, s, flags=re.S)
    open(cfg_path, "w", encoding="utf-8").write(s)


if __name__ == "__main__":
    root = sys.argv[1]
    cases = {
        "stratified": {"atmo.alog": "1"},
        "balloon": {"src.directivite": "5", "src.u": "1", "src.v": "0", "src.w": "0", "src.directivity_file": "speaker.txt"},
        "fittings": {},
        "fittings-off": {"enc_calc": "0"},
    }
    for name, attrs in cases.items():
        d = os.path.join(root, name)
        stage.stage(SRC, d, dict(attrs), template=False)
        if name.startswith("fittings"):
            vids = [v for v in volume_ids(os.path.join(d, "tetramesh.mbin")) if v != 0]
            fitting(os.path.join(d, "config.xml"), vids[0] if vids else 1)
        rc = stage.run(EXE, d, [])
        err = open(os.path.join(d, "solver.stderr.txt"), encoding="utf-8", errors="replace").read().strip()
        print(json.dumps({"case": name, "exit": rc, "stderr": err[:400]}))
    p = subprocess.run([EXE, "--probe"], capture_output=True, text=True)
    print(json.dumps({"case": "--probe", "exit": p.returncode, "stdout": p.stdout.strip()}))
    p = subprocess.run([EXE, "--probe"], capture_output=True, text=True, env={**os.environ, "CUDA_VISIBLE_DEVICES": "-1"})
    print(json.dumps({"case": "--probe, CUDA_VISIBLE_DEVICES=-1", "exit": p.returncode, "stdout": p.stdout.strip()}))
