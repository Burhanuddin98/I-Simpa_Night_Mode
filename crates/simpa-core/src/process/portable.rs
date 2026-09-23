//! Anything but Windows: std only. There is no job, so cancel, the end of the run and drop kill
//! the direct child and nothing it started. On Windows this is compiled for its unit test only.

use std::io;
use std::process::{Child, ChildStderr, ChildStdout, Command, ExitStatus};
use std::thread;
use std::time::Duration;

use super::Tree;

/// The direct child. Dropping it kills the child if it still runs.
pub(super) struct ChildTree(Child);

impl ChildTree {
    /// Spawns `command`, whose stdout and stderr must be piped.
    pub(super) fn spawn(mut command: Command) -> io::Result<(Self, ChildStdout, ChildStderr)> {
        let mut child = command.spawn()?;
        let stdout = child.stdout.take().expect("stdout is piped");
        let stderr = child.stderr.take().expect("stderr is piped");
        Ok((ChildTree(child), stdout, stderr))
    }
}

impl Tree for ChildTree {
    fn wait_exit(&mut self, timeout: Duration) -> io::Result<Option<ExitStatus>> {
        match self.0.try_wait()? {
            None if !timeout.is_zero() => {
                thread::sleep(timeout);
                self.0.try_wait()
            }
            status => Ok(status),
        }
    }

    fn kill_all(&mut self) -> io::Result<()> {
        if self.0.try_wait()?.is_none() {
            // `kill` on a child that exited meanwhile is `Ok`.
            self.0.kill()?;
        }
        self.0.wait().map(drop)
    }
}

impl Drop for ChildTree {
    fn drop(&mut self) {
        if let Ok(None) = self.0.try_wait() {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
}
