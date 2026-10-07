"""C2 bed (docs/investigations/2026-10-07-dock/SPEC.md): the bottom dock dragged and maximised on a project's
results. The release app.exe, driven over the DevTools protocol only (no OS mouse, no focus): the pointer
events are the page's own (Input.dispatchMouseEvent / dispatchKeyEvent), in its own WebView2 profile so
nothing it stores reaches another app.exe's. Checks the done-when's behaviour and takes the screenshots at
three heights (folded, half, maximised) on the Acoustics tab, plus the Console and Runs tabs maximised.
Receipt `bed-dock.json` and the PNGs in <out>.

usage: python bed-dock.py <app.exe> <solvers-dir> <project.simpa> <out-dir> [port]
Opens the project in place and never saves or runs. Needs websocket-client (do not run with python -I).
Stops only the app.exe it started.
"""
import base64, json, os, subprocess, sys, time, urllib.request
import websocket

app, solvers, project, out = sys.argv[1:5]
port = int(sys.argv[5]) if len(sys.argv) > 5 else 9241
os.makedirs(out, exist_ok=True)
profile = os.path.join(os.path.dirname(app), "..", "wv2-profile-bed-dock")
receipt = {"app": app, "app_mtime": time.strftime("%Y-%m-%d %H:%M:%S", time.localtime(os.path.getmtime(app))),
           "solvers": solvers, "project": project, "port": port, "profile": os.path.abspath(profile), "checks": []}


def log(msg):
    line = time.strftime("%H:%M:%S") + "  " + msg
    print(line, flush=True)
    with open(os.path.join(out, "bed-dock.log"), "a", encoding="utf-8") as f:
        f.write(line + "\n")


env = dict(os.environ, SIMPA_SOLVERS_DIR=solvers, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=f"--remote-debugging-port={port}",
           WEBVIEW2_USER_DATA_FOLDER=os.path.abspath(profile))
proc = subprocess.Popen([app, "--e2e", "--project", project], env=env)
log(f"started app pid {proc.pid} on {project}")
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
                ws = websocket.create_connection(page["webSocketDebuggerUrl"], timeout=900, suppress_origin=True)
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


def wait_js(expr, timeout=120, every=0.5):
    t0 = time.time()
    while time.time() - t0 < timeout:
        v = js(expr)
        if v:
            return v
        time.sleep(every)
    raise RuntimeError(f"timed out waiting for {expr}")


def shot(name):
    data = base64.b64decode(cdp("Page.captureScreenshot", {"format": "png"})["data"])
    p = os.path.join(out, name)
    open(p, "wb").write(data)
    log(f"screenshot {p} ({len(data)} bytes)")
    return p


def check(name, ok, detail):
    receipt["checks"].append({"check": name, "ok": bool(ok), "detail": detail})
    log(f"{'PASS' if ok else 'FAIL'}  {name}: {json.dumps(detail)}")


FRAMES = "new Promise(r => requestAnimationFrame(() => requestAnimationFrame(() => r(true))))"
# Two frames, then the dock's 240 ms height transition (motion.css runs outside WebDriver).
SETTLE = "new Promise(r => requestAnimationFrame(() => requestAnimationFrame(() => setTimeout(() => r(true), 400))))"
SIZE = "window.__m10.dockSize()"
STATE = ("(() => { const d = document.querySelector('.dock').getBoundingClientRect(); "
         "const v = document.querySelector('.center > .viewport').getBoundingClientRect(); "
         "return {dock: Math.round(d.height), view: Math.round(v.height), "
         "max: document.querySelector('.dock').dataset.max, folded: document.querySelector('.dock').dataset.folded, "
         "covers: (() => { const w = document.querySelector('.work').getBoundingClientRect(), g = parseFloat(getComputedStyle(document.querySelector('.work')).paddingLeft); "
         "return Math.abs(d.left - w.left - g) < 1 && Math.abs(d.right - w.right + g) < 1 && Math.abs(d.top - w.top - g) < 1 && Math.abs(d.bottom - w.bottom + g) < 1; })(), "
         "viewVisible: getComputedStyle(document.querySelector('.center > .viewport')).visibility}; })()")


def mouse(kind, x, y, clicks=1):
    cdp("Input.dispatchMouseEvent", {"type": kind, "x": x, "y": y, "button": "left", "buttons": 1 if kind != "mouseReleased" else 0,
                                     "clickCount": clicks})


def key(k, code=None, shift=False, vk=None):
    p = {"key": k, "code": code or k, "modifiers": 8 if shift else 0}
    if vk:
        p["windowsVirtualKeyCode"] = vk
    cdp("Input.dispatchKeyEvent", dict(p, type="rawKeyDown"))
    cdp("Input.dispatchKeyEvent", dict(p, type="keyUp"))
    js(SETTLE)


def center(sel):
    return js(f"(() => {{ const r = document.querySelector({json.dumps(sel)}).getBoundingClientRect(); return [r.left + r.width / 2, r.top + r.height / 2, r.top]; }})()")


try:
    connect()
    cdp("Emulation.setDeviceMetricsOverride", {"width": 1440, "height": 900, "deviceScaleFactor": 1, "mobile": False})
    wait_js("!!window.__m10 && !!window.__m10.dockSize && !!document.querySelector('[data-step]')", 60)
    js("window.__m10.idle(300000)")
    rows = js("window.__m10.runsRows()")
    run = next(r["run"] for r in rows if r["status"] == "OK")
    receipt["run"] = run
    js(f"window.__m10.selectRun({json.dumps(run)})")
    js("window.__m10.setStep('results')")
    js("window.__m10.dockTab('acoustics')")
    t0 = time.time()
    wait_js(f"(() => {{ const v = window.__m10.acousticsView(); return v && v.run === {json.dumps(run)} && v.state === 'ready'; }})()", 900, 1)
    log(f"Acoustics ready on {run} after {time.time() - t0:.1f} s")
    js(SETTLE)
    first = js(SIZE)
    stored0 = js("localStorage.getItem('nm-dock-height')")
    want0 = int(stored0) if stored0 and stored0.isdigit() else 250
    check("Results opens the dock at the last stored height (250 with none stored)", first["height"] == want0 and not first["folded"] and not first["max"],
          {"size": first, "stored": stored0})
    if first["height"] != 250:
        js("window.__m10.dockSize(250)")
        js(SETTLE)

    # 1. A pointer drag of the top edge: the 3D view's region follows each move before release.
    gx, gy, _ = center(".dock-grip")
    mouse("mousePressed", gx, gy)
    live = []
    for i in range(1, 9):
        mouse("mouseMoved", gx, gy - 30 * i)
        js(FRAMES)
        live.append(js(STATE))
    mouse("mouseReleased", gx, gy - 240)
    js(SETTLE)
    after_drag = js(SIZE)
    views = [s["view"] for s in live]
    check("drag: the 3D view resizes live, before release", all(b < a for a, b in zip(views, views[1:])) and live[0]["dock"] > 250,
          {"samples": live, "after": after_drag})
    check("drag: the height follows the pointer (250 + 240)", after_drag["height"] == 490 and after_drag["stored"] == 490, after_drag)

    # 2. Dragged to the top: maximised, the 3D view hidden; Escape restores the height before the drag.
    gx, gy, _ = center(".dock-grip")
    mouse("mousePressed", gx, gy)
    for y in (gy - 100, gy - 300, 20):
        mouse("mouseMoved", gx, y)
        js(FRAMES)
    mouse("mouseReleased", gx, 20)
    js(SETTLE)
    top = js(STATE)
    s = js(SIZE)
    check("drag to the top: maximised over the whole work area, the 3D view hidden", s["max"] and top["covers"] and top["viewVisible"] == "hidden",
          {"size": s, "state": top})
    key("Escape", "Escape", vk=27)
    s = js(SIZE)
    check("Escape restores the height before the drag", not s["max"] and s["height"] == 490 and s["view"] > 0, s)

    # 3. The maximise button and a double-click on the tab strip toggle full height and back.
    bx, by, _ = center(".dock-max")
    mouse("mousePressed", bx, by)
    mouse("mouseReleased", bx, by)
    js(SETTLE)
    m1 = {**js(SIZE), **js(STATE)}
    bx, by, _ = center(".dock-max")
    mouse("mousePressed", bx, by)
    mouse("mouseReleased", bx, by)
    js(SETTLE)
    m2 = js(SIZE)
    check("maximise button: full window, then back to 490", m1["max"] and m1["covers"] and m1["viewVisible"] == "hidden" and not m2["max"] and m2["height"] == 490, [m1, m2])
    tx = js("(() => { const t = document.querySelector('.dock-tabs').getBoundingClientRect(); return [t.left + t.width * 0.6, t.top + t.height / 2]; })()")
    for c in (1, 2):
        mouse("mousePressed", tx[0], tx[1], c)
        mouse("mouseReleased", tx[0], tx[1], c)
    js(SETTLE)
    d1 = js(SIZE)
    tx = js("(() => { const t = document.querySelector('.dock-tabs').getBoundingClientRect(); return [t.left + t.width * 0.6, t.top + t.height / 2]; })()")
    for c in (1, 2):
        mouse("mousePressed", tx[0], tx[1], c)
        mouse("mouseReleased", tx[0], tx[1], c)
    js(SETTLE)
    d2 = js(SIZE)
    check("double-click on the tab strip: full window, then back", d1["max"] and not d2["max"] and d2["height"] == 490, [d1, d2])

    # 4. The handle with the keys.
    js("document.querySelector('.dock-grip').focus()")
    seq = []
    for k, sh in (("ArrowUp", False), ("ArrowDown", True), ("End", False), ("Home", False), ("ArrowUp", False)):
        key(k, k, shift=sh, vk={"ArrowUp": 38, "ArrowDown": 40, "End": 35, "Home": 36}[k])
        seq.append({"key": ("Shift+" if sh else "") + k, **js(SIZE)})
    exp = [(514, False, False), (418, False, False), (418, True, False), (418, False, True), (418, False, False)]
    got = [(x["stored"], x["max"], x["folded"]) for x in seq]
    check("keys: Up +24, Shift+Down -96, End maximises, Home folds, Up unfolds at the height",
          got == exp and seq[3]["height"] == 36 and seq[0]["height"] == 514 and seq[4]["height"] == 418, seq)

    # 5. Across steps: the height holds on Simulate (the 3D view in sight) and back on Results.
    js("window.__m10.dockSize(520)")
    js(SETTLE)
    js("window.__m10.setStep('simulate')")
    js(SETTLE)
    sim = js(SIZE)
    js("window.__m10.setStep('results')")
    js(SETTLE)
    res = js(SIZE)
    js("window.__m10.dockSize('max')")
    js(SETTLE)
    js("window.__m10.setStep('simulate')")
    js(SETTLE)
    sim_max = js(SIZE)
    js("window.__m10.setStep('results')")
    js(SETTLE)
    res_max = js(SIZE)
    js("window.__m10.dockSize('restore')")
    stored = js("localStorage.getItem('nm-dock-height')")
    check("across steps: the height and maximised hold; Simulate keeps the 3D view in sight when not maximised",
          sim["height"] == 520 and sim["view"] >= 120 and res["height"] == 520 and sim_max["max"] and res_max["max"] and stored == "520"
          and sim_max["stored"] == 520,
          {"simulate": sim, "results": res, "simulate_max": sim_max, "results_max": res_max, "stored": stored})

    # 6. The screenshots: folded, half the column, maximised; the Acoustics tab's layout at full window.
    # The handle loses the keyboard focus first, so its pill is drawn at rest.
    js("document.activeElement && document.activeElement.blur()")
    js("window.__m10.dockSize('fold')")
    js(SETTLE)
    time.sleep(0.5)
    receipt["min"] = js(SIZE)
    shot("dock-1-min.png")
    room = js(SIZE)["room"]
    js("window.__m10.dockSize('restore')")
    js(f"window.__m10.dockSize({round(room / 2)})")
    js(SETTLE)
    time.sleep(1)
    receipt["half"] = js(SIZE)
    shot("dock-2-half.png")
    js("window.__m10.dockSize('max')")
    js(SETTLE)
    time.sleep(1)
    receipt["full"] = {**js(SIZE), **js(STATE)}
    layout = js("""(() => {
      const body = document.querySelector('.ac-body'), w = document.querySelector('.ac-wording');
      const cards = [...document.querySelectorAll('.ac-body > .ac-card')].map(c => { const r = c.getBoundingClientRect(); return {label: c.getAttribute('aria-label'), left: Math.round(r.left), top: Math.round(r.top), width: Math.round(r.width), height: Math.round(r.height), cut: c.scrollHeight - c.clientHeight}; });
      const wr = w.getBoundingClientRect();
      return {bodyScrollWidth: body.scrollWidth, bodyClientWidth: body.clientWidth, wording: {width: Math.round(wr.width), height: Math.round(wr.height)}, cards};
    })()""")
    tops = [c["top"] for c in layout["cards"]]
    side_by_side = len(tops) - len(set(tops))
    check("full window: the Acoustics cards in a grid across the width (cards side by side, each as tall as its content, nothing clipped sideways), the wording on one or two lines",
          layout["bodyScrollWidth"] <= layout["bodyClientWidth"] and side_by_side >= 1 and all(c["cut"] <= 1 for c in layout["cards"])
          and layout["wording"]["height"] <= 40 and receipt["full"]["covers"],
          layout)
    shot("dock-3-full.png")
    js("window.__m10.dockTab('console')")
    js(FRAMES)
    time.sleep(0.5)
    receipt["full_console"] = js("window.__m10.consoleScroll()")
    shot("dock-3-full-console.png")
    js("window.__m10.dockTab('runs')")
    js(FRAMES)
    time.sleep(0.5)
    shot("dock-3-full-runs.png")
    js("window.__m10.dockTab('acoustics')")
    js("window.__m10.dockSize('restore')")
    js("window.__m10.dockSize(250)")
finally:
    with open(os.path.join(out, "bed-dock.json"), "w", encoding="utf-8") as f:
        json.dump(receipt, f, indent=1)
    if ws:
        ws.close()
    proc.terminate()
    try:
        proc.wait(10)
    except Exception:
        proc.kill()
    n = sum(1 for c in receipt["checks"] if c["ok"])
    log(f"app pid {proc.pid} stopped; {n} of {len(receipt['checks'])} checks passed; receipt {os.path.join(out, 'bed-dock.json')}")
