"""A5 bed items 2 and 3 (docs/investigations/2026-10-06-gpu/A5-SPEC.md): the release app.exe, driven over
the DevTools protocol only (no mouse, no focus): the Simulate step's GPU entry, a CR4 run started from the
Run button on the GPU, the Results step and the Acoustics tab naming the solver and device, then spps-gpu's
refusal of a project with a fitting shown under the last run. Screenshots and a receipt JSON in <out>.

usage: python bed-a5.py <app.exe> <solvers-dir> <cr4-project> <fitting-project> <out-dir> [port]
Each project is copied into its own folder under <out-dir> (its runs land beside the copy). Needs
websocket-client (do not run with python -I). Stops only the app.exe it started.
"""
import base64, json, os, shutil, subprocess, sys, time, urllib.request
import websocket

app, solvers, cr4, fitting, out = sys.argv[1:6]
port = int(sys.argv[6]) if len(sys.argv) > 6 else 9231
os.makedirs(out, exist_ok=True)
receipt = {"app": app, "app_mtime": time.strftime("%Y-%m-%d %H:%M:%S", time.localtime(os.path.getmtime(app))),
           "solvers": solvers, "port": port}


def log(msg):
    line = time.strftime("%H:%M:%S") + "  " + msg
    print(line, flush=True)
    with open(os.path.join(out, "app-bed.log"), "a", encoding="utf-8") as f:
        f.write(line + "\n")


def copy_project(src, sub):
    d = os.path.join(out, sub)
    os.makedirs(d, exist_ok=True)
    dst = os.path.join(d, os.path.basename(src))
    shutil.copyfile(src, dst)
    return dst


cr4_copy = copy_project(cr4, "app-cr4")
fit_copy = copy_project(fitting, "app-refusal")
env = dict(os.environ, SIMPA_SOLVERS_DIR=solvers, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=f"--remote-debugging-port={port}")
proc = subprocess.Popen([app, "--e2e", "--project", cr4_copy], env=env)
log(f"started app pid {proc.pid} on {cr4_copy}")
ws = None


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


msg_id = 0


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


CHOICES = """[...document.querySelectorAll('[data-solver]')].map(b => ({solver: b.dataset.solver, device: b.dataset.device,
  available: b.dataset.available ?? null, checked: b.getAttribute('aria-checked'), disabled: b.disabled,
  text: b.innerText.replace(/\\s+/g, ' ').trim(), title: b.title}))"""

try:
    connect()
    wait_js("!!window.__m10 && !!document.querySelector('[data-step]')", 60)
    gpu = js("window.__m10.gpuStatus()")
    receipt["gpu_status"] = gpu
    log(f"gpuStatus: {json.dumps(gpu)}")
    js("window.__m10.setStep('simulate')")
    wait_js("!!document.querySelector('[data-solver=\"spps-gpu\"]')", 30)
    time.sleep(1)
    receipt["choices_before"] = js(CHOICES)
    log(f"choices: {json.dumps(receipt['choices_before'])}")
    shot("a5-simulate-gpu-entry.png")

    # --- bed 2: choose the GPU entry and press the panel's Run button (DOM events, no mouse)
    js("document.querySelector('[data-solver=\"spps-gpu\"]').click()")
    time.sleep(0.5)
    receipt["choices_chosen"] = js(CHOICES)
    if not any(c["solver"] == "spps-gpu" and c["checked"] == "true" for c in receipt["choices_chosen"]):
        shot("a5-gpu-not-chosen.png")
        raise SystemExit("the GPU entry could not be chosen (unavailable?): stopping before any run")
    receipt["run_button"] = js("({panel: document.querySelector('[data-part=\"run-panel\"]')?.innerText, blockers: document.querySelector('[data-part=\"run-panel\"]')?.dataset.blockers, top: document.querySelector('[data-part=\"run-label\"]')?.innerText})")
    log(f"run button: {json.dumps(receipt['run_button'])}")
    shot("a5-simulate-gpu-chosen.png")
    t_click = time.time()
    js("document.querySelector('[data-part=\"run-panel\"]').click()")
    run = js("window.__m10.waitRun(null, 'ended', 900000)")
    receipt["gpu_run"] = {"run": run, "click_to_ended_s": round(time.time() - t_click, 2)}
    log(f"run ended: {run} ({receipt['gpu_run']['click_to_ended_s']} s from the click)")
    rows = js("window.__m10.runsRows()")
    row = next(r for r in rows if r["run"] == run)
    receipt["gpu_run"]["row"] = {k: row.get(k) for k in ("status", "solver", "gpu_device", "exe", "elapsed_s", "solver_build", "reasons")}
    log(f"row: {json.dumps(receipt['gpu_run']['row'])}")
    shot("a5-simulate-after-gpu-run.png")

    js(f"window.__m10.selectRun({json.dumps(run)})")
    js("window.__m10.setStep('results')")
    state = wait_js("(() => { const s = document.querySelector('[data-props-step=\"results\"] [data-results-state]')?.getAttribute('data-results-state'); return s && s !== 'checking' ? s : null; })()", 300)
    receipt["results"] = {
        "state": state,
        "solver_text": js("document.querySelector('[data-props-step=\"results\"] [data-part=\"run-solver\"]')?.innerText ?? null"),
        "sub": js("document.querySelector('[data-props-step=\"results\"] .props-head .sub')?.innerText ?? null"),
        "label": js("document.querySelector('[data-props-step=\"results\"] [data-run-label]')?.innerText ?? null"),
    }
    log(f"results: {json.dumps(receipt['results'])}")
    js("window.__m10.dockTab('acoustics')")
    ac = wait_js("(() => { const a = document.querySelector('[data-acoustics]'); const s = a?.getAttribute('data-acoustics-state'); return s === 'ready' || s === 'refused' || s === 'error' ? s : null; })()", 300)
    time.sleep(2)
    receipt["acoustics"] = {"state": ac, "solver_text": js("document.querySelector('.ac-solver')?.innerText ?? null"),
                            "solver_title": js("document.querySelector('.ac-solver')?.title ?? null")}
    log(f"acoustics: {json.dumps(receipt['acoustics'])}")
    shot("a5-results-gpu.png")
    js("window.__m10.dockTab('runs')")
    time.sleep(1.5)
    receipt["runs_tab"] = {
        "solvers": js("[...document.querySelectorAll('.c-solver')].map(e => ({text: e.innerText, device: e.dataset.device, title: e.title}))"),
        "status_bar": js("document.querySelector('[data-part=\"solvers\"]')?.innerText ?? null"),
    }
    log(f"runs tab and status bar: {json.dumps(receipt['runs_tab'])}")
    shot("a5-runs-tab.png")

    # --- bed 3: spps-gpu's refusal of a project with a fitting, shown in the app
    js(f"window.__m10.openPath({json.dumps(fit_copy)})")
    js("window.__m10.setStep('simulate')")
    wait_js("!!document.querySelector('[data-solver=\"spps-gpu\"]')", 30)
    js("document.querySelector('[data-solver=\"spps-gpu\"]').click()")
    time.sleep(0.5)
    if js("document.querySelector('[data-solver=\"spps-gpu\"]').getAttribute('aria-checked')") != "true":
        raise SystemExit("the GPU entry is not chosen on the fitting project: stopping before any run")
    receipt["refusal_run_button"] = js("({panel: document.querySelector('[data-part=\"run-panel\"]')?.innerText, blockers: document.querySelector('[data-part=\"run-panel\"]')?.dataset.blockers})")
    log(f"refusal project run button: {json.dumps(receipt['refusal_run_button'])}")
    js("document.querySelector('[data-part=\"run-panel\"]').click()")
    run2 = js("window.__m10.waitRun(null, 'ended', 300000)")
    time.sleep(1.5)
    receipt["refusal"] = {
        "run": run2,
        "status": js("document.querySelector('[data-part=\"last-status\"]')?.innerText ?? null"),
        "reasons": js("document.querySelector('[data-part=\"last-reasons\"]')?.innerText ?? null"),
        "gpu_refusal": js("document.querySelector('[data-part=\"gpu-refusal\"]')?.innerText ?? null"),
    }
    log(f"refusal: {json.dumps(receipt['refusal'])}")
    js("document.querySelector('[data-part=\"last-reasons\"]')?.scrollIntoView({block: 'center'})")
    time.sleep(0.5)
    shot("a5-refusal.png")
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
