"""C1 audit, fix 4: a source typed onto an edge of the box's tetrahedral mesh, and one at its exact
centroid, each run with SPPS on the CPU from the release app: the verdict must be FAIL with
`particle_loss_excess` (never OK), and its reason must name the source. A control off the edge.

The box's mesh (TetGen -pq5 -A -n -Y, six tetrahedra) has one interior edge, the diagonal from
(0, 0, 0) to (6, 10, 3); the centroid (3, 5, 1.5) lies on it, and so does (1.8, 3, 0.9).

usage: python bed_edge.py <app.exe> <solvers-dir> <saved box.simpa> <out-dir> [port]
The project is copied per case into <out-dir>; the app is driven over the DevTools protocol, and
the bed checks that the page it drives belongs to the app.exe it started (another app on the same
port was driven once, 2026-10-07 14:19). Writes <out-dir>/bed_edge.json.
"""
import json, os, shutil, subprocess, sys, time, urllib.request
import websocket
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from bedport import refuse_if_taken, require_owner

app, solvers, src, out = sys.argv[1:5]
port = int(sys.argv[5]) if len(sys.argv) > 5 else 9263
os.makedirs(out, exist_ok=True)
CASES = [("centroid", [3, 5, 1.5]), ("on-edge", [1.8, 3, 0.9]), ("off-edge", [2.1, 3.7, 1.33])]


refuse_if_taken(port)
env = dict(os.environ, SIMPA_SOLVERS_DIR=solvers, WEBVIEW2_USER_DATA_FOLDER="C:/tmp/nm-target-blank/webview2",
           WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=f"--remote-debugging-port={port}")
proc = subprocess.Popen([app, "--e2e"], env=env)
ws, mid = None, 0
R = {"app": app, "app_pid": proc.pid, "cases": []}


def cdp(method, params=None):
    global mid
    mid += 1
    ws.send(json.dumps({"id": mid, "method": method, "params": params or {}}))
    while True:
        m = json.loads(ws.recv())
        if m.get("id") == mid:
            if "error" in m:
                raise RuntimeError(m["error"])
            return m["result"]


def js(expr):
    r = cdp("Runtime.evaluate", {"expression": expr, "awaitPromise": True, "returnByValue": True})
    if "exceptionDetails" in r:
        raise RuntimeError(json.dumps(r["exceptionDetails"].get("exception", {}).get("description")))
    return r["result"].get("value")


def hook(name, *a):
    return js(f"window.__m10[{json.dumps(name)}](...{json.dumps(list(a))})")


try:
    t0 = time.time()
    while ws is None:
        try:
            pages = json.load(urllib.request.urlopen(f"http://127.0.0.1:{port}/json", timeout=5))
            page = next((p for p in pages if p.get("type") == "page"), None)
            if page:
                ws = websocket.create_connection(page["webSocketDebuggerUrl"], timeout=1200, suppress_origin=True)
        except Exception:
            if time.time() - t0 > 60:
                raise
            time.sleep(0.5)
    R["port_owner_chain"] = require_owner(port, proc.pid)
    while not js("!!window.__m10 && !!document.querySelector('[data-step]')"):
        time.sleep(0.3)
    for label, pos in CASES:
        d = os.path.join(out, label)
        os.makedirs(d, exist_ok=True)
        p = os.path.join(d, "box.simpa")
        shutil.copyfile(src, p)
        hook("openProject", p)
        sid = json.loads(hook("projectJson"))["sources"][0]["id"]
        r = hook("edit", {"op": "move_source", "id": sid, "position": pos})
        hook("saveAs", p)
        hook("runStart", "spps", "cpu")
        name = hook("waitRun", None, "ended", 900000)
        row = next(x for x in hook("runsRows") if x["run"] == name)
        run = json.load(open(os.path.join(d, "runs", name, "run.json")))
        bands = (run.get("particles") or {}).get("bands") or []
        lost = [round(100 * (b["lost_by_infinite_loops"] + b["lost_by_meshing_problems"]) / b["total"], 3) for b in bands]
        stderr = open(os.path.join(d, "runs", name, "solver.stderr.txt"), encoding="utf-8", errors="replace").read()
        case = {"case": label, "position": pos, "edit_applied": r["applied"], "run": name, "status": row["status"],
                "reasons": run["verdict"]["reasons"], "lost_percent_per_band": lost, "solver_stderr": stderr[-500:]}
        R["cases"].append(case)
        print(json.dumps(case)[:900], flush=True)
finally:
    json.dump(R, open(os.path.join(out, "bed_edge.json"), "w", encoding="utf-8"), indent=1)
    if ws:
        ws.close()
    proc.terminate()
    try:
        proc.wait(10)
    except Exception:
        proc.kill()
