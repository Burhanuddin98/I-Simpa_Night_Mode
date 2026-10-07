"""Evaluate one JS expression in the app's WebView2 page over the DevTools protocol.
usage: python -I cdp.py <port> <expression> [timeout_s]
Prints the JSON result value (awaits a promise)."""
import json, sys, urllib.request
import websocket  # websocket-client

port, expr = sys.argv[1], sys.argv[2]
timeout = float(sys.argv[3]) if len(sys.argv) > 3 else 60.0
pages = json.load(urllib.request.urlopen(f"http://127.0.0.1:{port}/json", timeout=10))
page = next((p for p in pages if p.get("type") == "page"), None)
if page is None:
    sys.exit(f"no page target on port {port}: {pages}")
ws = websocket.create_connection(page["webSocketDebuggerUrl"], timeout=timeout, suppress_origin=True)
ws.send(json.dumps({"id": 1, "method": "Runtime.evaluate",
                    "params": {"expression": expr, "awaitPromise": True, "returnByValue": True}}))
while True:
    msg = json.loads(ws.recv())
    if msg.get("id") == 1:
        break
ws.close()
if "error" in msg:
    sys.exit(f"CDP error: {msg['error']}")
r = msg["result"]
if "exceptionDetails" in r:
    ex = r["exceptionDetails"]
    sys.exit("JS exception: " + json.dumps(ex.get("exception", {}).get("description") or ex.get("text")))
print(json.dumps(r["result"].get("value")))
