"""spps-gpu bed: stage a solver folder safely, and run a solver on it.

SPPS and spps-gpu write into config.xml's ROOT `workingdirectory`. This script copies only a run's
inputs (config.xml, mesh.cbin, tetramesh.mbin, and the loudspeakers folder when present) into a NEW
folder under one of the two output roots, rewrites the root attribute to that folder, and refuses to
run any solver unless the attribute names the folder it is in, inside an output root.

usage:
  python stage.py copy <src-solve-dir> <dest-dir> [attr=value ...]     stage a direct run folder
  python stage.py template <src-solve-dir> <dest-dir> [attr=value ...] stage a run-folder template
                                                                        (workingdirectory="__RUNDIR__")
  python stage.py run <exe> <dest-dir> [extra args ...]                 run a solver on a staged folder
attr=value pairs set <simulation> attributes (e.g. nbparticules=20000 random_seed=7); a value of
"-" removes the attribute; only_band=<freq> computes that band alone.
"""
import os, re, shutil, subprocess, sys, time
import xml.etree.ElementTree as ET

ROOTS = [os.path.normcase(os.path.abspath(p)) + os.sep for p in (
    r"B:\repos\I-Simpa_Night_Mode\.out\spps-gpu", r"C:\tmp\nm-spps-gpu",
    r"B:\repos\I-Simpa_Night_Mode\.out\a4", r"C:\tmp\nm-a4")]
INPUTS = ("config.xml", "mesh.cbin", "tetramesh.mbin")


def inside_roots(path):
    p = os.path.normcase(os.path.abspath(path)) + os.sep
    return any(p.startswith(r) for r in ROOTS)


def set_attrs(text, wd, attrs):
    # Edit the XML text in place (the attribute order and the rest of the file stay as they were).
    text = re.sub(r'(<configuration\b[^>]*\bworkingdirectory=")[^"]*(")', lambda m: m.group(1) + wd + m.group(2), text, count=1)
    only = attrs.pop("only_band", None)
    if only is not None:
        # compute one band: docalc="1" on it, "0" on the others (spectra stay mapped by position)
        def fix(m):
            tag = m.group(0)
            f = re.search(r'freq="([^"]*)"', tag).group(1)
            return re.sub(r'docalc="[^"]*"', 'docalc="%s"' % ("1" if f == only else "0"), tag)
        fe = re.search(r"<freq_enum>.*?</freq_enum>", text, re.S)
        text = text[:fe.start()] + re.sub(r"<bfreq\b[^>]*/>", fix, fe.group(0)) + text[fe.end():]
    # rule variants for the bed: every material band's attribute (mat.<attr>), every material's
    # side_material (side.material), every source's attribute (src.<attr>), the atmosphere (atmo.<attr>)
    for k in [k for k in attrs if "." in k]:
        v = attrs.pop(k)
        scope, a = k.split(".", 1)
        if scope == "mat":
            se = re.search(r"<surface_absorption_enum>.*?</surface_absorption_enum>", text, re.S)
            body = re.sub(r'(<bfreq\b[^>]*?\s' + re.escape(a) + r'=")[^"]*(")', lambda m: m.group(1) + v + m.group(2), se.group(0))
            text = text[:se.start()] + body + text[se.end():]
        elif scope == "side":
            text = re.sub(r'(<type_surface\b[^>]*\bside_material=")[^"]*(")', lambda m: m.group(1) + v + m.group(2), text)
        elif scope == "src":
            def put(m, a=a, v=v):
                tag = m.group(0)
                if re.search(r'\s' + re.escape(a) + r'="', tag):
                    return re.sub(r'(\s' + re.escape(a) + r'=")[^"]*(")', lambda n: n.group(1) + v + n.group(2), tag)
                return tag.replace("<source", "<source " + a + '="' + v + '"', 1)
            text = re.sub(r"<source\b[^>]*>", put, text)
        elif scope == "atmo":
            text = re.sub(r'(<condition_atmospherique\b[^>]*\s' + re.escape(a) + r'=")[^"]*(")', lambda m: m.group(1) + v + m.group(2), text)
        else:
            sys.exit(f"unknown variant key {k}")
    for k, v in attrs.items():
        pat = re.compile(r'(<simulation\b[^>]*?)\s' + re.escape(k) + r'="[^"]*"')
        if v == "-":
            text = pat.sub(lambda m: m.group(1), text, count=1)
        elif pat.search(text):
            text = pat.sub(lambda m: m.group(1) + " " + k + '="' + v + '"', text, count=1)
        else:
            text = re.sub(r'(<simulation\b)', lambda m: m.group(1) + " " + k + '="' + v + '"', text, count=1)
    return text


def stage(src, dest, attrs, template):
    if not inside_roots(dest):
        sys.exit(f"refused: {dest} is outside the output roots {ROOTS}")
    if os.path.exists(dest):
        sys.exit(f"refused: {dest} exists; stage into a new folder")
    os.makedirs(dest)
    for f in INPUTS:
        shutil.copyfile(os.path.join(src, f), os.path.join(dest, f))
    ls = os.path.join(src, "loudspeakers")
    if os.path.isdir(ls):
        shutil.copytree(ls, os.path.join(dest, "loudspeakers"))
    cfg = os.path.join(dest, "config.xml")
    with open(cfg, encoding="utf-8") as fh:
        text = fh.read()
    wd = "__RUNDIR__" if template else os.path.abspath(dest) + "\\"
    text = set_attrs(text, wd, attrs)
    with open(cfg, "w", encoding="utf-8", newline="") as fh:
        fh.write(text)
    check(dest, template)
    print(f"staged {dest} ({'template' if template else 'run folder'})")


def check(dest, template=False):
    root = ET.parse(os.path.join(dest, "config.xml")).getroot()
    wd = root.get("workingdirectory", "")
    if template:
        if wd != "__RUNDIR__":
            sys.exit(f"refused: template workingdirectory is {wd!r}, not __RUNDIR__")
        return
    want = os.path.normcase(os.path.abspath(dest)) + os.sep
    got = os.path.normcase(os.path.abspath(wd.rstrip("\\/"))) + os.sep if wd else ""
    if got != want or not wd.endswith("\\"):
        sys.exit(f"refused: workingdirectory {wd!r} does not name the staged folder {dest}")
    if not inside_roots(wd):
        sys.exit(f"refused: workingdirectory {wd!r} is outside the output roots")


def run(exe, dest, extra):
    check(dest)
    cmd = [exe] + extra + [os.path.join(os.path.abspath(dest), "config.xml")]
    t = time.perf_counter()
    with open(os.path.join(dest, "solver.stdout.txt"), "w") as out, open(os.path.join(dest, "solver.stderr.txt"), "w") as err:
        rc = subprocess.call(cmd, stdout=out, stderr=err, cwd=dest)
    wall = time.perf_counter() - t
    with open(os.path.join(dest, "wall.txt"), "w") as fh:
        fh.write(f"{wall:.3f}\n{rc}\n{' '.join(cmd)}\n")
    print(f"{exe}: exit {rc}, {wall:.3f} s wall, {dest}")
    return rc


if __name__ == "__main__":
    a = sys.argv[1:]
    if not a:
        sys.exit(__doc__)
    if a[0] in ("copy", "template"):
        attrs = dict(x.split("=", 1) for x in a[3:])
        stage(a[1], a[2], attrs, a[0] == "template")
    elif a[0] == "run":
        sys.exit(run(a[1], a[2], a[3:]))
    else:
        sys.exit(__doc__)
