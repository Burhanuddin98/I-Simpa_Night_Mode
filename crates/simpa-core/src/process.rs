//! Child processes for TetGen and the solvers: both output streams read concurrently and tagged,
//! a trailing partial line flushed at end of stream, and cancel. Milestone M5; see
//! `docs/m5-m6-design.md`.
//!
//! On Windows the child runs in a Job Object (`winproc`). It is created suspended with
//! `CREATE_NO_WINDOW`, assigned to a job with `KILL_ON_JOB_CLOSE`, and only then resumed, so
//! nothing it starts can escape the job. Cancel is `TerminateJobObject`. The tree also ends with
//! its root: when the child exits, whatever it left running is killed. Either way, no process of
//! the tree is alive when [`run`] returns. If this process dies first, the kernel closes the job
//! handle and `KILL_ON_JOB_CLOSE` takes the tree down.
//!
//! Elsewhere a std-only fallback (`portable`) kills the direct child and nothing it started.

use std::ffi::OsString;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::{ChildStderr, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, SyncSender};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

#[cfg(any(not(windows), test))]
mod portable;
#[cfg(windows)]
mod winproc;

/// The stream a line arrived on. Lines are classified by content, never by stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stream {
    Stdout,
    Stderr,
}

/// One line of child output.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Line {
    pub stream: Stream,
    /// Milliseconds since the child was spawned, when the line was read.
    pub t_ms: f64,
    /// The line without its terminator (`\n` or `\r\n`), decoded as lossy UTF-8.
    pub text: String,
    /// False only for a final partial line flushed at end of stream.
    pub terminated: bool,
}

/// What to run. `args` are passed as given; `cwd` is the child's working folder.
#[derive(Clone, Debug)]
pub struct Spec {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: PathBuf,
}

/// A cancel flag shared between the caller, the line callback and other threads.
#[derive(Clone, Debug, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// How a child ended.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Outcome {
    /// The raw exit code (`u32`, so `0xC0000005` survives); `None` when the child was killed by
    /// cancel before it reported one.
    pub exit_code: Option<u32>,
    pub cancelled: bool,
    pub elapsed_ms: f64,
}

/// How often the loop looks at the cancel flag and the child while no line arrives.
const POLL: Duration = Duration::from_millis(20);
/// Lines queued between the pipe readers and `on_line`. A full queue stalls the child's writes,
/// never memory.
const QUEUE: usize = 4096;
/// Once the tree is dead, how long the pipes may stay open with nothing on them before the
/// readers are abandoned. Only a process outside the tree can still hold them.
const DRAIN_IDLE: Duration = Duration::from_secs(2);

/// The running child and whatever it starts.
trait Tree {
    /// Waits up to `timeout` (zero: not at all) for the child to exit; its exit status once it
    /// has.
    fn wait_exit(&mut self, timeout: Duration) -> io::Result<Option<ExitStatus>>;
    /// Kills every process of the tree, returning once none is alive.
    fn kill_all(&mut self) -> io::Result<()>;
}

/// Runs `spec` to completion or cancel. `on_line` runs on the calling thread, in arrival order,
/// and may call [`CancelToken::cancel`]. A cancel that arrives after the child has exited does
/// nothing. When this returns, no process of the tree is alive (Windows; elsewhere, the direct
/// child only).
///
/// Returns `Err` when the child cannot be started (the program or `cwd` does not exist), and on
/// Windows also when the tree cannot be killed within 10 s.
pub fn run(
    spec: &Spec,
    cancel: &CancelToken,
    on_line: &mut dyn FnMut(&Line),
) -> io::Result<Outcome> {
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.args)
        .current_dir(&spec.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    let (tree, stdout, stderr) = winproc::JobTree::spawn(command)?;
    #[cfg(not(windows))]
    let (tree, stdout, stderr) = portable::ChildTree::spawn(command)?;
    drive(tree, stdout, stderr, cancel, on_line)
}

/// The loop behind [`run`]: delivers lines, watches the cancel flag and the child, and kills the
/// tree on cancel or once the child has exited. Dropping `tree` early (an error, a panic in
/// `on_line`) kills it too.
fn drive<T: Tree>(
    mut tree: T,
    stdout: ChildStdout,
    stderr: ChildStderr,
    cancel: &CancelToken,
    on_line: &mut dyn FnMut(&Line),
) -> io::Result<Outcome> {
    let start = Instant::now();
    let (tx, rx) = mpsc::sync_channel::<Line>(QUEUE);
    let readers = [
        spawn_reader(stdout, Stream::Stdout, start, tx.clone())?,
        spawn_reader(stderr, Stream::Stderr, start, tx)?,
    ];
    let mut exit_code = None;
    let mut cancelled = false;
    // A reader still holds a sender.
    let mut open = true;
    // Set once the tree is dead: the time after which pipes still open but silent are abandoned.
    let mut give_up: Option<Instant> = None;
    let mut next_wait = start;
    loop {
        if give_up.is_none() {
            let ended = if cancel.is_cancelled() {
                cancelled = true;
                true
            } else if !open || Instant::now() >= next_wait {
                // With both pipes closed there is nothing else to wait on. They close before the
                // process is signalled, so a zero-timeout look would usually miss the exit.
                let timeout = if open { Duration::ZERO } else { POLL };
                next_wait = Instant::now() + POLL;
                match tree.wait_exit(timeout)? {
                    Some(status) => {
                        // `i32` to `u32` keeps the bits: -1073741819 is 0xC0000005.
                        exit_code = status.code().map(|c| c as u32);
                        true
                    }
                    None => false,
                }
            } else {
                false
            };
            if ended {
                tree.kill_all()?;
                give_up = Some(Instant::now() + DRAIN_IDLE);
            }
        }
        if !open {
            if give_up.is_some() {
                break;
            }
            // Both pipes closed, but the child still runs: `wait_exit` above does the waiting.
            continue;
        }
        match rx.recv_timeout(POLL) {
            Ok(line) => {
                on_line(&line);
                if give_up.is_some() {
                    give_up = Some(Instant::now() + DRAIN_IDLE);
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                if give_up.is_some_and(|t| Instant::now() >= t) {
                    break;
                }
            }
            Err(RecvTimeoutError::Disconnected) => open = false,
        }
    }
    // Abandoned readers (`open`) end on their own when the last writer closes.
    if !open {
        for reader in readers {
            let _ = reader.join();
        }
    }
    Ok(Outcome {
        exit_code,
        cancelled,
        elapsed_ms: start.elapsed().as_secs_f64() * 1e3,
    })
}

/// Reads `pipe` to its end, sending one [`Line`] per `\n` and a final partial line. Stops early
/// only when the receiver is gone.
fn spawn_reader(
    mut pipe: impl Read + Send + 'static,
    stream: Stream,
    start: Instant,
    tx: SyncSender<Line>,
) -> io::Result<thread::JoinHandle<()>> {
    let name = match stream {
        Stream::Stdout => "simpa-process-stdout",
        Stream::Stderr => "simpa-process-stderr",
    };
    thread::Builder::new().name(name.into()).spawn(move || {
        let emit = |bytes: &[u8], terminated: bool| {
            let bytes = bytes.strip_suffix(b"\r").unwrap_or(bytes);
            tx.send(Line {
                stream,
                t_ms: start.elapsed().as_secs_f64() * 1e3,
                text: String::from_utf8_lossy(bytes).into_owned(),
                terminated,
            })
            .is_ok()
        };
        let mut pending: Vec<u8> = Vec::new();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let n = match pipe.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                // A broken pipe is the end of the stream.
                Err(_) => break,
            };
            // `pending` holds no `\n`: only the new bytes need scanning.
            let mut scan = pending.len();
            pending.extend_from_slice(&buf[..n]);
            let mut begin = 0;
            while let Some(i) = pending[scan..].iter().position(|&b| b == b'\n') {
                let end = scan + i;
                if !emit(&pending[begin..end], true) {
                    return;
                }
                begin = end + 1;
                scan = begin;
            }
            pending.drain(..begin);
        }
        if !pending.is_empty() {
            emit(&pending, false);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hands out at most `step` bytes per read, to split lines across reads.
    struct Trickle {
        data: Vec<u8>,
        at: usize,
        step: usize,
    }

    impl Read for Trickle {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let n = self.step.min(buf.len()).min(self.data.len() - self.at);
            buf[..n].copy_from_slice(&self.data[self.at..self.at + n]);
            self.at += n;
            Ok(n)
        }
    }

    fn split(data: &[u8], step: usize) -> Vec<(String, bool)> {
        let (tx, rx) = mpsc::sync_channel(QUEUE);
        let pipe = Trickle {
            data: data.to_vec(),
            at: 0,
            step,
        };
        spawn_reader(pipe, Stream::Stdout, Instant::now(), tx)
            .unwrap()
            .join()
            .unwrap();
        rx.iter().map(|l| (l.text, l.terminated)).collect()
    }

    #[test]
    fn lines_split_on_lf_whatever_the_read_sizes() {
        let data = b"a\r\nbb\n\nc\r\r\nx\xffy\ntail\r";
        let want: Vec<(String, bool)> = vec![
            ("a".into(), true),
            ("bb".into(), true),
            ("".into(), true),
            ("c\r".into(), true),
            ("x\u{fffd}y".into(), true),
            ("tail".into(), false),
        ];
        for step in [1, 2, 3, 7, 64 * 1024] {
            assert_eq!(split(data, step), want, "step {step}");
        }
    }

    #[test]
    fn no_partial_line_after_a_final_newline() {
        assert_eq!(
            split(b"one\ntwo\n", 3),
            [("one".into(), true), ("two".into(), true)]
        );
        assert_eq!(split(b"", 1), []);
    }

    #[cfg(windows)]
    fn piped(program: &str, args: &[&str]) -> Command {
        let mut command = Command::new(program);
        command
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    /// The std-only fallback, driven on Windows so that it is compiled and run here too.
    #[cfg(windows)]
    #[test]
    fn portable_fallback_reports_exit_and_cancels_the_child() {
        let (tree, out, err) =
            portable::ChildTree::spawn(piped("cmd.exe", &["/d", "/c", "echo a& exit 3"])).unwrap();
        let mut lines = Vec::new();
        let outcome = drive(tree, out, err, &CancelToken::new(), &mut |l| {
            lines.push(l.text.clone())
        })
        .unwrap();
        assert_eq!((outcome.exit_code, outcome.cancelled), (Some(3), false));
        assert_eq!(lines, ["a"]);

        let (tree, out, err) =
            portable::ChildTree::spawn(piped("ping.exe", &["-n", "30", "127.0.0.1"])).unwrap();
        let token = CancelToken::new();
        let outcome = drive(tree, out, err, &token, &mut |_| token.cancel()).unwrap();
        assert_eq!((outcome.exit_code, outcome.cancelled), (None, true));
        assert!(outcome.elapsed_ms < 10_000.0, "{outcome:?}");
    }
}
