"""Receipt for M8b tamper round 2 at the gate (findings 1 and 4): tools/gates/m8a.ps1 -From reads an
earlier bed only against a committed seal, found under any spelling of the bed folder's name. Each
case runs the gate script and records what it printed and its exit code:

  G1  the fixed gate, -From a renamed bed folder (no seal of that name): refused before anything runs;
  G2  the fixed gate, -Seal a copy of the committed seal outside the repository: refused;
  G3  the fixed gate, -Seal a copy inside the repository that git does not track (under the
      worktree's gitignored target/): refused;
  G4  the fixed gate, -From the real M8a bed, its committed seal changed by one byte in the work tree
      (restored byte for byte after, and checked): refused;
  G5  the fixed gate, -From the real M8a bed as named: the seal is taken (stopped once it prints it);
  G6  the fixed gate, -From the real M8a bed spelled in lower case: the seal is taken, under the
      name git tracks (stopped once it prints it);
  G0  the gate before round 2 (4d79c7d's m8a.ps1, written in place and restored byte for byte
      after), -From the renamed folder: it prints "seal: (none: ...)" and goes on to E1;
  G7  the gate of round 2's finding-1 commit (0069cd2's m8a.ps1, written in place and restored),
      -From the real M8a bed spelled in lower case: refused, the seal "not tracked by git", since
      git's pathspecs match case exactly.
A case that is "stopped" has its process tree killed once the line named has printed, before the gate
builds or reads anything.

Writes the log given as argv[1]. It never reads the M8a bed: every gate run here stops before simpa
bed. Deletes nothing: the files it writes are listed at the end of the log."""

import hashlib
import os
import shutil
import subprocess
import sys
import time

WT = r"B:\repos\I-Simpa_Night_Mode\.claude\worktrees\m8b-tamper"
GATE = os.path.join(WT, "tools", "gates", "m8a.ps1")
SEAL = os.path.join(WT, "beds", "m8a-20260929T093134Z", "outputs-seal.json")
BED = r"C:\tmp\nm-m8a-bed\20260929T093134Z"
SCRATCH = r"C:\tmp\nm-judge-tamper\r2\gate"
ENV = dict(
    os.environ,
    CARGO_TARGET_DIR="C:/tmp/nm-target",
    CARGO_BUILD_JOBS="16",
    CARGO_INCREMENTAL="0",
    SIMPA_SOLVERS_DIR="C:/tmp/nm-m8a-solvers",
)


def sha(path):
    return hashlib.sha256(open(path, "rb").read()).hexdigest()


def gate(args, stop_after=None, limit=180):
    """Runs the gate with args; with stop_after, kills its process tree once a line containing it
    has printed. Returns (exit code or 'stopped', output, seconds)."""
    cmd = ["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", GATE] + args
    t0 = time.time()
    p = subprocess.Popen(cmd, cwd=WT, env=ENV, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, encoding="utf-8", errors="replace")
    lines = []
    stopped = False
    for line in p.stdout:
        lines.append(line.rstrip("\n"))
        if (stop_after and stop_after in line) or time.time() - t0 > limit:
            subprocess.run(["taskkill", "/T", "/F", "/PID", str(p.pid)], capture_output=True)
            stopped = True
            if not (stop_after and stop_after in line):
                lines.append(f"(stopped by the receipt after {limit} s)")
            break
    p.wait()
    return ("stopped" if stopped else p.returncode), "\n".join(lines), time.time() - t0


def with_gate_of(commit, body):
    """Runs body with m8a.ps1 as of `commit` in place, then restores the fixed file byte for byte."""
    fixed = open(GATE, "rb").read()
    old = subprocess.run(["git", "show", f"{commit}:tools/gates/m8a.ps1"], cwd=WT, capture_output=True).stdout
    old = old.replace(b"\r\n", b"\n")
    if b"\r\n" in fixed:
        old = old.replace(b"\n", b"\r\n")
    try:
        open(GATE, "wb").write(old)
        body()
    finally:
        open(GATE, "wb").write(fixed)
    return open(GATE, "rb").read() == fixed


def main():
    log = open(sys.argv[1], "w", encoding="utf-8", newline="\n")
    written = []
    renamed = os.path.join(SCRATCH, "renamed", "20260929T093134Y")
    os.makedirs(os.path.join(renamed, "runs"), exist_ok=True)
    written.append(renamed + r"\runs (empty)")

    def case(name, args, **kw):
        code, out, secs = gate(args, **kw)
        log.write(f"==== {name}\nargs: {' '.join(args)}\nexit: {code} ({secs:.1f} s)\n{out}\n\n")
        log.flush()
        print(f"{name}: exit {code}", flush=True)

    head = subprocess.run(["git", "rev-parse", "--short", "HEAD"], cwd=WT, capture_output=True, text=True).stdout.strip()
    log.write(f"tree: HEAD {head} and the working tree as it stands; the committed seal's sha256 (bytes as checked out) {sha(SEAL)}\n\n")

    case("G1 fixed gate, -From a renamed bed folder", ["-From", renamed])

    outside = os.path.join(SCRATCH, "seal-copy", "outputs-seal.json")
    os.makedirs(os.path.dirname(outside), exist_ok=True)
    shutil.copyfile(SEAL, outside)
    written.append(outside)
    case("G2 fixed gate, -Seal a copy outside the repository", ["-From", BED, "-Seal", outside])

    untracked = os.path.join(WT, "target", "r2-untracked-seal", "outputs-seal.json")
    os.makedirs(os.path.dirname(untracked), exist_ok=True)
    shutil.copyfile(SEAL, untracked)
    written.append(untracked + " (gitignored)")
    case("G3 fixed gate, -Seal an untracked copy inside the repository", ["-From", BED, "-Seal", untracked])

    original = open(SEAL, "rb").read()
    try:
        open(SEAL, "wb").write(original.replace(b'"seal_version": 1', b'"seal_version":  1', 1))
        case("G4 fixed gate, -From the M8a bed, its committed seal changed by one byte", ["-From", BED])
    finally:
        open(SEAL, "wb").write(original)
    log.write(f"G4 seal restored byte for byte: {open(SEAL, 'rb').read() == original}\n\n")

    case("G5 fixed gate, -From the M8a bed as named", ["-From", BED], stop_after="seal: ")
    lower = BED[: -len("20260929T093134Z")] + "20260929t093134z"
    case("G6 fixed gate, -From the M8a bed spelled in lower case", ["-From", lower], stop_after="seal: ")

    ok = with_gate_of("4d79c7d", lambda: case("G0 the gate before round 2 (4d79c7d), -From the renamed folder", ["-From", renamed], stop_after="PASS  E1 preprocess.exe"))
    log.write(f"G0 m8a.ps1 restored byte for byte: {ok}\n\n")
    ok = with_gate_of("0069cd2", lambda: case("G7 the gate of 0069cd2, -From the M8a bed spelled in lower case", ["-From", lower]))
    log.write(f"G7 m8a.ps1 restored byte for byte: {ok}\n\n")

    log.write("written by this receipt, left in place:\n" + "\n".join("  " + w for w in written) + "\n")
    log.write("and each gate run's work folder under C:\\tmp\\nm-target\\gates\\m8a\\ that it printed as 'work:'\n")
    log.close()


if __name__ == "__main__":
    main()
