"""Which process a DevTools port belongs to (C1 audit). On 2026-10-07 at 14:19 a bed connected to
another builder's app.exe listening on the same port and drove it. The beds now refuse a port that
is already in use, and check after connecting that the page belongs to the app.exe they started."""
import subprocess


def port_owner_chain(port):
    """The pid listening on `port`, then its parent, grandparent and so on; [] when none listens."""
    ps = (
        f"$o = (Get-NetTCPConnection -LocalPort {port} -State Listen -ErrorAction SilentlyContinue | Select-Object -First 1).OwningProcess; "
        "$chain = @(); while ($o) { $chain += $o; $o = (Get-CimInstance Win32_Process -Filter \"ProcessId=$o\").ParentProcessId; if ($chain -contains $o) { break } }; $chain -join ','"
    )
    r = subprocess.run(["powershell", "-NoProfile", "-Command", ps], capture_output=True, text=True)
    return [int(x) for x in r.stdout.strip().split(",") if x.strip().isdigit()]


def refuse_if_taken(port):
    if port_owner_chain(port):
        raise SystemExit(f"port {port} is already in use: not starting, so as not to drive another app")


def require_owner(port, pid):
    chain = port_owner_chain(port)
    if pid not in chain:
        raise SystemExit(f"port {port} belongs to {chain}, not to the app.exe started ({pid}): stopping")
    return chain
