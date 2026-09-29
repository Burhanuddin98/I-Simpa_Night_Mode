// Windows and processes, read from outside the page through Windows PowerShell 5.1
// (docs/investigations/2026-09-29-m11/PLAN.md 4.3): which window is in the foreground, whether
// the app's windows are visible, not minimised and on a monitor, which processes run from a
// path, and the close and kill the gate's (d) needs. Nothing here moves, resizes, hides,
// minimises or activates a window: the test windows stay as Burhan sees them.
import { spawnSync } from 'node:child_process';

/** Runs `script` in Windows PowerShell and returns its stdout; throws on a non-zero exit. */
export function ps(script: string): string {
  const r = spawnSync(
    'powershell.exe',
    ['-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-Command', `$ProgressPreference='SilentlyContinue'; ${script}`],
    { encoding: 'utf8', windowsHide: true },
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

/** Ids of the processes whose executable is exactly `exe` (case-insensitive), as CIM reports them. */
export function processesFrom(exe: string): number[] {
  const esc = exe.replace(/'/g, "''");
  const out = ps(
    `Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -and $_.ExecutablePath -ieq '${esc}' } | ForEach-Object { $_.ProcessId }`,
  );
  return out
    .split(/\r?\n/)
    .filter((l) => l.trim())
    .map(Number);
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
