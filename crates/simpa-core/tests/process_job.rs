//! The process layer on Windows (`simpa_core::process`): tagged lines from both pipes, exit
//! codes, and a cancel that takes the whole tree down, grandchildren included. The children are
//! cmd.exe, powershell.exe and ping.exe; nothing new is built.
//!
//! Processes of a tree are watched through handles opened while they are alive, so an id cannot
//! be reused before a test looks at it. A [`Proc`] kills its process on drop if it is still
//! running, so a failing test leaves no ping behind.
#![cfg(windows)]

use std::ffi::OsString;
use std::io::{BufRead, BufReader, Write};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use simpa_core::process::{self, CancelToken, Line, Outcome, Spec, Stream};
use windows_sys::Win32::Foundation::{WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows_sys::Win32::System::Threading::{
    OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
    TerminateProcess, WaitForSingleObject,
};

/// Set in the environment of the re-run test binary that plays the parent process.
const HELPER_ENV: &str = "SIMPA_PROCESS_JOB_HELPER";

#[allow(dead_code)]
#[path = "common/scratch.rs"]
mod scratch;

/// A fresh working folder per test under `target/tmp/process_job/`, removed when the test passes
/// and kept when it fails (`common/scratch.rs`).
fn work_dir(name: &str) -> PathBuf {
    scratch::fresh("process_job", name)
}

fn spec(program: &str, args: &[&str], cwd: PathBuf) -> Spec {
    Spec {
        program: program.into(),
        args: args.iter().map(OsString::from).collect(),
        cwd,
    }
}

fn cmd(script: &str, cwd: PathBuf) -> Spec {
    spec("cmd.exe", &["/d", "/c", script], cwd)
}

fn powershell(script: &str, cwd: PathBuf) -> Spec {
    // No progress records: PowerShell 5.1 writes them to a redirected stderr as CLIXML.
    let script = format!("$ProgressPreference='SilentlyContinue'; {script}");
    let args = [
        "-NoLogo",
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        &script,
    ];
    spec("powershell.exe", &args, cwd)
}

/// PowerShell starts `ping -n 30` (output on the shared pipes), prints `PIDS <own> <ping>`, then
/// does `tail`.
fn ping_grandchild(tail: &str) -> String {
    format!(
        "$p = Start-Process -FilePath ping.exe -ArgumentList '-n','30','127.0.0.1' -NoNewWindow \
         -PassThru; [Console]::Out.WriteLine('PIDS ' + $PID + ' ' + $p.Id); {tail}"
    )
}

/// As [`ping_grandchild`], with ping's stdout sent to `ping_out.txt`, so the `PIDS` line is the
/// first line on either pipe. Ping still holds the stderr pipe.
fn quiet_ping_grandchild(tail: &str) -> String {
    format!(
        "$p = Start-Process -FilePath ping.exe -ArgumentList '-n','30','127.0.0.1' -NoNewWindow \
         -PassThru -RedirectStandardOutput ping_out.txt; \
         [Console]::Out.WriteLine('PIDS ' + $PID + ' ' + $p.Id); {tail}"
    )
}

/// The ids in a `PIDS a b` line. The line may carry a prefix: in the re-run test binary, libtest's
/// `test helper_parent_process ... ` can share it.
fn pids(text: &str) -> Option<Vec<u32>> {
    let rest = &text[text.find("PIDS ")? + 5..];
    rest.split_whitespace().map(|p| p.parse().ok()).collect()
}

fn collect(spec: &Spec) -> (Vec<Line>, Outcome) {
    let mut lines = Vec::new();
    let outcome = process::run(spec, &CancelToken::new(), &mut |l| lines.push(l.clone()))
        .expect("the child starts");
    (lines, outcome)
}

fn on(lines: &[Line], stream: Stream) -> Vec<(String, bool)> {
    lines
        .iter()
        .filter(|l| l.stream == stream)
        .map(|l| (l.text.clone(), l.terminated))
        .collect()
}

fn full(texts: &[&str]) -> Vec<(String, bool)> {
    texts.iter().map(|t| (t.to_string(), true)).collect()
}

/// A process of a child's tree, held open from the moment its id is known.
struct Proc {
    pid: u32,
    handle: OwnedHandle,
}

impl Proc {
    /// `None` when no process has this id any more.
    fn open(pid: u32) -> Option<Proc> {
        let access = PROCESS_SYNCHRONIZE | PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION;
        // SAFETY: plain call; a null result is checked.
        let raw = unsafe { OpenProcess(access, 0, pid) };
        if raw.is_null() {
            return None;
        }
        // SAFETY: `raw` is a fresh handle owned by nothing else.
        let handle = unsafe { OwnedHandle::from_raw_handle(raw) };
        Some(Proc { pid, handle })
    }

    fn open_alive(pid: u32) -> Proc {
        let p = Proc::open(pid).unwrap_or_else(|| panic!("process {pid} is already gone"));
        assert!(!p.exited_within(0), "process {pid} is already gone");
        p
    }

    fn exited_within(&self, ms: u32) -> bool {
        // SAFETY: the handle is open and has SYNCHRONIZE access.
        match unsafe { WaitForSingleObject(self.handle.as_raw_handle(), ms) } {
            WAIT_OBJECT_0 => true,
            WAIT_TIMEOUT => false,
            other => panic!("WaitForSingleObject on process {}: {other}", self.pid),
        }
    }
}

impl Drop for Proc {
    fn drop(&mut self) {
        if !self.exited_within(0) {
            // SAFETY: the handle is open and has PROCESS_TERMINATE access.
            unsafe { TerminateProcess(self.handle.as_raw_handle(), 1) };
        }
    }
}

fn alive(procs: &[Proc], grace_ms: u32) -> Vec<u32> {
    procs
        .iter()
        .filter(|p| !p.exited_within(grace_ms))
        .map(|p| p.pid)
        .collect()
}

#[test]
fn streams_are_tagged_and_ordered_and_crlf_is_stripped() {
    let script = "echo out1& 1>&2 echo err1& echo out2& 1>&2 echo err2& echo out3& 1>&2 echo err3";
    let (lines, outcome) = collect(&cmd(script, work_dir("tagged")));
    assert_eq!(outcome.exit_code, Some(0));
    assert!(!outcome.cancelled);
    assert_eq!(on(&lines, Stream::Stdout), full(&["out1", "out2", "out3"]));
    assert_eq!(on(&lines, Stream::Stderr), full(&["err1", "err2", "err3"]));
}

#[test]
fn a_hundred_thousand_lines_on_each_stream_are_all_delivered_in_order() {
    // 100 blocks of 1,000 lines, alternating between the pipes: stdout ends lines with LF,
    // stderr with CRLF. A reader that drained one pipe before the other would deadlock here.
    let script = "$o=[Console]::Out; $e=[Console]::Error; \
        for($b=0;$b -lt 100;$b++){ $s=[Text.StringBuilder]::new(); $t=[Text.StringBuilder]::new(); \
        for($i=1;$i -le 1000;$i++){ $n=$b*1000+$i; \
        [void]$s.Append('o').Append($n).Append([char]10); \
        [void]$t.Append('e').Append($n).Append([char]13).Append([char]10) }; \
        $o.Write($s.ToString()); $o.Flush(); $e.Write($t.ToString()); $e.Flush() }";
    let started = Instant::now();
    let (lines, outcome) = collect(&powershell(script, work_dir("flood")));
    let took = started.elapsed();
    assert_eq!(outcome.exit_code, Some(0));
    for (stream, prefix) in [(Stream::Stdout, 'o'), (Stream::Stderr, 'e')] {
        let got: Vec<&Line> = lines.iter().filter(|l| l.stream == stream).collect();
        assert_eq!(got.len(), 100_000, "{stream:?}: every line arrives");
        for (i, l) in got.iter().enumerate() {
            assert_eq!(l.text, format!("{prefix}{}", i + 1), "{stream:?} line {i}");
            assert!(l.terminated);
        }
        assert!(
            got.windows(2).all(|w| w[0].t_ms <= w[1].t_ms),
            "{stream:?}: t_ms is monotone"
        );
    }
    eprintln!(
        "measure: 2 x 100,000 lines through run(): {:.0} ms (child reported {:.0} ms)",
        took.as_secs_f64() * 1e3,
        outcome.elapsed_ms
    );
}

#[test]
fn a_final_partial_line_is_flushed_on_both_streams() {
    // `set /p` with no input sets errorlevel 1, hence the explicit exit.
    let (lines, outcome) = collect(&cmd(
        "echo whole& <nul set /p=partial_out& 1>&2 <nul set /p=partial_err& exit 0",
        work_dir("partial"),
    ));
    assert_eq!(outcome.exit_code, Some(0));
    assert_eq!(
        on(&lines, Stream::Stdout),
        vec![("whole".into(), true), ("partial_out".into(), false)]
    );
    assert_eq!(
        on(&lines, Stream::Stderr),
        vec![("partial_err".into(), false)]
    );
}

#[test]
fn raw_bytes_split_on_lf_with_one_cr_stripped_and_lossy_utf8() {
    // a CR LF, b LF, x 0xFF y LF, then "tail" with no terminator.
    let script = "$o=[Console]::OpenStandardOutput(); \
        $b=[byte[]](97,13,10,98,10,120,255,121,10,116,97,105,108); $o.Write($b,0,$b.Length); $o.Flush()";
    let (lines, outcome) = collect(&powershell(script, work_dir("bytes")));
    assert_eq!(outcome.exit_code, Some(0));
    assert_eq!(
        on(&lines, Stream::Stdout),
        vec![
            ("a".into(), true),
            ("b".into(), true),
            ("x\u{FFFD}y".into(), true),
            ("tail".into(), false),
        ]
    );
}

#[test]
fn exit_codes_are_raw_u32() {
    for (script, code) in [
        ("exit 0", 0u32),
        ("exit 3", 3),
        ("exit -1073741819", 0xC000_0005),
    ] {
        let (_, outcome) = collect(&cmd(script, work_dir("exit")));
        assert_eq!(outcome.exit_code, Some(code), "{script}");
        assert!(!outcome.cancelled);
    }
}

#[test]
fn cancel_from_another_thread_kills_the_grandchild() {
    let spec = powershell(
        &ping_grandchild("Start-Sleep -Seconds 30"),
        work_dir("cancel_thread"),
    );
    let token = CancelToken::new();
    let (pid_tx, pid_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let runner = {
        let token = token.clone();
        thread::spawn(move || {
            let mut lines = Vec::new();
            let result = process::run(&spec, &token, &mut |l| {
                if let Some(ids) = pids(&l.text) {
                    let _ = pid_tx.send(ids);
                }
                lines.push(l.clone());
            });
            let _ = done_tx.send(Instant::now());
            (result, lines)
        })
    };
    let ids = pid_rx
        .recv_timeout(Duration::from_secs(60))
        .expect("PowerShell printed its PIDS line");
    let procs: Vec<Proc> = ids.iter().map(|&p| Proc::open_alive(p)).collect();
    thread::sleep(Duration::from_millis(200));
    let asked = Instant::now();
    token.cancel();
    let returned = done_rx.recv_timeout(Duration::from_secs(3));
    let left = alive(&procs, 0);
    let Ok(returned) = returned else {
        drop(procs);
        let _ = runner.join();
        panic!("run() had not returned 3 s after cancel; still alive (powershell, ping): {left:?}");
    };
    let (result, _) = runner.join().unwrap();
    let outcome = result.expect("the child started");
    assert!(outcome.cancelled);
    assert_eq!(outcome.exit_code, None);
    assert!(
        left.is_empty(),
        "alive when run() returned (powershell, ping): {left:?}"
    );
    eprintln!(
        "measure: cancel() to run() returning, powershell + ping: {:.1} ms",
        (returned - asked).as_secs_f64() * 1e3
    );
}

#[test]
fn cancel_from_inside_on_line_on_the_first_line() {
    let spec = powershell(
        &quiet_ping_grandchild("Start-Sleep -Seconds 30"),
        work_dir("cancel_on_line"),
    );
    let token = CancelToken::new();
    let mut procs = Vec::new();
    let mut asked = None;
    let outcome = process::run(&spec, &token, &mut |l| {
        if asked.is_none() {
            let ids = pids(&l.text).unwrap_or_else(|| panic!("first line: {:?}", l.text));
            procs = ids.iter().map(|&p| Proc::open_alive(p)).collect();
            asked = Some(Instant::now());
            token.cancel();
        }
    })
    .expect("the child started");
    let took = asked.expect("a first line").elapsed();
    let left = alive(&procs, 0);
    assert!(outcome.cancelled);
    assert_eq!(outcome.exit_code, None);
    assert_eq!(procs.len(), 2);
    assert!(
        left.is_empty(),
        "alive when run() returned (powershell, ping): {left:?}"
    );
    assert!(
        took < Duration::from_secs(3),
        "run() returned {took:?} after cancel"
    );
}

#[test]
fn the_tree_ends_with_its_root() {
    // PowerShell leaves ping running (holding both pipes) and exits 5.
    let spec = powershell(
        &ping_grandchild("Start-Sleep -Seconds 1; exit 5"),
        work_dir("root_exit"),
    );
    let mut procs = Vec::new();
    let mut printed = None;
    let outcome = process::run(&spec, &CancelToken::new(), &mut |l| {
        if let Some(ids) = pids(&l.text) {
            printed = Some(Instant::now());
            procs.push(Proc::open_alive(ids[1]));
        }
    })
    .expect("the child started");
    let after = printed.expect("PIDS line").elapsed();
    let left = alive(&procs, 0);
    assert_eq!(outcome.exit_code, Some(5));
    assert!(!outcome.cancelled);
    assert!(left.is_empty(), "ping alive when run() returned: {left:?}");
    assert!(
        after < Duration::from_secs(5),
        "run() returned {after:?} after the PIDS line: it waited for the grandchild"
    );
}

#[test]
fn a_panic_in_on_line_kills_the_tree() {
    let spec = powershell(
        &quiet_ping_grandchild("Start-Sleep -Seconds 30"),
        work_dir("panic"),
    );
    let (tx, rx) = mpsc::channel();
    let runner = thread::spawn(move || {
        process::run(&spec, &CancelToken::new(), &mut |l| {
            let ids = pids(&l.text).unwrap_or_else(|| panic!("first line: {:?}", l.text));
            tx.send(ids.iter().map(|&p| Proc::open_alive(p)).collect::<Vec<_>>())
                .unwrap();
            panic!("deliberate panic in on_line");
        })
    });
    assert!(runner.join().is_err(), "run() unwound");
    let procs = rx.recv().expect("the PIDS line arrived");
    // Closing the job handle kills asynchronously: allow the gates' 2 s.
    let left = alive(&procs, 2000);
    assert!(
        left.is_empty(),
        "alive 2 s after the panic (powershell, ping): {left:?}"
    );
}

/// Not a test on its own: the parent process for `killing_the_parent_kills_the_tree`, which runs
/// this test binary again with only this function selected and [`HELPER_ENV`] set.
#[test]
#[ignore = "helper process for killing_the_parent_kills_the_tree"]
fn helper_parent_process() {
    if std::env::var_os(HELPER_ENV).is_none() {
        return;
    }
    let spec = powershell(
        &ping_grandchild("Start-Sleep -Seconds 30"),
        work_dir("parent_killed"),
    );
    let _ = process::run(&spec, &CancelToken::new(), &mut |l| {
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "{}", l.text);
        let _ = out.flush();
    });
}

/// Gate M6(f)'s `Stop-Process` clause at library level: `TerminateProcess` on the process that
/// called `run` leaves neither its child nor its grandchild running 2 s later.
#[test]
fn killing_the_parent_kills_the_tree() {
    let exe = std::env::current_exe().unwrap();
    // The helper is killed, so it removes nothing: its folder goes inside this test's own.
    let helper_root = work_dir("parent_killed_root");
    let mut parent = Command::new(exe)
        .args([
            "helper_parent_process",
            "--exact",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(HELPER_ENV, "1")
        .env(scratch::ROOT_ENV, &helper_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let stdout = parent.stdout.take().unwrap();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if let Some(ids) = pids(line.trim_end()) {
                let _ = tx.send(ids);
            }
        }
    });
    let ids = match rx.recv_timeout(Duration::from_secs(60)) {
        Ok(ids) => ids,
        Err(e) => {
            let _ = parent.kill();
            panic!("the helper printed no PIDS line: {e}");
        }
    };
    let procs: Vec<Proc> = ids.iter().map(|&p| Proc::open_alive(p)).collect();
    // What Stop-Process does.
    parent.kill().unwrap();
    parent.wait().unwrap();
    let left = alive(&procs, 2000);
    assert!(
        left.is_empty(),
        "alive 2 s after the parent was killed (powershell, ping): {left:?}"
    );
}

#[test]
fn a_cwd_with_a_non_ascii_name_works() {
    let dir = work_dir("\u{141}\u{f3}d\u{17a}-cwd");
    let marker = dir.join("marker.txt");
    let _ = std::fs::remove_file(&marker);
    let (_, outcome) = collect(&cmd("echo ok> marker.txt", dir.clone()));
    assert_eq!(outcome.exit_code, Some(0));
    assert!(
        marker.exists(),
        "cmd wrote its marker in the \u{141} folder"
    );

    let (lines, outcome) = collect(&powershell(
        "[Console]::OutputEncoding=[Text.UTF8Encoding]::new($false); \
         [Console]::Out.WriteLine((Get-Location).Path)",
        dir.clone(),
    ));
    assert_eq!(outcome.exit_code, Some(0));
    let out = on(&lines, Stream::Stdout);
    assert_eq!(out.len(), 1, "{out:?}");
    assert!(
        out[0].0.contains('\u{141}'),
        "UTF-8 decoded: {:?}",
        out[0].0
    );
    assert_eq!(
        std::fs::canonicalize(&out[0].0).unwrap(),
        std::fs::canonicalize(&dir).unwrap()
    );
}

#[test]
fn a_program_that_does_not_exist_is_an_error() {
    let cwd = work_dir("missing");
    for program in [
        "simpa-no-such-program-7f3a.exe".to_string(),
        cwd.join("nothing-here.exe").display().to_string(),
    ] {
        let err = process::run(
            &spec(&program, &[], cwd.clone()),
            &CancelToken::new(),
            &mut |_| {},
        )
        .expect_err(&program);
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound, "{program}: {err}");
    }
}

#[test]
fn a_cwd_that_does_not_exist_is_an_error() {
    let cwd = work_dir("missing").join("no-such-folder");
    assert!(process::run(&cmd("echo hi", cwd), &CancelToken::new(), &mut |_| {}).is_err());
}
