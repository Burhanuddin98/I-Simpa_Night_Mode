//! Child processes for TetGen and the solvers: both output streams read concurrently and tagged,
//! a trailing partial line flushed at end of stream, and cancel. Milestone M5; see
//! `docs/m5-m6-design.md`.
//!
//! Scaffold: this implementation uses `std::process` only. The Job Object (`KILL_ON_JOB_CLOSE`),
//! `CREATE_NO_WINDOW` and `TerminateJobObject` cancel replace it without changing the interface.

use std::ffi::OsString;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

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

const POLL: Duration = Duration::from_millis(20);

/// Runs `spec` to completion or cancel. `on_line` runs on the calling thread, in arrival order,
/// and may call [`CancelToken::cancel`]. Returns `Err` only when the child cannot be started.
pub fn run(
    spec: &Spec,
    cancel: &CancelToken,
    on_line: &mut dyn FnMut(&Line),
) -> io::Result<Outcome> {
    let mut child = Command::new(&spec.program)
        .args(&spec.args)
        .current_dir(&spec.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let start = Instant::now();
    let (tx, rx) = mpsc::channel::<Line>();
    let readers = [
        spawn_reader(
            child.stdout.take().expect("piped"),
            Stream::Stdout,
            start,
            tx.clone(),
        ),
        spawn_reader(
            child.stderr.take().expect("piped"),
            Stream::Stderr,
            start,
            tx,
        ),
    ];
    let mut cancelled = false;
    let mut status = None;
    loop {
        match rx.recv_timeout(POLL) {
            Ok(line) => on_line(&line),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        if !cancelled && cancel.is_cancelled() {
            cancelled = true;
            let _ = child.kill();
        }
        if status.is_none() {
            status = child.try_wait()?;
        }
    }
    let status = match status {
        Some(s) => s,
        None => child.wait()?,
    };
    for r in readers {
        let _ = r.join();
    }
    Ok(Outcome {
        exit_code: if cancelled {
            None
        } else {
            status.code().map(|c| c as u32)
        },
        cancelled,
        elapsed_ms: start.elapsed().as_secs_f64() * 1e3,
    })
}

fn spawn_reader(
    mut pipe: impl Read + Send + 'static,
    stream: Stream,
    start: Instant,
    tx: mpsc::Sender<Line>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut pending: Vec<u8> = Vec::new();
        let mut buf = [0u8; 8192];
        let emit = |bytes: &[u8], terminated: bool| {
            let bytes = bytes.strip_suffix(b"\r").unwrap_or(bytes);
            let _ = tx.send(Line {
                stream,
                t_ms: start.elapsed().as_secs_f64() * 1e3,
                text: String::from_utf8_lossy(bytes).into_owned(),
                terminated,
            });
        };
        loop {
            match pipe.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    pending.extend_from_slice(&buf[..n]);
                    while let Some(i) = pending.iter().position(|&b| b == b'\n') {
                        emit(&pending[..i], true);
                        pending.drain(..=i);
                    }
                }
            }
        }
        if !pending.is_empty() {
            emit(&pending, false);
        }
    })
}
