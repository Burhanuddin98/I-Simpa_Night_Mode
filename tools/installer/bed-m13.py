"""M13's bed: the installer, proven on this machine without touching what is already on it.

usage: python tools/installer/bed-m13.py <setup.exe> <payload-dir> <out-dir> [port]

1. Silent per-user install into %LOCALAPPDATA%\\nm-m13-scratch\\<stamp>\\install, Start menu entry under a scratch name
   (/MENUNAME), no admin. Every staged file present at its sha256, nothing else but uninstall.exe; the
   ProgID, the command Windows resolves for .simpa (AssocQueryString), the shortcut's target, the
   uninstall entry.
2. Tutorial 1's installed project copied to a scratch project folder and opened by the shell's own
   "open" (os.startfile: what a double-click does). The app found by its path, the DevTools port
   (default 9488) owned by a WebView2 process under that app, the project open is the copy.
3. Help > User manual and Help > About through the page's own menu (DOM clicks, no mouse, no focus).
   $SIMPA_HELP_OPEN_LOG stands in for the browser; TMP, the WebView2 profile and the recovery folder
   are scratch, so nothing reaches the person's own.
4. An SPPS run from the Run button, through the installed solvers: run.json names the installed
   spps.exe, its verdict, solver.stderr.txt empty; the C runtime the solver loaded is the app-local
   copy (decision 85).
5. Silent uninstall: the install folder gone, the shortcut, ProgID, uninstall entry gone, .simpa as it
   was before, the scratch project and its runs untouched.
6. A second install and uninstall with a user's project and run planted inside the install folder:
   they survive, and only they.
Receipt: <out>\\bed-m13.json and bed-m13.log. Needs websocket-client (do not run with python -I).
"""
import ctypes, hashlib, json, os, shutil, subprocess, sys, time, urllib.request, winreg
import websocket

installer, payload, out = os.path.abspath(sys.argv[1]), os.path.abspath(sys.argv[2]), os.path.abspath(sys.argv[3])
port = int(sys.argv[4]) if len(sys.argv) > 4 else 9488
os.makedirs(out, exist_ok=True)
LOCAL, ROAMING = os.environ["LOCALAPPDATA"], os.environ["APPDATA"]
# A fresh folder per run: the bed deletes nothing of its own; only the uninstaller under test removes files.
scratch = os.path.join(LOCAL, "nm-m13-scratch", time.strftime("%Y%m%d-%H%M%S"))
instdir = os.path.join(scratch, "install")
projdir = os.path.join(scratch, "project")
MENU = "NM-M13-scratch"
lnk = os.path.join(ROAMING, "Microsoft", "Windows", "Start Menu", "Programs", MENU + ".lnk")
PROGID = "DockyardNightMode.simpa"
UNINST = r"Software\Microsoft\Windows\CurrentVersion\Uninstall\I-Simpa Night Mode"
CLASSES = r"Software\Classes"
FILEEXTS = r"Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.simpa"
receipt = {"installer": installer, "payload": payload, "port": port, "scratch": scratch, "checks": []}
logf = os.path.join(out, "bed-m13.log")


def log(msg):
    line = time.strftime("%H:%M:%S") + "  " + msg
    print(line, flush=True)
    with open(logf, "a", encoding="utf-8") as f:
        f.write(line + "\n")


def check(cid, ok, detail):
    receipt["checks"].append({"id": cid, "ok": bool(ok), "detail": detail})
    log(f"{'PASS' if ok else 'FAIL'} {cid}: {json.dumps(detail, default=str)[:600]}")
    return ok


def sha(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for b in iter(lambda: f.read(1 << 20), b""):
            h.update(b)
    return h.hexdigest()


def reg(path, name=""):
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, path) as k:
            return winreg.QueryValueEx(k, name)[0]
    except OSError:
        return None


def reg_key(path):
    try:
        winreg.CloseKey(winreg.OpenKey(winreg.HKEY_CURRENT_USER, path))
        return True
    except OSError:
        return False


def reg_values(path):
    try:
        with winreg.OpenKey(winreg.HKEY_CURRENT_USER, path) as k:
            vals, i = {}, 0
            while True:
                try:
                    n, v, _ = winreg.EnumValue(k, i)
                except OSError:
                    return vals
                vals[n] = v
                i += 1
    except OSError:
        return None


def assoc_command(ext=".simpa"):
    """The command the shell runs to open ext (UserChoice, HKCU and HKLM merged), or None."""
    n = ctypes.c_uint32(0)
    shl = ctypes.windll.shlwapi
    if shl.AssocQueryStringW(0, 1, ext, "open", None, ctypes.byref(n)) not in (0, 1) or n.value == 0:
        return None
    buf = ctypes.create_unicode_buffer(n.value)
    if shl.AssocQueryStringW(0, 1, ext, "open", buf, ctypes.byref(n)) != 0:
        return None
    return buf.value


def assoc_state():
    return {"simpa_default": reg(CLASSES + r"\.simpa"), "simpa_key": reg_key(CLASSES + r"\.simpa"),
            "openwith": reg_values(CLASSES + r"\.simpa\OpenWithProgids"), "progid_key": reg_key(CLASSES + "\\" + PROGID),
            "fileexts_openwith": reg_values(FILEEXTS + r"\OpenWithProgids"), "command": assoc_command()}


def processes():
    ps = ("Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId, Name, ExecutablePath, CommandLine"
          " | ConvertTo-Json -Compress")
    r = subprocess.run(["powershell", "-NoProfile", "-NonInteractive", "-Command", ps], capture_output=True, text=True, timeout=120)
    return {p["ProcessId"]: p for p in json.loads(r.stdout)}


def listener(p):
    r = subprocess.run(["netstat", "-ano", "-p", "TCP"], capture_output=True, text=True)
    for line in r.stdout.splitlines():
        f = line.split()
        if len(f) >= 5 and f[3] == "LISTENING" and f[1].endswith(f":{p}"):
            return int(f[4])
    return None


def tree_files(root):
    return sorted(os.path.relpath(os.path.join(d, f), root) for d, _, fs in os.walk(root) for f in fs)


def install(tag):
    t0 = time.time()
    r = subprocess.run([installer, "/S", f"/MENUNAME={MENU}", f"/D={instdir}"], timeout=900)
    log(f"install ({tag}): exit {r.returncode} in {time.time() - t0:.1f} s")
    return r.returncode


def uninstall(tag):
    t0 = time.time()
    u = os.path.join(instdir, "uninstall.exe")
    r = subprocess.run([u, "/S"], timeout=300)
    # NSIS's uninstaller copies itself to %TEMP% and runs from there: wait for its work to finish.
    while time.time() - t0 < 300 and (os.path.exists(u) or reg_key(UNINST)):
        time.sleep(0.5)
    time.sleep(2)
    log(f"uninstall ({tag}): exit {r.returncode}, done in {time.time() - t0:.1f} s")
    return r.returncode


ws, app_pid, msg_id = None, None, 0


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


def wait(fn, timeout, every=0.5, what=""):
    t0 = time.time()
    while time.time() - t0 < timeout:
        v = fn()
        if v:
            return v
        time.sleep(every)
    raise RuntimeError(f"timed out after {timeout} s waiting for {what}")


INVOKE = "window.__TAURI_INTERNALS__.invoke"


def menu(item):
    js("document.querySelector('[data-menu=\"Help\"]').click()")
    wait(lambda: js(f"!!document.querySelector('[data-menu-item=\"{item}\"]')"), 10, what=f"Help > {item}")
    js(f"document.querySelector('[data-menu-item=\"{item}\"]').click()")


# ---- 0. what is here before anything is installed
os.makedirs(scratch)
if reg_key(UNINST):
    sys.exit("an I-Simpa Night Mode install is registered for this user: the bed would replace its entry; stopping")
if os.path.exists(lnk):
    sys.exit(f"{lnk} exists: stopping")
if listener(port):
    sys.exit(f"port {port} is already listening (pid {listener(port)}): not ours; stopping")
before = assoc_state()
receipt["assoc_before"] = before
log(f"before: {json.dumps(before)}")
receipt["installer_sha256"] = sha(installer)
receipt["installer_bytes"] = os.path.getsize(installer)

try:
    # ---- 1. install
    rc = install("first")
    check("install-exit", rc == 0, rc)
    want = tree_files(payload)
    got = tree_files(instdir)
    check("install-files", got == sorted(want + ["uninstall.exe"]),
          {"payload": len(want), "installed": len(got), "extra": sorted(set(got) - set(want)),
           "missing": sorted(set(want) - set(got))})
    diff = [f for f in want if os.path.exists(os.path.join(instdir, f)) and sha(os.path.join(instdir, f)) != sha(os.path.join(payload, f))]
    check("install-hashes", not diff, {"differ": diff})
    app = os.path.join(instdir, "app.exe")
    cmd_want = f'"{app}" --project "%1"'
    a = assoc_state()
    receipt["assoc_installed"] = a
    check("a42-progid", a["simpa_default"] == PROGID and reg(CLASSES + "\\" + PROGID + r"\shell\open\command") == cmd_want, a)
    check("a42-resolved", (a["command"] or "").lower() == cmd_want.lower(), {"AssocQueryString": a["command"], "want": cmd_want})
    target = subprocess.run(["powershell", "-NoProfile", "-Command", f"(New-Object -ComObject WScript.Shell).CreateShortcut('{lnk}').TargetPath"],
                            capture_output=True, text=True).stdout.strip()
    check("start-menu", os.path.exists(lnk) and target.lower() == app.lower(), {"lnk": lnk, "target": target})
    entry = reg_values(UNINST) or {}
    check("uninstall-entry", entry.get("InstallLocation", "").lower() == instdir.lower() and bool(entry.get("DisplayVersion")),
          {k: entry.get(k) for k in ("DisplayName", "DisplayVersion", "Comments", "InstallLocation", "QuietUninstallString", "EstimatedSize")})
    version = entry.get("DisplayVersion")

    # ---- 2. a bundled tutorial, opened the way a double-click opens it
    os.makedirs(projdir)
    proj = os.path.join(projdir, "tutorial_1.simpa")
    shutil.copyfile(os.path.join(instdir, "tutorials", "tutorial_1.simpa"), proj)
    for sub in ("tmp", "webview2", "recovery"):
        os.makedirs(os.path.join(scratch, sub))
    helplog = os.path.join(scratch, "help-open.txt")
    env_set = {"WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS": f"--remote-debugging-port={port}",
               "WEBVIEW2_USER_DATA_FOLDER": os.path.join(scratch, "webview2"),
               "SIMPA_RECOVERY_DIR": os.path.join(scratch, "recovery"), "SIMPA_HELP_OPEN_LOG": helplog,
               "TMP": os.path.join(scratch, "tmp"), "TEMP": os.path.join(scratch, "tmp")}
    os.environ.update(env_set)
    os.environ.pop("SIMPA_SOLVERS_DIR", None)  # the installed solvers, found beside app.exe
    receipt["env"] = env_set
    os.startfile(proj)
    log(f"shell open: {proj}")

    def find_app():
        for p in processes().values():
            if (p.get("ExecutablePath") or "").lower() == app.lower():
                return p
        return None
    p = wait(find_app, 60, 1, "the installed app.exe")
    app_pid = p["ProcessId"]
    check("a42-launch", "--project" in (p.get("CommandLine") or "") and proj.lower() in (p.get("CommandLine") or "").lower(),
          {"pid": app_pid, "cmdline": p.get("CommandLine")})
    lpid = wait(lambda: listener(port), 60, 1, f"DevTools on {port}")
    procs = processes()
    chain, cur = [], lpid
    while cur in procs and len(chain) < 10:
        chain.append({"pid": cur, "name": procs[cur]["Name"]})
        if cur == app_pid:
            break
        cur = procs[cur]["ParentProcessId"]
    check("devtools-port-ours", any(c["pid"] == app_pid for c in chain), {"port": port, "listener": lpid, "chain": chain})

    pages = wait(lambda: [x for x in json.load(urllib.request.urlopen(f"http://127.0.0.1:{port}/json", timeout=5)) if x.get("type") == "page"], 60, 1, "a page")
    ws = websocket.create_connection(pages[0]["webSocketDebuggerUrl"], timeout=900, suppress_origin=True)
    wait(lambda: js("!!document.querySelector('[data-step]')"), 60, what="the app's page")
    info = wait(lambda: js(f"{INVOKE}('project_info')"), 60, 1, "the project")
    check("a42-opened", os.path.normcase(info.get("path") or "") == os.path.normcase(proj),
          {k: info.get(k) for k in ("name", "path", "faces", "sources", "point_receivers")})
    check("webview2-profile-scratch", len(os.listdir(os.path.join(scratch, "webview2"))) > 0, os.listdir(os.path.join(scratch, "webview2")))

    # ---- 3. Help > User manual, Help > About
    menu("help-manual")
    line = wait(lambda: os.path.exists(helplog) and open(helplog, encoding="utf-8").read().strip(), 20, what="the manual's open")
    page = line.splitlines()[-1]
    same = os.path.exists(page) and open(page, "rb").read() == open(os.path.join(instdir, "manual", "manual.html"), "rb").read()
    check("help-manual", page.endswith("manual.html") and same and page.lower().startswith(os.path.join(scratch, "tmp").lower()),
          {"opened": page, "same_as_installed_manual": same})
    menu("help-about")
    about_text = wait(lambda: js("document.querySelector('[data-part=\"about-dialog\"]')?.innerText ?? null"), 20, what="About")
    about = js(f"{INVOKE}('app_about')")
    build = js("document.querySelector('[data-field=\"about.build\"]')?.innerText ?? null")
    check("about", bool(version) and version in (build or "") and about.get("app_version") == version,
          {"build_line": build, "app_about": about})
    js("document.querySelector('[data-part=\"about-close\"]').click()")

    # ---- 4. SPPS from the Run button, through the installed solvers
    js("document.querySelector('[data-step=\"simulate\"]').click()")
    wait(lambda: js("!!document.querySelector('[data-part=\"run-panel\"]')"), 30, what="the Simulate step")
    if js("document.querySelector('[data-solver=\"spps\"]')?.getAttribute('aria-checked')") != "true":
        js("document.querySelector('[data-solver=\"spps\"]').click()")
        time.sleep(0.5)
    receipt["run_panel"] = js("({solver: document.querySelector('[data-solver=\"spps\"]')?.getAttribute('aria-checked'), panel: document.querySelector('[data-part=\"run-panel\"]')?.innerText, blockers: document.querySelector('[data-part=\"run-panel\"]')?.dataset.blockers})")
    log(f"run panel: {json.dumps(receipt['run_panel'])}")
    mods_file = os.path.join(scratch, "spps-modules.txt")
    watcher = subprocess.Popen(["powershell", "-NoProfile", "-NonInteractive", "-Command",
        f"$t=[DateTime]::Now; while(([DateTime]::Now-$t).TotalSeconds -lt 900) {{ $p = Get-Process spps -ErrorAction SilentlyContinue | Where-Object {{ $_.Path -like '{instdir}\\solvers\\*' }} | Select-Object -First 1; "
        f"if ($p) {{ Start-Sleep -Milliseconds 300; try {{ $p.Modules | ForEach-Object FileName | Set-Content -Encoding utf8 '{mods_file}'; break }} catch {{}} }}; Start-Sleep -Milliseconds 100 }}"])
    t_click = time.time()
    js("document.querySelector('[data-part=\"run-panel\"]').click()")
    view = wait(lambda: (lambda v: v if v["rows"] and not v.get("active") else None)(js(f"{INVOKE}('runs_list')")), 900, 2, "the run's end")
    receipt["run_seconds"] = round(time.time() - t_click, 1)
    row = view["rows"][-1] if len(view["rows"]) == 1 else max(view["rows"], key=lambda r: r["number"])
    run_dir = os.path.join(view["root"], row["run"])
    manifest = json.load(open(os.path.join(run_dir, "run.json"), encoding="utf-8"))
    stderr = open(os.path.join(run_dir, "solver.stderr.txt"), "rb").read()
    exe = manifest["exe"]["path"]
    check("spps-run", row["status"] == "OK" and str(manifest["verdict"]["status"]).upper() == "OK" and row.get("solver") == "spps",
          {"row_status": row["status"], "stage": manifest.get("stage"), "exit_class": manifest.get("exit_class"),
           "verdict": manifest.get("verdict"), "particles": manifest.get("particles"), "seconds": receipt["run_seconds"], "run_dir": run_dir})
    check("spps-installed-solver", os.path.normcase(exe) == os.path.normcase(os.path.join(instdir, "solvers", "spps.exe")), {"exe": manifest["exe"]})
    check("spps-stderr-empty", len(stderr) == 0, {"bytes": len(stderr), "head": stderr[:300].decode("utf-8", "replace")})
    watcher.wait(30)
    mods = open(mods_file, encoding="utf-8-sig").read().split() if os.path.exists(mods_file) else []
    crt = [m for m in mods if os.path.basename(m).lower() in ("msvcp140.dll", "vcruntime140.dll", "vcruntime140_1.dll")]
    check("crt-app-local", len(crt) == 3 and all(os.path.dirname(m).lower() == os.path.join(instdir, "solvers").lower() for m in crt), {"loaded": crt})

    # ---- close the app: its own quit, then the process if it is still there
    try:
        js(f"{INVOKE}('app_quit')")
    except Exception as e:
        log(f"app_quit: {e}")
    ws.close()
    ws = None
    try:
        wait(lambda: app_pid not in processes(), 20, 1, "the app to exit")
    except RuntimeError:
        subprocess.run(["taskkill", "/PID", str(app_pid), "/T", "/F"], capture_output=True)
        log("app did not exit on app_quit: killed")
    time.sleep(2)
    proj_before = tree_files(projdir)
    proj_sha = sha(proj)

    # ---- 5. uninstall
    rc = uninstall("first")
    check("uninstall-exit", rc == 0, rc)
    check("uninstall-folder-gone", not os.path.exists(instdir), tree_files(instdir) if os.path.exists(instdir) else [])
    check("uninstall-start-menu-gone", not os.path.exists(lnk), lnk)
    check("uninstall-entry-gone", not reg_key(UNINST), UNINST)
    after = assoc_state()
    receipt["assoc_after"] = after
    check("uninstall-a42-gone", after == before, {"before": before, "after": after})
    check("uninstall-project-untouched", tree_files(projdir) == proj_before and sha(proj) == proj_sha,
          {"files": len(proj_before), "runs": sorted({f.split(os.sep)[1] for f in proj_before if f.startswith("runs" + os.sep)})})

    # ---- 6. a user's project and run inside the install folder survive an uninstall
    rc = install("second")
    planted = [os.path.join("tutorials", "my room.simpa"), os.path.join("tutorials", "runs", "run-1", "run.json")]
    for f in planted:
        os.makedirs(os.path.dirname(os.path.join(instdir, f)), exist_ok=True)
        with open(os.path.join(instdir, f), "w") as h:
            h.write("a user's file\n")
    uninstall("second")
    left = tree_files(instdir) if os.path.exists(instdir) else []
    check("uninstall-keeps-user-files", sorted(left) == sorted(planted), {"left": left})
    check("uninstall-second-clean", not os.path.exists(lnk) and not reg_key(UNINST) and assoc_state() == before, {})
finally:
    if ws:
        ws.close()
    if app_pid and app_pid in processes():
        subprocess.run(["taskkill", "/PID", str(app_pid), "/T", "/F"], capture_output=True)
    receipt["passed"] = sum(c["ok"] for c in receipt["checks"])
    receipt["total"] = len(receipt["checks"])
    with open(os.path.join(out, "bed-m13.json"), "w", encoding="utf-8") as f:
        json.dump(receipt, f, indent=1, default=str)
    log(f"{receipt['passed']}/{receipt['total']} checks passed; receipt {os.path.join(out, 'bed-m13.json')}")
