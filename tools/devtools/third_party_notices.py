"""Writes app/src-tauri/about/THIRD-PARTY-NOTICES.txt (parity A23): what app.exe and its solvers carry
of other people's work, under which licence, with the licence texts the packages ship.

usage: python tools/devtools/third_party_notices.py [--check]

- The solvers, the mesher, the fonts and the shipped data: written here by hand (SHIPPED below), each
  with the file in this repository or upstream's that is its receipt.
- Rust: `cargo metadata --filter-platform x86_64-pc-windows-msvc`, walked from the `app` package
  along normal dependencies (build and dev dependencies are not shipped). Each crate's own LICENSE*,
  COPYING*, NOTICE* and UNLICENSE* files are read from its source folder.
- JavaScript: app/package.json's `dependencies`, walked through app/node_modules (devDependencies
  build the bundle and are not in it), with each package's licence files.

Identical licence texts are printed once, with every package that ships that text. `--check` writes
nothing and exits 1 when the committed file differs from what would be written (the generator's own
test that the file is not stale).
"""
import hashlib
import json
import os
import subprocess
import sys

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
APP = os.path.join(REPO, "app")
OUT = os.path.join(APP, "src-tauri", "about", "THIRD-PARTY-NOTICES.txt")
LICENCE_PREFIXES = ("license", "licence", "copying", "notice", "unlicense")

# The parts of the release that no package manager lists. Each receipt is a path in this repository,
# or in upstream I-Simpa's source at the pinned tag (B:\repos\I-Simpa-upstream).
SHIPPED = [
    ("I-Simpa solvers: SPPS (spps.exe), TCR (classicalTheory.exe), preprocess (preprocess.exe)",
     "GPL-3.0-or-later",
     "Copyright (C) 2007-2022 Universite Gustave Eiffel - Judicael Picaut, Nicolas Fortin. "
     "Built from upstream I-Simpa (https://github.com/Universite-Gustave-Eiffel/I-Simpa) at the commit "
     "solvers/manifest.json names, with this repository's patches/ (receipt: upstream Docs/License.txt, "
     "LICENSE.md)."),
    ("Libraries the I-Simpa solvers are built with",
     "BSL-1.0 (Boost; utf8-cpp), MIT (pugixml, rply)",
     "Boost (random, thread, filesystem, algorithm), pugixml, rply and utf8-cpp, from upstream's "
     "src/lib_interface (input_output/pugixml, input_output/ply, input_output/utf8) and its CMake "
     "dependencies (src/spps/CMakeLists.txt, src/lib_interface/CMakeLists.txt)."),
    ("TetGen 1.5.0 (tetgen.exe), the volume mesher",
     "AGPL-3.0-or-later (dual-licensed: AGPL-3.0, or a commercial licence from WIAS)",
     "Copyright (c) 2002-2013 Hang Si, Weierstrass Institute for Applied Analysis and Stochastics (WIAS). "
     "This free release uses it under the AGPL-3.0; its source is third_party/tetgen-1.5.0 and its "
     "licence text is printed in full below."),
    ("SPPS on the GPU (spps-gpu.exe)",
     "GPL-3.0-only",
     "This repository's port of SPPS (solvers/spps-gpu). It includes TinyXML-2 10.0.0 (zlib licence, "
     "solvers/spps-gpu/third_party/tinyxml2/LICENSE.txt, printed below); see solvers/spps-gpu/THIRD-PARTY.md."),
    ("Microsoft Visual C++ runtime (solvers\\msvcp140.dll, vcruntime140.dll, vcruntime140_1.dll, vcomp140.dll)",
     "Microsoft Software License Terms for Visual Studio 2022, Distributable Code",
     "Copyright (c) Microsoft Corporation. Redistributed unmodified beside the solvers that link it, from "
     "Visual Studio's VC\\Redist folder, whose redist.txt lists these files as distributable "
     "(decision 85; tools/devtools/payload.ps1)."),
    ("The installer and uninstaller (NSIS 3.11)",
     "zlib/libpng (NSIS); its LZMA compression module under the Common Public License 1.0 with NSIS's "
     "linking exception",
     "Nullsoft Scriptable Install System (https://nsis.sourceforge.io/), as its COPYING states; the "
     "installer's script is tools/installer/night-mode.nsi."),
    ("Fonts: Inter and JetBrains Mono, bundled in the interface",
     "OFL-1.1",
     "The SIL Open Font License texts ship beside them (app/ui/public/licenses/OFL-Inter.txt, "
     "OFL-JetBrainsMono.txt) and are printed below."),
    ("Example rooms: Elmia hall, Industrial hall and I-Simpa's tutorial projects",
     "GPL-3.0",
     "From upstream I-Simpa's tutorial projects (src/isimpa/resources/doc/tutorial); "
     "app/src-tauri/examples/ATTRIBUTION.md."),
    ("Example rooms: BRAS CR1, CR2, CR3, CR4",
     "CC-BY-SA-4.0",
     "Derived from the Benchmark for Room Acoustical Simulation (BRAS), L. Aspoeck, M. Vorlaender, "
     "F. Brinkmann, D. Ackermann, S. Weinzierl (RWTH Aachen University, TU Berlin). "
     "https://creativecommons.org/licenses/by-sa/4.0/ ; app/src-tauri/examples/ATTRIBUTION.md."),
    ("Dry clip for Listen: speech (the claps, melody and groove beside it are the app's own, GPL-3.0)",
     "Public domain (LibriVox)",
     "app/src-tauri/examples/clips/speech-lv-hislastbow.provenance.md."),
]
# Licence texts kept in this repository, printed with the hand-written rows.
SHIPPED_TEXTS = [
    ("TetGen 1.5.0", os.path.join(REPO, "third_party", "tetgen-1.5.0", "LICENSE")),
    ("TinyXML-2 10.0.0", os.path.join(REPO, "solvers", "spps-gpu", "third_party", "tinyxml2", "LICENSE.txt")),
    ("Inter", os.path.join(APP, "ui", "public", "licenses", "OFL-Inter.txt")),
    ("JetBrains Mono", os.path.join(APP, "ui", "public", "licenses", "OFL-JetBrainsMono.txt")),
]


def read_text(path):
    with open(path, "rb") as f:
        t = f.read().decode("utf-8", errors="replace")
    return t.replace("\r\n", "\n").replace("\r", "\n").strip("\n\ufeff ") + "\n"


def licence_files(folder):
    out = []
    for name in sorted(os.listdir(folder)):
        p = os.path.join(folder, name)
        if os.path.isfile(p) and name.lower().startswith(LICENCE_PREFIXES) and not name.lower().endswith((".rs", ".toml", ".json")):
            out.append(p)
    return out


def rust_crates():
    meta = json.loads(subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--filter-platform", "x86_64-pc-windows-msvc"],
        cwd=REPO, capture_output=True, encoding="utf-8", check=True).stdout)
    packages = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    root = next(p["id"] for p in meta["packages"] if p["name"] == "app")
    seen, stack = set(), [root]
    while stack:
        i = stack.pop()
        if i in seen:
            continue
        seen.add(i)
        for d in nodes[i]["deps"]:
            if any(k["kind"] is None for k in d["dep_kinds"]):
                stack.append(d["pkg"])
    rows = []
    for i in seen:
        p = packages[i]
        if p["source"] is None:  # this workspace's own crates: GPL-3.0, the app itself
            continue
        folder = os.path.dirname(p["manifest_path"])
        rows.append((p["name"], p["version"], p.get("license") or "see its licence file",
                     p.get("repository") or "", licence_files(folder)))
    return sorted(rows, key=lambda r: (r[0].lower(), r[1]))


def js_packages():
    pkg = json.load(open(os.path.join(APP, "package.json"), encoding="utf-8"))
    nm = os.path.join(APP, "node_modules")
    rows, seen, stack = [], set(), list(pkg.get("dependencies", {}))
    while stack:
        name = stack.pop()
        if name in seen:
            continue
        seen.add(name)
        folder = os.path.join(nm, *name.split("/"))
        meta = json.load(open(os.path.join(folder, "package.json"), encoding="utf-8"))
        stack.extend(meta.get("dependencies", {}))
        repo = meta.get("repository")
        repo = repo.get("url", "") if isinstance(repo, dict) else (repo or "")
        rows.append((name, meta["version"], meta.get("license") or "see its licence file", repo, licence_files(folder)))
    return sorted(rows, key=lambda r: r[0].lower())


def build():
    crates, js = rust_crates(), js_packages()
    texts = {}  # sha -> [text, [who]]

    def keep(who, text):
        h = hashlib.sha256(text.encode("utf-8")).hexdigest()
        texts.setdefault(h, [text, []])[1].append(who)

    for who, path in SHIPPED_TEXTS:
        keep(who, read_text(path))
    for name, ver, _, _, files in crates + js:
        for f in files:
            keep(f"{name} {ver} ({os.path.basename(f)})", read_text(f))

    L = []
    L.append("I-Simpa Night Mode: third-party notices")
    L.append("=" * 39)
    L.append("")
    L.append("Night Mode itself is free software under the GNU General Public License, version 3 (LICENSE).")
    L.append("This file lists the work of others that app.exe and the solvers beside it carry, and under")
    L.append("which licence. Written by tools/devtools/third_party_notices.py; do not edit by hand.")
    L.append("")
    L.append("1. The solvers, the mesher, the fonts and the shipped data")
    L.append("-" * 58)
    for what, lic, note in SHIPPED:
        L.append("")
        L.append(what)
        L.append(f"  Licence: {lic}")
        L.append(f"  {note}")
    L.append("")
    L.append("")
    L.append(f"2. Rust crates compiled into app.exe ({len(crates)})")
    L.append("-" * 50)
    for name, ver, lic, repo, files in crates:
        L.append(f"{name} {ver}  [{lic}]{'  ' + repo if repo else ''}{'' if files else '  (ships no licence file)'}")
    L.append("")
    L.append("")
    L.append(f"3. JavaScript packages in the interface ({len(js)})")
    L.append("-" * 50)
    for name, ver, lic, repo, files in js:
        L.append(f"{name} {ver}  [{lic}]{'  ' + repo if repo else ''}{'' if files else '  (ships no licence file)'}")
    L.append("")
    L.append("")
    L.append(f"4. Licence texts ({len(texts)} distinct), each with the packages that ship it")
    L.append("-" * 60)
    for text, who in sorted(texts.values(), key=lambda t: t[1][0].lower()):
        L.append("")
        L.append("~" * 72)
        L.append("Shipped by: " + "; ".join(who))
        L.append("~" * 72)
        L.append(text.rstrip("\n"))
    return "\n".join(L) + "\n"


def main():
    text = build()
    if "--check" in sys.argv:
        old = open(OUT, encoding="utf-8").read().replace("\r\n", "\n") if os.path.exists(OUT) else ""
        if old != text:
            print(f"{OUT} is stale: run python tools/devtools/third_party_notices.py")
            sys.exit(1)
        print(f"{OUT} is current")
        return
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    with open(OUT, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)
    print(f"wrote {OUT}: {len(text.encode('utf-8'))} bytes")


if __name__ == "__main__":
    main()
