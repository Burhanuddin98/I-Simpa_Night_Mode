"""B3 bed (docs/investigations/2026-10-06-gpu/B3-SPEC.md): the running GPU solve on the Simulate step. The release
app.exe, driven over the DevTools protocol only (no mouse, no focus): a project run from the Run button on the GPU,
the live layer sampled every 200 ms while it runs (the liveView hook: what arrived, what is drawn, the clock), three
screenshots at three moments of the run, the frame rate while live, then the layer cleared at the run's end and the
Results step's particles shown as before. Screenshots and a receipt JSON in <out>.

usage: python bed-b3.py <app.exe> <solvers-dir> <project> <out-dir> [look] [port]
  look: dots, glow or rays (the live layer's look, chosen on the caption; default glow).
The project is copied into <out-dir>/app-<name>/ (its runs land beside the copy). Needs websocket-client (do not
run with python -I). Stops only the app.exe it started.
"""
import base64, json, os, shutil, subprocess, sys, time, urllib.request
import websocket

app, solvers, project, out = sys.argv[1:5]
look = sys.argv[5] if len(sys.argv) > 5 else "glow"
port = int(sys.argv[6]) if len(sys.argv) > 6 else 9233
os.makedirs(out, exist_ok=True)
receipt = {"app": app, "app_mtime": time.strftime("%Y-%m-%d %H:%M:%S", time.localtime(os.path.getmtime(app))),
           "solvers": solvers, "project": project, "look": look, "port": port}


def log(msg):
    line = time.strftime("%H:%M:%S") + "  " + msg
    print(line, flush=True)
    with open(os.path.join(out, "app-bed.log"), "a", encoding="utf-8") as f:
        f.write(line + "\n")


d = os.path.join(out, "app-" + os.path.splitext(os.path.basename(project))[0])
os.makedirs(d, exist_ok=True)
copy = os.path.join(d, os.path.basename(project))
if not os.path.exists(copy):
    shutil.copyfile(project, copy)
env = dict(os.environ, SIMPA_SOLVERS_DIR=solvers, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=f"--remote-debugging-port={port}")
proc = subprocess.Popen([app, "--e2e", "--project", copy], env=env)
log(f"started app pid {proc.pid} on {copy}")
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


def cdp(method, params):
    global msg_id
    msg_id += 1
    ws.send(json.dumps({"id": msg_id, "method": method, "params": params}))
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


def shot(name):
    data = base64.b64decode(cdp("Page.captureScreenshot", {"format": "png"})["data"])
    p = os.path.join(out, name)
    open(p, "wb").write(data)
    log(f"screenshot {p} ({len(data)} bytes)")
    return p


def wait_js(expr, timeout=120, every=0.5):
    t0 = time.time()
    while time.time() - t0 < timeout:
        v = js(expr)
        if v:
            return v
        time.sleep(every)
    raise RuntimeError(f"timed out waiting for {expr}")


LIVE = "window.__m10.liveView()"
CAPTION = "document.querySelector('[data-part=\"live-caption-text\"]')?.innerText ?? null"
# frames drawn per second while live: rAF callbacks counted over one second
FPS = "new Promise(r => { let n = 0; const t0 = performance.now(); const f = () => { n++; if (performance.now() - t0 < 1000) requestAnimationFrame(f); else r(n * 1000 / (performance.now() - t0)); }; requestAnimationFrame(f); })"

try:
    connect()
    wait_js("!!window.__m10 && !!document.querySelector('[data-step]')", 60)
    receipt["gpu_status"] = js("window.__m10.gpuStatus()")
    log(f"gpuStatus: {json.dumps(receipt['gpu_status'])}")
    js("window.__m10.setStep('simulate')")
    wait_js("!!document.querySelector('[data-solver=\"spps-gpu\"]')", 30)
    js("document.querySelector('[data-solver=\"spps-gpu\"]').click()")
    time.sleep(0.5)
    if js("document.querySelector('[data-solver=\"spps-gpu\"]').getAttribute('aria-checked')") != "true":
        shot("b3-gpu-not-chosen.png")
        raise SystemExit("the GPU entry could not be chosen: stopping before any run")
    receipt["before"] = {"live": js(LIVE), "caption": js(CAPTION)}
    t_click = time.time()
    js("document.querySelector('[data-part=\"run-panel\"]').click()")
    samples, shots, fps = [], [], []
    looked = False
    run_name = None
    while True:
        s = js(f"({{live: {LIVE}, caption: {CAPTION}, run: window.__m10.runState ? window.__m10.runState() : null}})")
        t = round(time.time() - t_click, 3)
        ui = (s["live"] or {}).get("ui")
        if s["run"] and s["run"].get("run"):
            run_name = s["run"]["run"]
        samples.append({"t": t, "caption": s["caption"], "summary": ui and ui.get("summary"), "look": ui and ui.get("look"),
                        "view": (s["live"] or {}).get("view"), "clock": (s["live"] or {}).get("clock")})
        if ui and not looked and look != "dots":
            js(f"document.querySelector('[data-live-look=\"{look}\"]')?.click()")
            looked = True
        summ = ui and ui.get("summary")
        if summ:
            frac = (summ["bandIndex"] + 1) / max(1, summ["bands"])
            moment = None
            if not shots and summ["soFar"] > 0:
                moment = "early"
            elif len(shots) == 1 and frac >= 0.5:
                moment = "middle"
            elif len(shots) == 2 and frac >= 0.9:
                moment = "late"
            if moment:
                time.sleep(0.6)  # one rebuild and a few frames of the clock after the batch
                shots.append({"moment": moment, "t": round(time.time() - t_click, 3), "caption": js(CAPTION), "live": js(LIVE),
                              "png": shot(f"b3-live-{len(shots) + 1}-{moment}.png")})
                fps.append({"t": round(time.time() - t_click, 3), "fps": js(FPS)})
        if not ui and samples and any(x["summary"] for x in samples):
            break
        if time.time() - t_click > 900:
            raise SystemExit("the run did not end within 900 s")
        if not ui and run_name and not s["run"] and not any(x["summary"] for x in samples):
            break  # the run ended and nothing live ever arrived
        time.sleep(0.2)
    receipt["samples"] = samples
    receipt["screenshots"] = shots
    receipt["fps_while_live"] = fps
    run = js(f"window.__m10.waitRun({json.dumps(run_name)}, 'ended', 900000)")
    receipt["run"] = {"run": run, "click_to_ended_s": round(time.time() - t_click, 2)}
    rows = js("window.__m10.runsRows()")
    row = next(r for r in rows if r["run"] == run)
    receipt["run"]["row"] = {k: row.get(k) for k in ("status", "solver", "gpu_device", "exe", "elapsed_s", "solver_build", "reasons")}
    log(f"run ended: {json.dumps(receipt['run'])}")
    time.sleep(1)
    receipt["after"] = {"live": js(LIVE), "caption": js(CAPTION)}
    log(f"after the run: {json.dumps(receipt['after'])}")
    shot("b3-after-run.png")

    # the Results step's particles, as before B3: the run's saved particles load and play there
    js(f"window.__m10.selectRun({json.dumps(run)})")
    js("window.__m10.setStep('results')")
    state = wait_js("(() => { const s = document.querySelector('[data-props-step=\"results\"] [data-results-state]')?.getAttribute('data-results-state'); return s && s !== 'checking' ? s : null; })()", 300)
    time.sleep(3)
    receipt["results"] = {"state": state, "live_after": js(LIVE)}
    try:
        receipt["results"]["m12Particles"] = js("window.__m10.m12Particles ? window.__m10.m12Particles() : null")
    except Exception as e:  # noqa: BLE001
        receipt["results"]["m12Particles_error"] = str(e)
    log(f"results: {json.dumps(receipt['results'])}")
    shot("b3-results-after.png")
finally:
    with open(os.path.join(out, "app-bed.json"), "w", encoding="utf-8") as f:
        json.dump(receipt, f, indent=1)
    if ws:
        ws.close()
    proc.terminate()
    try:
        proc.wait(10)
    except Exception:
        proc.kill()
    log(f"app pid {proc.pid} stopped; receipt {os.path.join(out, 'app-bed.json')}")
