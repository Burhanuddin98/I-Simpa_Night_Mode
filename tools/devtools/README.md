# devtools: driving the app headless (banked from the 2026-10-06 session's scratchpad)

- `cdp.py <port> "<js>" [timeout]`: evaluates JS in the app's WebView2 page over the DevTools protocol (awaits
  promises). The app must be started with `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=<port>`;
  the websocket sends no Origin header (`suppress_origin`), or WebView2 answers 403. Needs `websocket-client`
  (user site of Python 3.13 on Grace); do not run with `python -I`.
- `cdp-shot.py <port> <out.png>`: a screenshot of the page, no window focus, no mouse.
- `bed-cr4-27-v2.ps1`: app `--e2e` on CR4-27, Results step via `window.__m10.setStep('results')`, memory sampled.
- `bed-a5.py <app.exe> <solvers> <cr4.simpa> <fitting.simpa> <out>`: A5's app bed (SPPS on the GPU): the Simulate
  step's GPU entry, a CR4 run from the Run button, the Results step and Acoustics tab naming solver and device,
  spps-gpu's refusal of a fitting; screenshots and `app-bed.json` in `<out>`. Stops only the app it started.
- `bed-dock.py <app.exe> <solvers> <project.simpa> <out> [port]`: C2's bed, the bottom dock dragged, maximised,
  keyed and carried across steps on a project's results, in a WebView2 profile of its own
  (`WEBVIEW2_USER_DATA_FOLDER`, beside the target dir) so nothing it stores reaches another app.exe; the pointer
  and keys are CDP input events, no OS mouse. Screenshots at three heights and `bed-dock.json` in `<out>`.
- `shot-rw.ps1`: the response window's screenshots (default, wheel zoom, span 100 + bin 5).
- `package-zeph.ps1 -Sha <sha>`: app.exe + verified solvers into OneDrive for Zeph.
- `du.py <root> <depth> <minGB>`: folder sizes, junctions skipped.

The app is built with `npx --no-install tauri build --no-bundle` in `app/` (CARGO_TARGET_DIR set); a plain
`cargo build -p app` is a dev-mode binary with no UI embedded. Never click in Burhan's window: drive the page.
