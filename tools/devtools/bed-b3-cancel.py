"""B3 audit fix 2 bed: a GPU run cancelled while its stream file exists leaves no spps-gpu.pstream, and the run's other
files are there. The release app.exe over the DevTools protocol only (no mouse, no focus). Also leaves the Simulate
step and comes back while the run is live (fix 3), recording the live layer each way.

usage: python bed-b3-cancel.py <app.exe> <solvers-dir> <project> <out-dir> [delay-s] [port]
  delay-s: how long after the solve stage begins the run is cancelled (default 0.35).
The project is copied into <out-dir>/app-<name>/. Needs websocket-client. Stops only the app.exe it started.
"""
import json, os, shutil, subprocess, sys, time, urllib.request
import websocket

app, solvers, project, out = sys.argv[1:5]
delay = float(sys.argv[5]) if len(sys.argv) > 5 else 0.35
port = int(sys.argv[6]) if len(sys.argv) > 6 else 9235
os.makedirs(out, exist_ok=True)
d = os.path.join(out, "app-" + os.path.splitext(os.path.basename(project))[0])
os.makedirs(d, exist_ok=True)
copy = os.path.join(d, os.path.basename(project))
if not os.path.exists(copy):
    shutil.copyfile(project, copy)
receipt = {"app": app, "project": copy, "delay_s": delay}
env = dict(os.environ, SIMPA_SOLVERS_DIR=solvers, WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=f"--remote-debugging-port={port}")
env.pop("SPPS_GPU_STREAM", None)
proc = subprocess.Popen([app, "--e2e", "--project", copy], env=env)
ws = None
msg_id = 0


def log(msg):
    line = time.strftime("%H:%M:%S") + "  " + msg
    print(line, flush=True)
    with open(os.path.join(out, "cancel-bed.log"), "a", encoding="utf-8") as f:
        f.write(line + "\n")


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


def js(expr):
    global msg_id
    msg_id += 1
    ws.send(json.dumps({"id": msg_id, "method": "Runtime.evaluate", "params": {"expression": expr, "awaitPromise": True, "returnByValue": True}}))
    while True:
        m = json.loads(ws.recv())
        if m.get("id") == msg_id:
            r = m["result"]
            if "exceptionDetails" in r:
                raise RuntimeError(json.dumps(r["exceptionDetails"])[:500])
            return r["result"].get("value")


def wait_js(expr, timeout=60, every=0.02):
    t0 = time.time()
    while time.time() - t0 < timeout:
        v = js(expr)
        if v:
            return v
        time.sleep(every)
    raise RuntimeError(f"timed out waiting for {expr}")


try:
    connect()
    wait_js("!!window.__m10 && !!document.querySelector('[data-step]')", 60, 0.5)
    js("window.__m10.setStep('simulate')")
    wait_js("!!document.querySelector('[data-solver=\"spps-gpu\"]')", 30, 0.2)
    wait_js("!document.querySelector('[data-solver=\"spps-gpu\"]').disabled", 30, 0.2)
    js("document.querySelector('[data-solver=\"spps-gpu\"]').click()")
    time.sleep(0.5)
    if js("document.querySelector('[data-solver=\"spps-gpu\"]').getAttribute('aria-checked')") != "true":
        raise SystemExit("the GPU entry is not chosen: stopping before any run")
    js("document.querySelector('[data-part=\"run-panel\"]').click()")
    st = wait_js("(() => { const r = window.__m10.runState(); return r && r.stage === 'solve' && r.run ? r : null; })()", 120)
    run = st["run"]
    solve = os.path.join(d, "runs", run, "solve")
    stream = os.path.join(solve, "spps-gpu.pstream")
    time.sleep(delay)
    receipt["stream_before_cancel"] = os.path.exists(stream)
    receipt["stream_bytes_before_cancel"] = os.path.getsize(stream) if os.path.exists(stream) else None
    receipt["cancel"] = js("window.__m10.runCancel()")
    log(f"run {run}: stream before cancel {receipt['stream_before_cancel']} ({receipt['stream_bytes_before_cancel']} bytes); cancel {receipt['cancel']}")
    js(f"window.__m10.waitRun({json.dumps(run)}, 'ended', 120000)")
    time.sleep(0.5)
    rows = js("window.__m10.runsRows()")
    row = next(r for r in rows if r["run"] == run)
    receipt["row"] = {k: row.get(k) for k in ("status", "gpu_device", "exe", "reasons")}
    receipt["stream_after"] = os.path.exists(stream)
    receipt["solve_files"] = sorted(os.path.relpath(os.path.join(r, f), solve) for r, _, fs in os.walk(solve) for f in fs)
    receipt["run_folder"] = sorted(os.listdir(os.path.join(d, "runs", run)))
    receipt["live_after"] = js("window.__m10.liveView()")
    log(f"after: {json.dumps({k: receipt[k] for k in ('row', 'stream_after', 'solve_files', 'run_folder', 'live_after')})}")
finally:
    with open(os.path.join(out, "cancel-bed.json"), "w", encoding="utf-8") as f:
        json.dump(receipt, f, indent=1)
    if ws:
        ws.close()
    proc.terminate()
    try:
        proc.wait(10)
    except Exception:
        proc.kill()
