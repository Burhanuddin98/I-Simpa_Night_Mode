// Windows and processes, read from outside the page through Windows PowerShell 5.1
// (docs/investigations/2026-09-29-m11/PLAN.md 4.3): which window is in the foreground, whether
// the app's windows are visible, not minimised and on a monitor, which processes run from a
// path, and the close and kill the gate's (d) needs. Nothing here moves, resizes, hides,
// minimises or activates a window: the test windows stay as Burhan sees them.
import { spawnSync } from 'node:child_process';

/**
 * Runs `script` in Windows PowerShell and returns its stdout; throws on a non-zero exit. Each of
 * `vars` is set as a PowerShell variable of that name before the script, passed through the
 * environment, not the command line.
 */
export function ps(script: string, vars: Record<string, string> = {}): string {
  const env = { ...process.env };
  const set = Object.keys(vars).map((k) => {
    env[`M11_PS_${k}`] = vars[k];
    return `$${k} = $env:M11_PS_${k}; `;
  });
  const r = spawnSync(
    'powershell.exe',
    ['-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-Command', `$ProgressPreference='SilentlyContinue'; ${set.join('')}${script}`],
    { encoding: 'utf8', windowsHide: true, env },
  );
  if (r.status !== 0) throw new Error(`powershell exited ${r.status}: ${r.stderr}`);
  return r.stdout.trim();
}

const WIN32 = `
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class M11Windows {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr MonitorFromWindow(IntPtr h, uint flags);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  public static string Report(uint target) {
    var lines = new List<string>();
    uint fg;
    GetWindowThreadProcessId(GetForegroundWindow(), out fg);
    lines.Add("FG\\t" + fg);
    EnumWindows((h, l) => {
      uint pid;
      GetWindowThreadProcessId(h, out pid);
      if (pid != target) return true;
      var t = new StringBuilder(512);
      GetWindowText(h, t, 512);
      if (t.Length == 0) return true;
      lines.Add("WIN\\t" + (IsWindowVisible(h) ? 1 : 0) + "\\t" + (IsIconic(h) ? 1 : 0) + "\\t" +
        (MonitorFromWindow(h, 0) != IntPtr.Zero ? 1 : 0) + "\\t" + t.ToString());
      return true;
    }, IntPtr.Zero);
    return string.Join("\\n", lines);
  }
}`;

export interface AppWindow {
  visible: boolean;
  minimised: boolean;
  onMonitor: boolean;
  title: string;
}

export interface WindowState {
  /** The process that owns the foreground window. */
  foregroundPid: number;
  /** The titled top-level windows of the process asked about. */
  windows: AppWindow[];
}

/** The foreground window's process, and every titled top-level window of `pid`. */
export function windowState(pid: number): WindowState {
  const out = ps(`Add-Type -TypeDefinition @'\n${WIN32}\n'@; [M11Windows]::Report(${pid})`);
  const state: WindowState = { foregroundPid: -1, windows: [] };
  for (const line of out.split(/\r?\n/)) {
    const [kind, ...rest] = line.split('\t');
    if (kind === 'FG') state.foregroundPid = Number(rest[0]);
    if (kind === 'WIN') {
      state.windows.push({ visible: rest[0] === '1', minimised: rest[1] === '1', onMonitor: rest[2] === '1', title: rest.slice(3).join('\t') });
    }
  }
  return state;
}

const IMAGE = `
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class M11Image {
  [DllImport("kernel32.dll", SetLastError = true)] static extern IntPtr OpenProcess(uint access, bool inherit, uint pid);
  [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)] static extern bool QueryFullProcessImageNameW(IntPtr h, uint flags, StringBuilder path, ref uint size);
  [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
  public static string Path(uint pid) {
    IntPtr h = OpenProcess(0x1000, false, pid);
    if (h == IntPtr.Zero) return "";
    try {
      var s = new StringBuilder(32768);
      uint n = 32768;
      return QueryFullProcessImageNameW(h, 0, s, ref n) ? s.ToString() : "";
    } finally { CloseHandle(h); }
  }
}`;

/**
 * Ids of the processes whose executable is exactly `exe` (case-insensitive). Every process named
 * like `exe` is looked at. CIM's `ExecutablePath` is empty for a process that has never run (one
 * created suspended, as the core creates every child, and orphaned before it was resumed: M11
 * review 2, B1 and M1), so for those the kernel's image name is read (`QueryFullProcessImageNameW`,
 * set at creation). A process of that name whose path cannot be read at all is counted: it cannot
 * be shown not to run from `exe`.
 */
export function processesFrom(exe: string): number[] {
  const quote = (t: string) => `'${t.replace(/'/g, "''")}'`;
  const leaf = exe.split(/[\\/]/).pop() ?? exe;
  const out = ps(
    [
      `$want = ${quote(exe)}; $leaf = ${quote(leaf)}; $typed = $false`,
      'Get-CimInstance Win32_Process | Where-Object { $_.Name -ieq $leaf } | ForEach-Object {',
      '  $p = $_.ExecutablePath',
      "  if (-not $p) { if (-not $typed) { Add-Type -TypeDefinition $image; $typed = $true }; $p = [M11Image]::Path([uint32]$_.ProcessId) }",
      '  if (-not $p -or $p -ieq $want) { $_.ProcessId }',
      '}',
    ].join('\n'),
    { image: IMAGE },
  );
  return out
    .split(/\r?\n/)
    .filter((l) => l.trim())
    .map(Number);
}

/**
 * `pid` and every process descended from it (CIM's ParentProcessId): the app and the WebView2
 * processes it starts. A window of any of them in the foreground is the test taking focus.
 */
export function processTree(pid: number): number[] {
  const out = ps('Get-CimInstance Win32_Process | ForEach-Object { "$($_.ProcessId) $($_.ParentProcessId)" }');
  const children = new Map<number, number[]>();
  for (const line of out.split(/\r?\n/)) {
    const [p, parent] = line.trim().split(/\s+/).map(Number);
    if (!Number.isFinite(p) || !Number.isFinite(parent) || p === parent) continue;
    children.set(parent, [...(children.get(parent) ?? []), p]);
  }
  const tree = [pid];
  for (let i = 0; i < tree.length; i++) for (const c of children.get(tree[i]) ?? []) if (!tree.includes(c)) tree.push(c);
  return tree;
}

/** The machine-wide count of processes named `image`, as tasklist reports them (printed, never
 * asserted: other sessions run solvers, PLAN.md F8). */
export function machineWideCount(image: string): number {
  const out = ps(`@(Get-Process -Name '${image.replace(/\.exe$/i, '')}' -ErrorAction SilentlyContinue).Count`);
  return Number(out) || 0;
}

/** Sends WM_CLOSE to `pid`'s main window, as the close button does. */
export function closeMainWindow(pid: number): boolean {
  return ps(`(Get-Process -Id ${pid}).CloseMainWindow()`) === 'True';
}

/** Stop-Process -Force: the process dies with no chance to clean up. */
export function stopProcess(pid: number): void {
  ps(`Stop-Process -Id ${pid} -Force`);
}

/** Whether a process with `pid` still runs. */
export function alive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}
