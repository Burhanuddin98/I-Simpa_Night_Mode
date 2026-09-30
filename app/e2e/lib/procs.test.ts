// node --test suite for procs.ts's processesFrom, on real processes: a copy of ping.exe created
// suspended and never resumed (what the core's spawn leaves if its parent dies in the spawn
// window, M11 review 2, B1), and a running copy of the same name from another folder. CIM gives
// the suspended one no ExecutablePath (review 2, M1), so a query on that path alone cannot see
// it; processesFrom must, and must not count the other folder's process.
import { strict as assert } from 'node:assert';
import { spawn } from 'node:child_process';
import { copyFileSync, mkdirSync, mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import { alive, processesFrom, ps } from './procs.ts';

const CREATE = `
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class M11Suspended {
  [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
  struct SI { public int cb; public string r, d, t; public int x, y, w, h, xc, yc, fa, fl; public short sw, cr; public IntPtr r2, i, o, e; }
  [StructLayout(LayoutKind.Sequential)] struct PI { public IntPtr hp, ht; public int pid, tid; }
  [DllImport("kernel32.dll", SetLastError = true, CharSet = CharSet.Unicode)]
  static extern bool CreateProcessW(string app, StringBuilder cmd, IntPtr pa, IntPtr ta, bool inherit, uint flags, IntPtr env, string cwd, ref SI si, out PI pi);
  [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);
  public static int Start(string exe) {
    var si = new SI(); si.cb = Marshal.SizeOf(si); PI pi;
    // CREATE_SUSPENDED | CREATE_NO_WINDOW, as the core creates every child.
    if (!CreateProcessW(exe, new StringBuilder("\\"" + exe + "\\" -n 30 127.0.0.1"), IntPtr.Zero, IntPtr.Zero, false, 0x4 | 0x08000000, IntPtr.Zero, null, ref si, out pi))
      throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
    CloseHandle(pi.ht); CloseHandle(pi.hp);
    return pi.pid;
  }
}`;

function startSuspended(exe: string): number {
  return Number(ps('Add-Type -TypeDefinition $source; [M11Suspended]::Start($exe)', { source: CREATE, exe }));
}

/** The query processesFrom used before review 2: CIM's ExecutablePath alone. */
function byCimPathOnly(exe: string): number[] {
  const out = ps(
    'Get-CimInstance Win32_Process | Where-Object { $_.ExecutablePath -and $_.ExecutablePath -ieq $exe } | ForEach-Object { $_.ProcessId }',
    { exe },
  );
  return out
    .split(/\r?\n/)
    .filter((l) => l.trim())
    .map(Number);
}

test('processesFrom sees a process that never ran, and only from its own folder', () => {
  const dir = mkdtempSync(path.join(tmpdir(), 'm11-procs-'));
  const [a, b] = ['a', 'b'].map((sub) => {
    mkdirSync(path.join(dir, sub));
    const exe = path.join(dir, sub, 'm11probe.exe');
    copyFileSync(path.join(process.env.SystemRoot ?? 'C:\\Windows', 'System32', 'PING.EXE'), exe);
    return exe;
  });
  const pids: number[] = [];
  try {
    const suspended = startSuspended(a);
    pids.push(suspended);
    const other = spawn(b, ['-n', '30', '127.0.0.1'], { stdio: 'ignore', windowsHide: true });
    assert.ok(other.pid, "the other folder's copy started");
    pids.push(other.pid);
    assert.ok(alive(suspended), 'the suspended process exists');
    // The say-NO: CIM's path alone misses it.
    assert.deepEqual(byCimPathOnly(a), [], 'CIM reports no ExecutablePath for a process that never ran');
    assert.deepEqual(processesFrom(a), [suspended]);
    assert.deepEqual(processesFrom(b), [other.pid]);
  } finally {
    for (const pid of pids) {
      try {
        process.kill(pid);
      } catch {
        // already gone
      }
    }
    // A killed process has its exit code at once but unmaps its image a moment later: until then
    // the copy cannot be removed (EPERM), so the removal is tried again for up to 5 s.
    const t0 = Date.now();
    for (;;) {
      try {
        rmSync(dir, { recursive: true, force: true });
        break;
      } catch (e) {
        if (Date.now() - t0 > 5000) throw e;
        Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 100);
      }
    }
  }
  assert.deepEqual(processesFrom(a), [], 'none once it is killed');
});
