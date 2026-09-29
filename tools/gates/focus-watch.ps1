# m11-focus's runtime watcher (docs/investigations/2026-09-29-m11/PLAN.md 4.2, item 2).
# Burhan, 2026-09-29 11:26: the test windows stay visible; they must never take keyboard focus
# from what he is using. This records what the judge (app/e2e/lib/focus.ts) needs to prove it:
#   - every foreground change, out of context through SetWinEventHook(EVENT_SYSTEM_FOREGROUND), so
#     none is missed between samples: its time, window, rectangle, process, and whether that
#     process descends from the gate (-RootPid), with its ancestry;
#   - a person's input, through low-level mouse and keyboard hooks: for a button-down its time,
#     screen point and injected flag; for a key-down its time and injected flag. NO KEY CODE IS
#     RECORDED. WebDriver's events are CDP events inside the renderer and never pass these hooks;
#   - every -SampleMs (250 ms), each top-level window of every app.exe descended from the gate:
#     IsWindowVisible, IsIconic, MonitorFromWindow(MONITOR_DEFAULTTONULL).
# One JSON object per line into -Out. The hooks run only while the gate runs: the watcher stops
# when "<Out>.stop" appears, or when the gate's process is gone.
#
# m11.ps1 starts it as a process with no window at all (CreateNoWindow), so starting it shows
# nothing and takes no focus. It moves, hides, minimises and activates nothing.
#
#   powershell -File tools/gates/focus-watch.ps1 -Out <focus.jsonl> -RootPid <gate pid> [-SampleMs 250]
#   powershell -File tools/gates/focus-watch.ps1 -CheckOnly -RootPid <pid> [-AppName app.exe]
# -CheckOnly compiles the watcher and prints one sample of the process tree and its app windows,
# as JSON, without installing any hook (the gate's static step runs it).
#
# Windows PowerShell 5.1: the C# below is C# 5 (the .NET Framework compiler Add-Type uses).
param(
    [string]$Out = '',
    [Parameter(Mandatory = $true)][int]$RootPid,
    [int]$SampleMs = 250,
    # The image whose windows are sampled; app.exe but for probing the watcher itself.
    [string]$AppName = 'app.exe',
    [switch]$CheckOnly
)
$ErrorActionPreference = 'Stop'

Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;

public static class M11FocusWatch
{
    delegate void WinEventProc(IntPtr hook, uint ev, IntPtr hwnd, int idObject, int idChild, uint thread, uint time);
    delegate IntPtr LowLevelProc(int code, IntPtr wParam, IntPtr lParam);
    delegate bool EnumProc(IntPtr h, IntPtr l);

    [StructLayout(LayoutKind.Sequential)] struct POINT { public int X; public int Y; }
    [StructLayout(LayoutKind.Sequential)] struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
    [StructLayout(LayoutKind.Sequential)] struct MSG { public IntPtr hwnd; public uint message; public IntPtr wParam; public IntPtr lParam; public uint time; public POINT pt; }
    [StructLayout(LayoutKind.Sequential)] struct MSLLHOOKSTRUCT { public POINT pt; public uint mouseData; public uint flags; public uint time; public IntPtr extra; }
    [StructLayout(LayoutKind.Sequential)] struct KBDLLHOOKSTRUCT { public uint vkCode; public uint scanCode; public uint flags; public uint time; public IntPtr extra; }
    [StructLayout(LayoutKind.Sequential, CharSet = CharSet.Unicode)]
    struct PROCESSENTRY32W
    {
        public uint dwSize; public uint cntUsage; public uint th32ProcessID; public IntPtr th32DefaultHeapID;
        public uint th32ModuleID; public uint cntThreads; public uint th32ParentProcessID; public int pcPriClassBase;
        public uint dwFlags; [MarshalAs(UnmanagedType.ByValTStr, SizeConst = 260)] public string szExeFile;
    }

    [DllImport("user32.dll")] static extern IntPtr SetWinEventHook(uint min, uint max, IntPtr hmod, WinEventProc proc, uint pid, uint tid, uint flags);
    [DllImport("user32.dll")] static extern bool UnhookWinEvent(IntPtr h);
    [DllImport("user32.dll", SetLastError = true)] static extern IntPtr SetWindowsHookEx(int id, LowLevelProc proc, IntPtr hmod, uint tid);
    [DllImport("user32.dll")] static extern bool UnhookWindowsHookEx(IntPtr h);
    [DllImport("user32.dll")] static extern IntPtr CallNextHookEx(IntPtr h, int code, IntPtr w, IntPtr l);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] static extern IntPtr GetModuleHandle(string name);
    [DllImport("user32.dll")] static extern int GetMessage(out MSG msg, IntPtr hwnd, uint min, uint max);
    [DllImport("user32.dll")] static extern bool TranslateMessage(ref MSG msg);
    [DllImport("user32.dll")] static extern IntPtr DispatchMessage(ref MSG msg);
    [DllImport("user32.dll")] static extern bool PostThreadMessage(uint tid, uint msg, IntPtr w, IntPtr l);
    [DllImport("kernel32.dll")] static extern uint GetCurrentThreadId();
    [DllImport("user32.dll")] static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    [DllImport("user32.dll")] static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] static extern int GetClassName(IntPtr h, StringBuilder s, int n);
    [DllImport("user32.dll")] static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] static extern bool IsIconic(IntPtr h);
    [DllImport("user32.dll")] static extern IntPtr MonitorFromWindow(IntPtr h, uint flags);
    [DllImport("user32.dll")] static extern bool EnumWindows(EnumProc cb, IntPtr l);
    [DllImport("user32.dll")] static extern IntPtr GetForegroundWindow();
    [DllImport("kernel32.dll", SetLastError = true)] static extern IntPtr CreateToolhelp32Snapshot(uint flags, uint pid);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] static extern bool Process32FirstW(IntPtr snap, ref PROCESSENTRY32W e);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode)] static extern bool Process32NextW(IntPtr snap, ref PROCESSENTRY32W e);
    [DllImport("kernel32.dll")] static extern IntPtr OpenProcess(uint access, bool inherit, uint pid);
    [DllImport("kernel32.dll")] static extern bool GetProcessTimes(IntPtr h, out long creation, out long exit, out long kernel, out long user);
    [DllImport("kernel32.dll")] static extern bool CloseHandle(IntPtr h);

    const uint EVENT_SYSTEM_FOREGROUND = 3;
    const uint WINEVENT_OUTOFCONTEXT = 0;
    const uint WINEVENT_SKIPOWNPROCESS = 2;
    const int WH_KEYBOARD_LL = 13;
    const int WH_MOUSE_LL = 14;
    const uint WM_QUIT = 0x12;
    const uint TH32CS_SNAPPROCESS = 2;
    const uint MONITOR_DEFAULTTONULL = 0;
    const uint PROCESS_QUERY_LIMITED_INFORMATION = 0x1000;

    static readonly object sync = new object();
    static StreamWriter writer;
    static uint t0;
    static uint root;
    static string stopFile;
    static int sampleMs;
    static string appName = "app.exe";
    static uint hookThread;
    static volatile bool stopping;
    static string stopReason = "ended";
    static int nFg, nMouse, nKey, nSamples;
    // Held for the hooks' lifetime: the garbage collector must not take a delegate Windows calls.
    static WinEventProc fgProc;
    static LowLevelProc mouseProc;
    static LowLevelProc keyProc;

    class Proc { public uint Pid; public uint Parent; public string Name; }

    static int Rel(uint time) { return unchecked((int)(time - t0)); }
    static int Now() { return Rel(unchecked((uint)Environment.TickCount)); }
    static string B(bool b) { return b ? "true" : "false"; }
    static string Hwnd(IntPtr h) { return "\"0x" + h.ToInt64().ToString("x") + "\""; }

    public static string Json(string s)
    {
        if (s == null) return "null";
        var b = new StringBuilder("\"");
        foreach (char c in s)
        {
            switch (c)
            {
                case '"': b.Append("\\\""); break;
                case '\\': b.Append("\\\\"); break;
                case '\n': b.Append("\\n"); break;
                case '\r': b.Append("\\r"); break;
                case '\t': b.Append("\\t"); break;
                default:
                    if (c < 0x20) b.Append("\\u").Append(((int)c).ToString("x4")); else b.Append(c);
                    break;
            }
        }
        return b.Append('"').ToString();
    }

    static void Write(string line) { lock (sync) { if (writer != null) writer.WriteLine(line); } }
    static void Error(string message) { Write("{\"kind\":\"error\",\"t\":" + Now() + ",\"message\":" + Json(message) + "}"); }

    static string Title(IntPtr h) { var s = new StringBuilder(512); GetWindowText(h, s, 512); return s.ToString(); }
    static string Class(IntPtr h) { var s = new StringBuilder(256); GetClassName(h, s, 256); return s.ToString(); }

    static Dictionary<uint, Proc> Processes()
    {
        var d = new Dictionary<uint, Proc>();
        IntPtr snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if (snap == new IntPtr(-1)) return d;
        try
        {
            var e = new PROCESSENTRY32W();
            e.dwSize = (uint)Marshal.SizeOf(typeof(PROCESSENTRY32W));
            if (Process32FirstW(snap, ref e))
            {
                do { d[e.th32ProcessID] = new Proc { Pid = e.th32ProcessID, Parent = e.th32ParentProcessID, Name = e.szExeFile }; }
                while (Process32NextW(snap, ref e));
            }
        }
        finally { CloseHandle(snap); }
        return d;
    }

    /// A process's creation time (FILETIME ticks); 0 when it cannot be read.
    static long Created(uint pid)
    {
        IntPtr h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid);
        if (h == IntPtr.Zero) return 0;
        try { long c, e, k, u; return GetProcessTimes(h, out c, out e, out k, out u) ? c : 0; }
        finally { CloseHandle(h); }
    }

    /// The gate's process and every process descended from it. A "child" created before its
    /// parent is a reused process id, not a child, and is left out.
    static HashSet<uint> Tree(Dictionary<uint, Proc> ps)
    {
        var children = new Dictionary<uint, List<uint>>();
        foreach (var p in ps.Values)
        {
            if (p.Pid == p.Parent || p.Pid == 0) continue;
            List<uint> l;
            if (!children.TryGetValue(p.Parent, out l)) { l = new List<uint>(); children[p.Parent] = l; }
            l.Add(p.Pid);
        }
        var tree = new HashSet<uint>();
        if (!ps.ContainsKey(root)) return tree;
        var created = new Dictionary<uint, long>();
        created[root] = Created(root);
        var q = new Queue<uint>();
        q.Enqueue(root);
        tree.Add(root);
        while (q.Count > 0)
        {
            uint p = q.Dequeue();
            List<uint> l;
            if (!children.TryGetValue(p, out l)) continue;
            foreach (var c in l)
            {
                if (tree.Contains(c)) continue;
                long cc = Created(c);
                if (cc != 0 && created[p] != 0 && cc < created[p]) continue;
                created[c] = cc;
                tree.Add(c);
                q.Enqueue(c);
            }
        }
        return tree;
    }

    /// "name:pid" up the parent chain, at most 32 steps.
    static string Chain(Dictionary<uint, Proc> ps, uint pid)
    {
        var parts = new List<string>();
        var seen = new HashSet<uint>();
        uint cur = pid;
        for (int i = 0; i < 32 && cur != 0 && seen.Add(cur); i++)
        {
            Proc p;
            if (!ps.TryGetValue(cur, out p)) { parts.Add(Json("?:" + cur)); break; }
            parts.Add(Json(p.Name + ":" + p.Pid));
            cur = p.Parent;
        }
        return "[" + string.Join(",", parts.ToArray()) + "]";
    }

    static void OnForeground(IntPtr hook, uint ev, IntPtr hwnd, int idObject, int idChild, uint thread, uint time)
    {
        try
        {
            if (hwnd == IntPtr.Zero) return;
            nFg++;
            uint pid;
            GetWindowThreadProcessId(hwnd, out pid);
            RECT r;
            GetWindowRect(hwnd, out r);
            var ps = Processes();
            var tree = Tree(ps);
            Proc p;
            string name = ps.TryGetValue(pid, out p) ? p.Name : "?";
            Write("{\"kind\":\"fg\",\"t\":" + Rel(time) + ",\"hwnd\":" + Hwnd(hwnd) + ",\"pid\":" + pid + ",\"name\":" + Json(name) +
                ",\"title\":" + Json(Title(hwnd)) + ",\"cls\":" + Json(Class(hwnd)) + ",\"rect\":[" + r.Left + "," + r.Top + "," + r.Right + "," + r.Bottom +
                "],\"gate\":" + B(tree.Contains(pid)) + ",\"chain\":" + Chain(ps, pid) + "}");
        }
        catch (Exception e) { Error("foreground: " + e.Message); }
    }

    static IntPtr OnMouse(int code, IntPtr wParam, IntPtr lParam)
    {
        try
        {
            int m = wParam.ToInt32();
            // Button-downs only: left, right, middle, X.
            if (code >= 0 && (m == 0x201 || m == 0x204 || m == 0x207 || m == 0x20B))
            {
                var s = (MSLLHOOKSTRUCT)Marshal.PtrToStructure(lParam, typeof(MSLLHOOKSTRUCT));
                nMouse++;
                // LLMHF_INJECTED | LLMHF_LOWER_IL_INJECTED
                Write("{\"kind\":\"mouse\",\"t\":" + Rel(s.time) + ",\"injected\":" + B((s.flags & 0x3) != 0) + ",\"x\":" + s.pt.X + ",\"y\":" + s.pt.Y + "}");
            }
        }
        catch (Exception e) { Error("mouse: " + e.Message); }
        return CallNextHookEx(IntPtr.Zero, code, wParam, lParam);
    }

    static IntPtr OnKey(int code, IntPtr wParam, IntPtr lParam)
    {
        try
        {
            int m = wParam.ToInt32();
            // Key-downs only (WM_KEYDOWN, WM_SYSKEYDOWN); the key itself is never read.
            if (code >= 0 && (m == 0x100 || m == 0x104))
            {
                var s = (KBDLLHOOKSTRUCT)Marshal.PtrToStructure(lParam, typeof(KBDLLHOOKSTRUCT));
                nKey++;
                // LLKHF_INJECTED | LLKHF_LOWER_IL_INJECTED
                Write("{\"kind\":\"key\",\"t\":" + Rel(s.time) + ",\"injected\":" + B((s.flags & 0x12) != 0) + "}");
            }
        }
        catch (Exception e) { Error("key: " + e.Message); }
        return CallNextHookEx(IntPtr.Zero, code, wParam, lParam);
    }

    /// One sample: the gate's app.exe processes and their top-level windows (titled or visible).
    static string Sample(int t)
    {
        var ps = Processes();
        if (!ps.ContainsKey(root)) return null;
        var tree = Tree(ps);
        var apps = new HashSet<uint>();
        foreach (var pid in tree)
        {
            Proc p;
            if (ps.TryGetValue(pid, out p) && string.Equals(p.Name, appName, StringComparison.OrdinalIgnoreCase)) apps.Add(pid);
        }
        var windows = new List<string>();
        if (apps.Count > 0)
        {
            EnumWindows(delegate (IntPtr h, IntPtr l)
            {
                uint pid;
                GetWindowThreadProcessId(h, out pid);
                if (!apps.Contains(pid)) return true;
                string title = Title(h);
                bool visible = IsWindowVisible(h);
                if (title.Length == 0 && !visible) return true;
                windows.Add("{\"hwnd\":" + Hwnd(h) + ",\"pid\":" + pid + ",\"title\":" + Json(title) + ",\"cls\":" + Json(Class(h)) +
                    ",\"visible\":" + B(visible) + ",\"iconic\":" + B(IsIconic(h)) + ",\"monitor\":" + B(MonitorFromWindow(h, MONITOR_DEFAULTTONULL) != IntPtr.Zero) + "}");
                return true;
            }, IntPtr.Zero);
        }
        var appList = new List<string>();
        foreach (var a in apps) appList.Add(a.ToString());
        return "{\"kind\":\"sample\",\"t\":" + t + ",\"apps\":[" + string.Join(",", appList.ToArray()) + "],\"windows\":[" + string.Join(",", windows.ToArray()) + "]}";
    }

    static void Quit(string reason)
    {
        stopReason = reason;
        stopping = true;
        PostThreadMessage(hookThread, WM_QUIT, IntPtr.Zero, IntPtr.Zero);
    }

    static void SampleLoop()
    {
        while (!stopping)
        {
            try
            {
                if (File.Exists(stopFile)) { Quit("stop_file"); return; }
                string s = Sample(Now());
                if (s == null) { Quit("root_gone"); return; }
                Write(s);
                nSamples++;
                lock (sync) { if (writer != null) writer.Flush(); }
            }
            catch (Exception e) { Error("sample: " + e.Message); }
            Thread.Sleep(sampleMs);
        }
    }

    /// Compiles and reads one sample of `rootPid`'s tree, with no hook installed.
    public static string Probe(int rootPid, string app)
    {
        root = (uint)rootPid;
        appName = app;
        t0 = unchecked((uint)Environment.TickCount);
        var ps = Processes();
        var tree = Tree(ps);
        var names = new List<string>();
        foreach (var pid in tree) { Proc p; if (ps.TryGetValue(pid, out p)) names.Add(Json(p.Name + ":" + pid)); }
        string sample = Sample(0);
        return "{\"root\":" + rootPid + ",\"tree\":[" + string.Join(",", names.ToArray()) + "],\"sample\":" + (sample ?? "null") + "}";
    }

    /// Watches until `stop` appears or the gate's process is gone. Returns 0.
    public static int Run(string outPath, int rootPid, string stop, int everyMs, string app)
    {
        root = (uint)rootPid;
        appName = app;
        stopFile = stop;
        sampleMs = everyMs;
        t0 = unchecked((uint)Environment.TickCount);
        writer = new StreamWriter(new FileStream(outPath, FileMode.Create, FileAccess.Write, FileShare.ReadWrite), new UTF8Encoding(false));
        hookThread = GetCurrentThreadId();
        fgProc = OnForeground;
        mouseProc = OnMouse;
        keyProc = OnKey;
        IntPtr hmod = GetModuleHandle(null);
        IntPtr hFg = SetWinEventHook(EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND, IntPtr.Zero, fgProc, 0, 0, WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS);
        IntPtr hMouse = SetWindowsHookEx(WH_MOUSE_LL, mouseProc, hmod, 0);
        IntPtr hKey = SetWindowsHookEx(WH_KEYBOARD_LL, keyProc, hmod, 0);
        IntPtr fg = GetForegroundWindow();
        uint fgPid;
        GetWindowThreadProcessId(fg, out fgPid);
        Write("{\"kind\":\"start\",\"t\":0,\"wall\":" + Json(DateTimeOffset.Now.ToString("o")) + ",\"root\":" + rootPid + ",\"sample_ms\":" + everyMs +
            ",\"hooks\":{\"foreground\":" + B(hFg != IntPtr.Zero) + ",\"mouse\":" + B(hMouse != IntPtr.Zero) + ",\"keyboard\":" + B(hKey != IntPtr.Zero) +
            "},\"foreground_at_start\":{\"pid\":" + fgPid + ",\"title\":" + Json(Title(fg)) + "}}");
        lock (sync) { writer.Flush(); }
        File.WriteAllText(outPath + ".started", DateTimeOffset.Now.ToString("o"));
        var sampler = new Thread(SampleLoop);
        sampler.IsBackground = true;
        sampler.Start();
        MSG msg;
        while (GetMessage(out msg, IntPtr.Zero, 0, 0) > 0)
        {
            TranslateMessage(ref msg);
            DispatchMessage(ref msg);
        }
        stopping = true;
        if (hFg != IntPtr.Zero) UnhookWinEvent(hFg);
        if (hMouse != IntPtr.Zero) UnhookWindowsHookEx(hMouse);
        if (hKey != IntPtr.Zero) UnhookWindowsHookEx(hKey);
        sampler.Join(2000);
        Write("{\"kind\":\"stop\",\"t\":" + Now() + ",\"reason\":" + Json(stopReason) + ",\"fg\":" + nFg + ",\"mouse\":" + nMouse + ",\"key\":" + nKey + ",\"samples\":" + nSamples + "}");
        lock (sync) { writer.Flush(); writer.Close(); writer = null; }
        return 0;
    }
}
'@

if ($CheckOnly) {
    [M11FocusWatch]::Probe($RootPid, $AppName)
    exit 0
}
if (-not $Out) { throw '-Out is required (or -CheckOnly)' }
try {
    exit [M11FocusWatch]::Run($Out, $RootPid, "$Out.stop", $SampleMs, $AppName)
} catch {
    Add-Content -Path "$Out.err.txt" -Value $_.Exception.ToString()
    exit 1
}
