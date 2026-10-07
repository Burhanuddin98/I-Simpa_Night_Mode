"""C1's bed (SPEC.md, order of work 1): a stranger's blank 6 x 10 x 3 m box, written here as a raw OBJ,
driven through the release app over the DevTools protocol only (no window focus, no OS mouse): import
through the dialog, carve the walls into named groups from the 3D view, rename, merge, materials (library
and typed per band, scattering, transmission), a source with its power and spectrum, receivers and a
plane, an SPPS run on the CPU (and the GPU with --gpu), the Results step, the Acoustics tab and the map.

Each step is tried the way a user would do it: a control that is not on the page is recorded as MISSING
with what was looked for, and the bed goes on with what it can. The same script is the bed before the
build and after it.

usage: python bed.py <app.exe> <solvers-dir> <out-dir> [--grouped] [--gpu] [--port N]
  --grouped  the box with six `g` groups instead of the groupless one (no carving)
Writes <out-dir>/bed.json, bed.log and screenshots; the OBJ files and the saved project go in <out-dir>.
Needs websocket-client (do not run with python -I). Stops only the app.exe it started.
"""
import base64, json, math, os, subprocess, sys, time, urllib.request
import websocket

args = [a for a in sys.argv[1:] if not a.startswith("--")]
flags = [a for a in sys.argv[1:] if a.startswith("--")]
app, solvers, out = args[0], args[1], args[2]
GROUPED = "--grouped" in flags
GPU = "--gpu" in flags
port = 9241
for i, a in enumerate(sys.argv):
    if a == "--port":
        port = int(sys.argv[i + 1])
os.makedirs(out, exist_ok=True)
R = {"app": app, "app_mtime": time.strftime("%Y-%m-%d %H:%M:%S", time.localtime(os.path.getmtime(app))),
     "solvers": solvers, "grouped": GROUPED, "steps": []}


def log(msg):
    line = time.strftime("%H:%M:%S") + "  " + msg
    print(line, flush=True)
    with open(os.path.join(out, "bed.log"), "a", encoding="utf-8") as f:
        f.write(line + "\n")


def step(name, status, **kw):
    """status: OK (done the user's way), MISSING (no control for it), REFUSED (the app said no), FAIL."""
    R["steps"].append({"step": name, "status": status, **kw})
    log(f"[{status}] {name} {json.dumps(kw)[:400]}")


# ---- the box -----------------------------------------------------------------------------------
# x 0..6, y 0..10, z 0..3 (Z up). Each wall a quad, wound so its normal points out of the room.
V = [(0, 0, 0), (6, 0, 0), (6, 10, 0), (0, 10, 0), (0, 0, 3), (6, 0, 3), (6, 10, 3), (0, 10, 3)]
QUADS = {
    "floor": (1, 4, 3, 2),        # -z
    "ceiling": (5, 6, 7, 8),      # +z
    "wall south": (1, 2, 6, 5),   # -y
    "wall north": (3, 4, 8, 7),   # +y
    "wall west": (1, 5, 8, 4),    # -x
    "wall east": (2, 3, 7, 6),    # +x
}
NORMALS = {"floor": (0, 0, -1), "ceiling": (0, 0, 1), "wall south": (0, -1, 0), "wall north": (0, 1, 0),
           "wall west": (-1, 0, 0), "wall east": (1, 0, 0)}


def write_obj(path, grouped):
    lines = ["# a 6 x 10 x 3 m box, metres, Z up" + ("" if grouped else "; one object, no g, no usemtl")]
    lines += [f"v {x} {y} {z}" for x, y, z in V]
    for name, q in QUADS.items():
        if grouped:
            lines.append(f"g {name.replace(' ', '_')}")
        lines.append("f " + " ".join(map(str, q)))
    open(path, "w", encoding="ascii", newline="\n").write("\n".join(lines) + "\n")


obj = os.path.join(out, "box_grouped.obj" if GROUPED else "box_blank.obj")
write_obj(obj, GROUPED)
R["obj"] = obj
R["obj_text"] = open(obj).read()

# A WebView2 data folder of its own: with Burhan's app open, a shared one would join his browser
# process, which ignores the debugging port.
env = dict(os.environ, SIMPA_SOLVERS_DIR=solvers,
           WEBVIEW2_USER_DATA_FOLDER=os.environ.get("BED_WEBVIEW_DIR", "C:/tmp/nm-target-blank/webview2"),
           WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=f"--remote-debugging-port={port}")
proc = subprocess.Popen([app, "--e2e"], env=env)
log(f"started app pid {proc.pid}")
ws = None
msg_id = 0


def connect():
    global ws
    t0 = time.time()
    while time.time() - t0 < 60:
        try:
            pages = json.load(urllib.request.urlopen(f"http://127.0.0.1:{port}/json", timeout=5))
            page = next((p for p in pages if p.get("type") == "page"), None)
            if page:
                ws = websocket.create_connection(page["webSocketDebuggerUrl"], timeout=1200, suppress_origin=True)
                return
        except Exception:
            pass
        time.sleep(0.5)
    raise SystemExit("no DevTools page")


def cdp(method, params=None):
    global msg_id
    msg_id += 1
    ws.send(json.dumps({"id": msg_id, "method": method, "params": params or {}}))
    while True:
        m = json.loads(ws.recv())
        if m.get("id") == msg_id:
            if "error" in m:
                raise RuntimeError(f"CDP error: {m['error']}")
            return m["result"]


def js(expr):
    r = cdp("Runtime.evaluate", {"expression": expr, "awaitPromise": True, "returnByValue": True})
    if "exceptionDetails" in r:
        ex = r["exceptionDetails"]
        raise RuntimeError("JS exception: " + json.dumps(ex.get("exception", {}).get("description") or ex.get("text")))
    return r["result"].get("value")


def hook(name, *a):
    return js(f"window.__m10[{json.dumps(name)}](...{json.dumps(list(a))})")


def idle():
    return hook("idle")


def shot(name):
    data = base64.b64decode(cdp("Page.captureScreenshot", {"format": "png"})["data"])
    p = os.path.join(out, name)
    open(p, "wb").write(data)
    return name


def wait_js(expr, timeout=120, every=0.3):
    t0 = time.time()
    while time.time() - t0 < timeout:
        v = js(expr)
        if v:
            return v
        time.sleep(every)
    raise RuntimeError(f"timed out waiting for {expr}")


def exists(sel):
    return js(f"!!document.querySelector({json.dumps(sel)})")


def click(sel):
    """A DOM click on the element (the button's own handler, as a pointer click would run it)."""
    ok = js(f"(() => {{ const e = document.querySelector({json.dumps(sel)}); if (!e) return false; e.click(); return true; }})()")
    if not ok:
        raise RuntimeError(f"no element {sel}")
    idle()


def center(sel):
    return js(f"(() => {{ const e = document.querySelector({json.dumps(sel)}); if (!e) return null; e.scrollIntoView({{block: 'nearest'}}); const r = e.getBoundingClientRect(); return {{x: r.x + r.width / 2, y: r.y + r.height / 2}}; }})()")


def mouse(x, y, button="left", count=1, modifiers=0):
    cdp("Input.dispatchMouseEvent", {"type": "mouseMoved", "x": x, "y": y, "modifiers": modifiers})
    for c in range(1, count + 1):
        cdp("Input.dispatchMouseEvent", {"type": "mousePressed", "x": x, "y": y, "button": button, "clickCount": c, "modifiers": modifiers})
        cdp("Input.dispatchMouseEvent", {"type": "mouseReleased", "x": x, "y": y, "button": button, "clickCount": c, "modifiers": modifiers})
    time.sleep(0.15)
    idle()


VK = {"F2": 113, "Enter": 13, "Escape": 27, "Delete": 46, "Tab": 9}


def key(k, modifiers=0):
    for t in ("rawKeyDown", "keyUp"):
        cdp("Input.dispatchKeyEvent", {"type": t, "key": k, "code": k, "windowsVirtualKeyCode": VK[k], "modifiers": modifiers})
    time.sleep(0.1)
    idle()


def type_text(text):
    cdp("Input.insertText", {"text": text})
    time.sleep(0.05)


def project():
    return json.loads(hook("projectJson"))


def groups():
    p = project()
    counts = {}
    for f in p["geometry"]["faces"]:
        counts[f[3]] = counts.get(f[3], 0) + 1
    mats = {m["id"]: m["name"] for m in p["materials"]}
    return [{"id": g["id"], "name": g["name"], "faces": counts.get(g["id"], 0), "material": mats.get(g["material"])}
            for g in p["surface_groups"]]


def face_of(wall, which=0):
    """The index of the `which`-th face whose normal is the wall's (from the project's own mesh)."""
    p = project()
    vs = [[float(c) for c in v] for v in p["geometry"]["vertices"]]
    want = NORMALS[wall]
    hits = []
    for i, f in enumerate(p["geometry"]["faces"]):
        a, b, c = (vs[k] for k in f[:3])
        u = [b[j] - a[j] for j in range(3)]
        w = [c[j] - a[j] for j in range(3)]
        n = [u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]]
        L = math.sqrt(sum(x * x for x in n)) or 1
        # Either winding: the plane is what names the wall (the room's faces may point in).
        if abs(sum(n[j] / L * want[j] for j in range(3))) > 0.999:
            centre = [(a[j] + b[j] + c[j]) / 3 for j in range(3)]
            side = sum(centre[j] * want[j] for j in range(3))
            hits.append((side, i))
    # Of the two parallel planes, the wall's is the one furthest along its normal.
    top = max(s for s, _ in hits)
    faces = [i for s, i in hits if abs(s - top) < 1e-9]
    return faces[which] if which < len(faces) else None


def pick(face, double):
    """Picks a face in the 3D view as the mouse does: aim the camera at it, click (or double-click)."""
    hook("setStep", "geometry")
    pt = hook("aimAtFace", face)
    if not pt:
        return None
    mouse(pt["x"], pt["y"], count=2 if double else 1)
    return pt, hook("selection")


def view_menu(pt):
    mouse(pt["x"], pt["y"], button="right")
    return js("[...document.querySelectorAll('[data-part=\"viewport-menu\"] [data-menu-item]')].map(b => ({id: b.dataset.menuItem, text: b.innerText.replace(/\\s+/g, ' ').trim(), disabled: b.disabled}))")


def new_group_from(face, double):
    before = {g["id"] for g in groups()}
    pt, sel = pick(face, double)
    items = view_menu(pt)
    if not any(i["id"] == "new-group-from-selection" for i in items):
        return None, sel, items
    click('[data-part="viewport-menu"] [data-menu-item="new-group-from-selection"]')
    added = [g for g in groups() if g["id"] not in before]
    return (added[0] if added else None), sel, items


def group_row(gid):
    return f'.scene [data-entity="surface_group:{gid}"]'


def rename_group(gid, name):
    """F2 on the group's row in the scene list, as for a source; type the name, Enter."""
    row = center(group_row(gid))
    if not row:
        return "no row"
    mouse(row["x"], row["y"])
    key("F2")
    focused = js("(() => { const a = document.activeElement; return a && a.tagName === 'INPUT' ? {field: a.dataset.field ?? null, part: a.dataset.part ?? null, value: a.value, inScene: !!a.closest('.scene')} : null; })()")
    if not focused:
        return {"missing": "F2 on a selected surface group row focuses no name field", "step_now": js("document.querySelector('[data-props-step]')?.dataset.propsStep")}
    js("document.activeElement.select()")
    type_text(name)
    key("Enter")
    g = next((x for x in groups() if x["id"] == gid), None)
    return {"focused": focused, "name_after": g and g["name"]}


try:
    connect()
    wait_js("!!window.__m10 && !!document.querySelector('[data-step]')", 60)
    hook("frame") if js("window.__m10.ready(['frame'])") else None

    # ---- 1. import through the dialog -------------------------------------------------------------
    hook("openImportDialog", obj)
    wait_js("!!document.querySelector('[data-part=\"import-dialog\"]')", 20)
    R["import_dialog"] = js("document.querySelector('[data-part=\"import-dialog\"]').innerText")
    shot("01-import-dialog.png")
    click('[data-part="import-confirm"]')
    wait_js("!!window.__m10.projectJson && document.querySelector('[data-part=\"import-dialog\"]') === null", 30)
    idle()
    hook("setStep", "geometry")
    time.sleep(0.5)
    R["after_import"] = {
        "groups": groups(),
        "geometry_panel": js("document.querySelector('[data-props-step=\"geometry\"]')?.innerText ?? null"),
        "issues": hook("issues"),
        "run_blockers": hook("runBlockers"),
        "console_fail": js("[...document.querySelectorAll('.console-line.FAIL .text')].map(e => e.textContent)"),
    }
    imported = R["after_import"]["groups"]
    step("import", "OK" if imported and sum(g["faces"] for g in imported) == 12 else "FAIL",
         groups=[(g["name"], g["faces"]) for g in imported], says=R["after_import"]["geometry_panel"])
    shot("02-geometry.png")

    # ---- 2. carve into named groups from the 3D view ----------------------------------------------
    names = {}  # wall -> group id
    if GROUPED:
        for g in imported:
            names[g["name"].replace("_", " ")] = g["id"]
        step("carve", "OK", note="grouped file: six groups from `g`", groups=list(names))
    else:
        base = imported[0]["id"] if len(imported) == 1 else None
        for wall in ("floor", "ceiling", "wall south"):
            g, sel, items = new_group_from(face_of(wall), double=True)
            if not g:
                step(f"carve {wall}", "MISSING", selection=sel, menu=items)
                continue
            names[wall] = g["id"]
            step(f"carve {wall}", "OK", group=g, selection=sel)
        # North: one triangle to a new group, the other moved into it ("Move selection to group").
        g, sel, items = new_group_from(face_of("wall north", 0), double=False)
        if g:
            names["wall north"] = g["id"]
            step("split one face (north, triangle 1)", "OK", group=g)
            pt, sel = pick(face_of("wall north", 1), double=False)
            items = view_menu(pt)
            move = [i for i in items if i["id"].startswith("move-to-group")]
            target = next((i for i in move if g["id"] in i["id"]), None)
            if not target:
                step("move selection to group (north, triangle 2)", "MISSING", menu=items)
                js("document.dispatchEvent(new KeyboardEvent('keydown', {key: 'Escape', bubbles: true}))")
            else:
                click(f'[data-part="viewport-menu"] [data-menu-item="{target["id"]}"]')
                gg = next(x for x in groups() if x["id"] == g["id"])
                step("move selection to group (north, triangle 2)", "OK" if gg["faces"] == 2 else "FAIL", group=gg)
        else:
            step("split one face (north)", "MISSING", menu=items)
        # West: each triangle its own group, then the two merged.
        g1, _, _ = new_group_from(face_of("wall west", 0), double=False)
        g2, _, _ = new_group_from(face_of("wall west", 1), double=False)
        if g1 and g2:
            r1, r2 = center(group_row(g1["id"])), center(group_row(g2["id"]))
            mouse(r1["x"], r1["y"])
            mouse(r2["x"], r2["y"], modifiers=2)  # Ctrl
            sel = js("JSON.stringify(window.__m10.ready(['selectionState']) ? window.__m10.selectionState() : null)")
            js("document.querySelector('[data-menu=\"Edit\"]')?.click()")
            time.sleep(0.3)
            edit_items = js("[...document.querySelectorAll('[data-menu-item]')].map(b => ({id: b.dataset.menuItem, text: b.innerText.replace(/\\s+/g, ' ').trim(), disabled: b.disabled}))")
            merge = exists('[data-menu-item="merge-groups"]') and not js("document.querySelector('[data-menu-item=\"merge-groups\"]').disabled")
            if merge:
                click('[data-menu-item="merge-groups"]')
                left = [x for x in groups() if x["id"] in (g1["id"], g2["id"])]
                names["wall west"] = g1["id"]
                step("merge two groups (west)", "OK" if len(left) == 1 and left[0]["faces"] == 2 else "FAIL", groups=left, selection=sel)
            else:
                js("document.dispatchEvent(new KeyboardEvent('keydown', {key: 'Escape', bubbles: true}))")
                key("Escape")
                names["wall west"] = g1["id"]
                step("merge two groups (west)", "MISSING", edit_menu=edit_items, selection=sel)
        if base:
            names["wall east"] = base
        R["after_carve"] = groups()
        shot("03-carved.png")

    # ---- 3. rename every group to its wall --------------------------------------------------------
    for wall, gid in names.items():
        r = rename_group(gid, wall)
        ok = isinstance(r, dict) and r.get("name_after") == wall
        step(f"rename -> {wall}", "OK" if ok else "MISSING", result=r)
        if not ok:
            key("Escape")
    R["after_rename"] = groups()
    shot("04-renamed.png")

    # ---- 4. materials: library for most, one typed per band ---------------------------------------
    lib = hook("materialLibrary")
    plan = {"floor": "10% absorbing", "ceiling": "50% absorbing", "wall south": "20% absorbing",
            "wall north": "20% absorbing", "wall west": "30% absorbing"}
    for wall, gid in names.items():
        if wall not in plan:
            continue
        row = center(group_row(gid))
        mouse(row["x"], row["y"])
        hook("setStep", "materials")
        entry = next(e for e in lib if e["name"] == plan[wall])
        mats = {m["name"]: m["id"] for m in project()["materials"]}
        if plan[wall] not in mats:
            click('[data-action="library"]')
            click(f'[data-library-add="{entry["reference_id"]}"]')
            mats = {m["name"]: m["id"] for m in project()["materials"]}
        click(f'[data-material-option="{mats[plan[wall]]}"]')
        g = next(x for x in groups() if x["id"] == gid)
        step(f"material {wall} <- library {plan[wall]}", "OK" if g["material"] == plan[wall] else "FAIL", group=g)

    # The east wall: + Material, its absorption typed per band (a paste of one row), scattering, transmission.
    gid = names.get("wall east")
    if gid:
        hook("setStep", "materials")
        before = {m["id"] for m in project()["materials"]}
        click('[data-action="add-material"]')
        new = [m for m in project()["materials"] if m["id"] not in before]
        if new:
            m = new[0]
            nb = len(m["absorption"])
            alpha = [round(0.05 + 0.6 * i / max(1, nb - 1), 3) for i in range(nb)]
            row_i = js(f"[...document.querySelectorAll('[data-material-id]')].findIndex(r => r.dataset.materialId === {json.dumps(m['id'])})")
            cell = center(f'[data-grid-cell="{row_i}:0"]')
            mouse(cell["x"], cell["y"])
            pasted = js("""((txt) => { const grid = document.querySelector('[data-materials-grid]'); const t = document.activeElement;
              if (!grid || !t || !grid.contains(t)) return 'focus not in grid: ' + (t && t.tagName);
              const dt = new DataTransfer(); dt.setData('text/plain', txt);
              const ev = new ClipboardEvent('paste', {clipboardData: dt, bubbles: true, cancelable: true});
              if (ev.clipboardData === null) Object.defineProperty(ev, 'clipboardData', {value: dt});
              t.dispatchEvent(ev); return ev.defaultPrevented; })(%s)""" % json.dumps("\t".join(map(str, alpha))))
            idle()
            got = next(x for x in project()["materials"] if x["id"] == m["id"])
            step("typed absorption per band (+ Material, a pasted row)", "OK" if [float(v) for v in got["absorption"]] == alpha else "FAIL",
                 pasted=pasted, bands=nb, absorption=got["absorption"][:6])
            # Scattering per band: the grid's Scattering tab, one cell typed.
            if exists('[data-quantity-tab="scattering"]'):
                click('[data-quantity-tab="scattering"]')
                cell = center(f'[data-grid-cell="{row_i}:0"]')
                mouse(cell["x"], cell["y"], count=2)
                type_text("0.25")
                key("Enter")
                got = next(x for x in project()["materials"] if x["id"] == m["id"])
                step("typed scattering, band 1", "OK" if float(got["scattering"][0]) == 0.25 else "FAIL", scattering=got["scattering"][:3])
            else:
                step("typed scattering", "MISSING")
            tabs = js("[...document.querySelectorAll('[data-quantity-tab]')].map(t => t.dataset.quantityTab)")
            if "transmission" in tabs:
                click('[data-quantity-tab="transmission"]')
                cell = center(f'[data-grid-cell="{row_i}:0"]')
                mouse(cell["x"], cell["y"], count=2)
                type_text("20")
                key("Enter")
                got = next(x for x in project()["materials"] if x["id"] == m["id"])
                tl = got["transmission_loss_db"]
                step("typed transmission loss, band 1", "OK" if tl and tl[0] is not None and float(tl[0]) == 20 else "FAIL", transmission=tl and tl[:3])
                # Back off: the box has nothing behind its walls; keep the run plain.
                js("void 0")
            else:
                step("typed transmission loss", "MISSING", tabs=tabs,
                     panel=js("document.querySelector('.mat-row[title*=\"Transmission\"]')?.title ?? null"))
            click('[data-quantity-tab="absorption"]') if exists('[data-quantity-tab="absorption"]') else None
            row = center(group_row(gid))
            mouse(row["x"], row["y"])
            hook("setStep", "materials")
            click(f'[data-material-option="{m["id"]}"]')
            g = next(x for x in groups() if x["id"] == gid)
            step("material wall east <- the typed one", "OK" if g["material"] == got["name"] else "FAIL", group=g)
    R["after_materials"] = groups()
    shot("05-materials.png")

    # ---- 5. a source, its power and spectrum ------------------------------------------------------
    hook("setStep", "sources")
    click('[data-part="add-source"]')
    src = project()["sources"]
    step("add source", "OK" if len(src) == 1 else "FAIL", sources=[(s["name"], s["position"]) for s in src])
    fields = js("[...document.querySelectorAll('[data-props-step=\"sources\"] [data-field]')].map(e => e.dataset.field)")
    R["source_fields"] = fields
    if "power.global_db" in fields:
        f = '[data-props-step="sources"] [data-field="power.global_db"]'
        js(f"document.querySelector({json.dumps(f)}).focus(); document.querySelector({json.dumps(f)}).select()")
        type_text("95")
        key("Enter")
        s = project()["sources"][0]
        step("source power 95 dB", "OK" if float(s["power"]["global_db"]) == 95 else "FAIL", power=s["power"])
    else:
        step("source power", "MISSING", fields=fields,
             emission=js("document.querySelector('[data-props-step=\"sources\"] [data-input]')?.innerText ?? null"))
    if exists('[data-props-step="sources"] [data-field="spectrum"]'):
        opts = js("[...document.querySelector('[data-props-step=\"sources\"] [data-field=\"spectrum\"]').options].map(o => o.value)")
        want = next((o for o in opts if "ES_VL" in o), None) or opts[-1]
        js(f"""(() => {{ const s = document.querySelector('[data-props-step="sources"] [data-field="spectrum"]');
            const set = Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value').set; set.call(s, {json.dumps(want)});
            s.dispatchEvent(new Event('change', {{bubbles: true}})); }})()""")
        idle()
        s = project()["sources"][0]
        step(f"source spectrum <- {want}", "OK" if s["power"]["shape"]["kind"] != "pink" else "FAIL", options=opts, shape=s["power"]["shape"]["kind"])
    else:
        step("source spectrum", "MISSING")
    if exists('[data-props-step="sources"] [data-field="directivity"]'):
        opts = js("[...document.querySelector('[data-props-step=\"sources\"] [data-field=\"directivity\"]').options].map(o => o.value)")
        step("source directivity choices", "OK", options=opts)
    else:
        step("source directivity", "MISSING")
    shot("06-source.png")

    # ---- 6. receivers and a plane -----------------------------------------------------------------
    click('[data-part="add-receiver"]')
    rid = project()["point_receivers"][-1]["id"]
    f = '[data-props-step="sources"] [data-field="position.y"]'
    if exists(f):
        js(f"document.querySelector({json.dumps(f)}).focus(); document.querySelector({json.dumps(f)}).select()")
        type_text("8")
        key("Enter")
    click('[data-part="add-receiver"]')
    rs = project()["point_receivers"]
    step("add receivers", "OK" if len(rs) == 2 else "FAIL", receivers=[(r["name"], r["position"]) for r in rs])
    click('[data-part="add-plane"]')
    planes = project()["surface_receivers"]
    step("add plane", "OK" if planes else "FAIL", planes=[p["name"] for p in planes])
    shot("07-receivers.png")

    # ---- 7. save and run SPPS ---------------------------------------------------------------------
    path = os.path.join(out, "box.simpa")
    hook("saveAs", path)
    R["issues_before_run"] = hook("issues")
    # A group left without a material (one a missing step could not merge away) blocks the run:
    # recorded, then given the 30% library material through the same clicks, so the bed reaches
    # the run and the results.
    left = [g for g in groups() if g["material"] == "Default"]
    if left:
        step("groups left on the placeholder before the run", "REFUSED", groups=left, blockers=hook("runBlockers"))
        mats = {m["name"]: m["id"] for m in project()["materials"]}
        for g in left:
            row = center(group_row(g["id"]))
            mouse(row["x"], row["y"])
            hook("setStep", "materials")
            click(f'[data-material-option="{mats["30% absorbing"]}"]')
        hook("saveAs", path)

    def run(device):
        hook("setStep", "simulate")
        hook("setSolver", "spps")
        hook("setDevice", device)
        time.sleep(0.5)
        blockers = hook("runBlockers")
        panel = js("({text: document.querySelector('[data-part=\"run-panel\"]')?.innerText, blockers: document.querySelector('[data-part=\"run-panel\"]')?.dataset.blockers, disabled: document.querySelector('[data-part=\"run-panel\"]')?.disabled})")
        if blockers:
            step(f"run SPPS {device}", "REFUSED", blockers=blockers, panel=panel, issues=hook("issues"))
            shot(f"08-run-{device}-blocked.png")
            return None
        t0 = time.time()
        js("document.querySelector('[data-part=\"run-panel\"]').click()")
        name = js("window.__m10.waitRun(null, 'ended', 1200000)")
        rows = hook("runsRows")
        row = next(r for r in rows if r["run"] == name)
        keep = {k: row.get(k) for k in ("status", "solver", "gpu_device", "elapsed_s", "reasons", "exit_code")}
        # The solver's own stderr, read from the run folder beside the project.
        rdir = None
        for root, dirs, files in os.walk(out):
            if os.path.basename(root) == name:
                rdir = root
                break
        stderr = {}
        if rdir:
            for root, _, files in os.walk(rdir):
                for fn in files:
                    if "stderr" in fn.lower():
                        p = os.path.join(root, fn)
                        stderr[os.path.relpath(p, rdir)] = open(p, encoding="utf-8", errors="replace").read()[-1500:]
        step(f"run SPPS {device}", "OK" if keep["status"] == "OK" else "FAIL", run=name, row=keep,
             seconds=round(time.time() - t0, 1), run_dir=rdir, stderr=stderr)
        shot(f"08-run-{device}.png")
        return name

    name = run("cpu")
    if name:
        hook("selectRun", name)
        hook("setStep", "results")
        state = wait_js("(() => { const s = document.querySelector('[data-props-step=\"results\"] [data-results-state]')?.getAttribute('data-results-state'); return s && s !== 'checking' ? s : null; })()", 300)
        hook("dockTab", "acoustics")
        ac = wait_js("(() => { const a = document.querySelector('[data-acoustics]'); const s = a?.getAttribute('data-acoustics-state'); return s === 'ready' || s === 'refused' || s === 'error' ? s : null; })()", 300)
        time.sleep(2)
        view = hook("acousticsView")
        R["acoustics_view"] = view
        mp = None
        try:
            time.sleep(2)
            mp = hook("m12Map")
        except Exception as e:
            mp = {"error": str(e)}
        step("results", "OK" if state and ac == "ready" else "FAIL", results_state=state, acoustics=ac,
             acoustics_text=js("document.querySelector('[data-acoustics]')?.innerText?.slice(0, 600) ?? null"), map=mp)
        shot("09-results.png")
    if GPU:
        g = hook("gpuStatus")
        R["gpu_status"] = g
        n2 = run("gpu")
        if n2:
            hook("selectRun", n2)
            hook("setStep", "results")
            time.sleep(3)
            shot("10-results-gpu.png")
finally:
    with open(os.path.join(out, "bed.json"), "w", encoding="utf-8") as f:
        json.dump(R, f, indent=1)
    if ws:
        ws.close()
    proc.terminate()
    try:
        proc.wait(10)
    except Exception:
        proc.kill()
    log(f"app pid {proc.pid} stopped; receipt {os.path.join(out, 'bed.json')}")
