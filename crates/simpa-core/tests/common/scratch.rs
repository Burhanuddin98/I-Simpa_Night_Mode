// Scratch folders for the tests: each is removed when the test that made it passes, and kept,
// with its path printed, when that test fails. Every test target that writes files goes through
// this file (`mod common;` targets as `common::scratch`, the `*_support.rs` helpers and the CLI's
// `tests/support/mod.rs` with `#[path]`), so no test leaves its folders behind.
//
// Why: until 2026-09-25 no test removed anything, on the grounds that `target/` is build output.
// 368,032 files had piled up under `target/tmp` and `target/test-runs` of one worktree, about
// 47 GB on B:'s exFAT with its 128 KB clusters, and the disk ran out (the 19:00 disk emergency).
//
// How:
// - [`own`] records a folder against the calling test's thread; [`fresh`] makes a new, empty,
//   never-reused folder and records it. libtest runs every test on a thread of its own, named
//   after the test, and joins that thread before it reports the next result, so the record ends
//   with the test.
// - When the thread ends, each folder it recorded is removed, unless the test failed. A panic
//   hook, installed with the first record, marks the panicking thread as failed and prints
//   `scratch kept for debugging: <path>` for each of its folders, in the failed test's own
//   output beside the panic message. The folders are then left where they are, under
//   `target/tmp/<group>/` (cargo's `CARGO_TARGET_TMPDIR`), `target/test-runs/config_xml/` or
//   `target/parity-bed/`.
// - A folder that cannot be removed (a file still held open by a process that outlived the
//   test) is tried again for about 3 s, then left, with `scratch: could not remove <path>` on
//   stderr. It never fails the test: the removal runs as the test's thread ends, after its
//   result, where a panic would abort the whole test binary.
// - Only the test's own thread may record a folder. A thread the test spawns has no name, and
//   ends before the test does: a folder recorded there would be removed while the test still used
//   it, so recording one there panics. Make the folder first, then spawn.
// - A folder shared by several tests of one binary (a `static` built once) belongs to no single
//   test: keep what the tests share in memory instead, or build it once per test.
//
// Two variables change this, for debugging and for the check that the cleanup works
// (`crates/simpa/tests/cli_run.rs`, `a_passing_test_leaves_no_scratch_behind_*`):
// - `$SIMPA_KEEP_SCRATCH=1` keeps every folder, passed or not;
// - `$SIMPA_TEST_SCRATCH_ROOT` takes the place of `CARGO_TARGET_TMPDIR` as the root of [`fresh`].
//
// Like `paths.rs`, this file has no inner attributes or `//!` docs, so that any test target can
// include it.

use std::cell::{Cell, RefCell};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Once;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

/// Set to a folder, it is the root of [`fresh`] instead of cargo's `target/tmp`.
pub const ROOT_ENV: &str = "SIMPA_TEST_SCRATCH_ROOT";
/// Set to `1`, every scratch folder is kept, whether its test passed or not.
pub const KEEP_ENV: &str = "SIMPA_KEEP_SCRATCH";
/// What a failed test prints before the path of each folder it keeps.
pub const KEPT: &str = "scratch kept for debugging:";

/// The folders the current thread's test recorded, and whether that test failed.
struct Owned {
    failed: Cell<bool>,
    dirs: RefCell<Vec<PathBuf>>,
}

impl Drop for Owned {
    fn drop(&mut self) {
        let dirs = std::mem::take(self.dirs.get_mut());
        if dirs.is_empty() || self.failed.get() {
            return;
        }
        if keep_all() {
            for d in &dirs {
                let _ = writeln!(
                    std::io::stderr(),
                    "scratch kept ({KEEP_ENV}): {}",
                    d.display()
                );
            }
            return;
        }
        // Newest first: a folder recorded inside another goes before it.
        for d in dirs.iter().rev() {
            remove(d);
        }
    }
}

thread_local! {
    static OWNED: Owned = const {
        Owned {
            failed: Cell::new(false),
            dirs: RefCell::new(Vec::new()),
        }
    };
}

fn keep_all() -> bool {
    std::env::var_os(KEEP_ENV).is_some_and(|v| v == "1")
}

/// Removes `dir` and everything in it; a folder already gone is fine. Retried for about 3 s, as a
/// process the test ran may still be letting go of a file; after that, left and named on stderr.
fn remove(dir: &Path) {
    let mut wait = Duration::from_millis(25);
    for attempt in 0..8 {
        match std::fs::remove_dir_all(dir) {
            Ok(()) => return,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
            Err(e) if attempt == 7 => {
                let _ = writeln!(
                    std::io::stderr(),
                    "scratch: could not remove {}: {e}",
                    dir.display()
                );
            }
            Err(_) => {
                std::thread::sleep(wait);
                wait *= 2;
            }
        }
    }
}

/// Marks the panicking thread's test as failed and names its folders, after the panic message.
fn install_hook() {
    static HOOK: Once = Once::new();
    HOOK.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            previous(info);
            let _ = OWNED.try_with(|o| {
                o.failed.set(true);
                if let Ok(dirs) = o.dirs.try_borrow() {
                    for d in dirs.iter() {
                        eprintln!("{KEPT} {}", d.display());
                    }
                }
            });
        }));
    });
}

/// Records `dir` against the calling test: removed when it passes, kept when it fails. Panics on
/// a thread with no name, which is not a test's own thread (see the file's header).
pub fn own(dir: &Path) {
    let current = std::thread::current();
    assert!(
        current.name().is_some(),
        "scratch folder {} recorded on a thread with no name: only the test's own thread, which \
         libtest names after the test, may record one (make the folder before spawning)",
        dir.display()
    );
    install_hook();
    OWNED.with(|o| o.dirs.borrow_mut().push(dir.to_path_buf()));
}

/// `$SIMPA_TEST_SCRATCH_ROOT`, else cargo's `target/tmp` for this test target.
pub fn root() -> PathBuf {
    match std::env::var_os(ROOT_ENV) {
        Some(d) if !d.is_empty() => PathBuf::from(d),
        _ => PathBuf::from(env!("CARGO_TARGET_TMPDIR")),
    }
}

/// A new, empty folder `<root>/<group>/<label>-<pid>-<n>-<ns>`, never reused, recorded against
/// the calling test ([`own`]).
pub fn fresh(group: &str, label: &str) -> PathBuf {
    fresh_in(&root().join(group), label)
}

/// The same under `base` instead of `<root>/<group>`.
pub fn fresh_in(base: &Path, label: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = base.join(format!(
        "{label}-{}-{}-{nanos}",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    // Recorded first: a refusal (a thread with no name) then leaves nothing behind.
    own(&dir);
    std::fs::create_dir_all(&dir).unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
    dir
}
