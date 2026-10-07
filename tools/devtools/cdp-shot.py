"""Screenshot of the app's WebView2 page over the DevTools protocol (no window focus, no mouse).
usage: python cdp-shot.py <port> <out.png>"""
import base64, json, sys, urllib.request
import websocket

port, out = sys.argv[1], sys.argv[2]
pages = json.load(urllib.request.urlopen(f"http://127.0.0.1:{port}/json", timeout=10))
page = next(p for p in pages if p.get("type") == "page")
ws = websocket.create_connection(page["webSocketDebuggerUrl"], timeout=60, suppress_origin=True)
ws.send(json.dumps({"id": 1, "method": "Page.captureScreenshot", "params": {"format": "png"}}))
while True:
    msg = json.loads(ws.recv())
    if msg.get("id") == 1:
        break
ws.close()
if "error" in msg:
    sys.exit(f"CDP error: {msg['error']}")
data = base64.b64decode(msg["result"]["data"])
open(out, "wb").write(data)
print(f"{out}: {len(data)} bytes")
