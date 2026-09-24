//! The [`Mesher`] trait and its one real backend, [`TetgenMesher`]: upstream's `tetgen.exe` run
//! through [`crate::process::run`].

use std::ffi::OsString;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use crate::process::{self, CancelToken, Line, Outcome, Spec, Stream};

/// Log file for the mesher's stdout, in the folder it ran in.
pub const STDOUT_LOG: &str = "tetgen.stdout.txt";
/// Log file for the mesher's stderr, in the folder it ran in.
pub const STDERR_LOG: &str = "tetgen.stderr.txt";

/// Something that turns a `.poly` into TetGen's output files.
pub trait Mesher {
    /// The program, for the manifest; `None` when there is no file to hash.
    fn program(&self) -> Option<&Path>;

    /// Runs one meshing call with `dir` as the working folder and `args` as given, the input
    /// named relative to `dir`. Every output line goes to `on_line` in arrival order; `cancel`
    /// stops the call. `Err` when the call cannot start, or its output cannot be logged.
    fn run(
        &self,
        dir: &Path,
        args: &[String],
        cancel: &CancelToken,
        on_line: &mut dyn FnMut(&Line),
    ) -> io::Result<Outcome>;
}

/// Upstream's `tetgen.exe`. It runs with the working folder set to the mesh folder and the
/// `.poly` named relatively, because TetGen opens its files with the narrow `fopen`
/// (`docs/m5-m6-design.md`, "Layout"), and it logs its streams to [`STDOUT_LOG`] and
/// [`STDERR_LOG`] in that folder, line by line as they arrive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TetgenMesher {
    pub exe: PathBuf,
}

impl TetgenMesher {
    pub fn new(exe: impl Into<PathBuf>) -> Self {
        TetgenMesher { exe: exe.into() }
    }
}

impl Mesher for TetgenMesher {
    fn program(&self) -> Option<&Path> {
        Some(&self.exe)
    }

    fn run(
        &self,
        dir: &Path,
        args: &[String],
        cancel: &CancelToken,
        on_line: &mut dyn FnMut(&Line),
    ) -> io::Result<Outcome> {
        run_logged(
            &self.exe,
            dir,
            args,
            cancel,
            on_line,
            (STDOUT_LOG, STDERR_LOG),
        )
    }
}

/// Runs `exe` with `dir` as its working folder through [`process::run`] (a Job Object, killed on
/// cancel), logging its two streams line by line as they arrive to `logs` (stdout, stderr) in
/// that folder. `Err` when `exe` is not a file, it cannot start, or a log cannot be written.
pub(super) fn run_logged(
    exe: &Path,
    dir: &Path,
    args: &[String],
    cancel: &CancelToken,
    on_line: &mut dyn FnMut(&Line),
    logs: (&str, &str),
) -> io::Result<Outcome> {
    if !exe.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{} is not a file", exe.display()),
        ));
    }
    let mut out = BufWriter::new(File::create(dir.join(logs.0))?);
    let mut err = BufWriter::new(File::create(dir.join(logs.1))?);
    let mut write_error: Option<io::Error> = None;
    let spec = Spec {
        program: exe.to_path_buf(),
        args: args.iter().map(OsString::from).collect(),
        cwd: dir.to_path_buf(),
    };
    let outcome = process::run(&spec, cancel, &mut |line: &Line| {
        let log = match line.stream {
            Stream::Stdout => &mut out,
            Stream::Stderr => &mut err,
        };
        let eol = if line.terminated { "\n" } else { "" };
        if let Err(e) = write!(log, "{}{eol}", line.text)
            && write_error.is_none()
        {
            write_error = Some(e);
        }
        on_line(line);
    })?;
    out.flush()?;
    err.flush()?;
    match write_error {
        Some(e) => Err(e),
        None => Ok(outcome),
    }
}
