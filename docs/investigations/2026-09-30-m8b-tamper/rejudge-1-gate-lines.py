"""Round 1 re-judge: this gate transcript's lines against M8b step 1's (the fixed gate before the tamper
fix), with the clock, the work folder, the worktree's name and each run of seconds left out.

usage: py -3 r1_compare_gate_lines.py <step 1 transcript> <round 1 transcript>
"""
import sys as _s; _s.stdout.reconfigure(encoding="utf-8")
import difflib
import re
import sys


def norm(line):
    line = line.rstrip("\n")
    if line.startswith(("START ", "END ", "=== DONE")):
        return None
    line = re.sub(r"^\d\d:\d\d:\d\d ", "", line)
    line = re.sub(r"C:\\tmp\\nm-target\\gates\\m8a\\\d{8}-\d{6}(\\reread\\\d{8}T\d{6}Z)?", "<work>", line)
    line = re.sub(r"worktrees\\m8b(-tamper)?\\", r"worktrees\\<wt>\\", line)
    line = re.sub(r"after [\d,]+ s", "after <n> s", line)
    line = re.sub(r": \d+ s$", ": <n> s", line)
    return line


a = [x for x in (norm(l) for l in open(sys.argv[1], encoding="utf-8-sig")) if x is not None]
b = [x for x in (norm(l) for l in open(sys.argv[2], encoding="utf-8-sig")) if x is not None]
pa = [x for x in a if x.startswith(("PASS", "FAIL"))]
pb = [x for x in b if x.startswith(("PASS", "FAIL"))]
print(f"step 1: {len(a)} lines, {len(pa)} PASS/FAIL; round 1: {len(b)} lines, {len(pb)} PASS/FAIL")
print(f"PASS/FAIL lines identical, same order: {pa == pb}; FAIL lines in round 1: {sum(x.startswith('FAIL') for x in pb)}")
print("every other difference (step 1 '-', round 1 '+'):")
for d in difflib.unified_diff(a, b, lineterm="", n=0):
    if d.startswith(("---", "+++", "@@")):
        continue
    print("  " + d[:300])
