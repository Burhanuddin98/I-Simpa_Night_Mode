//! The tests' scratch folders (`common/scratch.rs`), in one process: a folder is removed when
//! the thread of the test that recorded it ends without a panic, and kept when it panicked, and
//! a thread with no name, which is never a test's own, may record none. Each case plays the
//! test on a thread of its own, as libtest runs every test, and looks at the folder once that
//! thread has been joined. The end-to-end check, on a real CLI test run by itself in a child
//! process, is `crates/simpa/tests/cli_run.rs`, `a_passing_test_leaves_no_scratch_behind_*`.

#[allow(dead_code)]
#[path = "common/scratch.rs"]
mod scratch;

use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

const GROUP: &str = "scratch_cleanup";

/// A folder with something in it, as a test leaves one: a nested folder and a file.
fn fill(dir: &std::path::Path) {
    std::fs::create_dir_all(dir.join("a/b")).unwrap();
    std::fs::write(dir.join("a/b/file.txt"), "written by the test").unwrap();
}

#[test]
fn a_passing_tests_folders_are_removed_when_its_thread_ends() {
    let (made, by_hand): (PathBuf, PathBuf) = thread::Builder::new()
        .name("a passing test".into())
        .spawn(|| {
            let made = scratch::fresh(GROUP, "passing");
            fill(&made);
            // A folder the test names itself and records by hand, as the parity bed does.
            let by_hand = scratch::root()
                .join(GROUP)
                .join(format!("by-hand-{}", std::process::id()));
            std::fs::create_dir_all(&by_hand).unwrap();
            scratch::own(&by_hand);
            fill(&by_hand);
            assert!(made.join("a/b/file.txt").is_file());
            (made, by_hand)
        })
        .unwrap()
        .join()
        .unwrap();
    assert!(!made.exists(), "{} is left", made.display());
    assert!(!by_hand.exists(), "{} is left", by_hand.display());
}

#[test]
fn a_failing_tests_folder_is_kept() {
    let (tx, rx) = mpsc::channel();
    let joined = thread::Builder::new()
        .name("a failing test".into())
        .spawn(move || {
            let dir = scratch::fresh(GROUP, "failing");
            fill(&dir);
            tx.send(dir).unwrap();
            panic!("a deliberate failure: its folder must be kept");
        })
        .unwrap()
        .join();
    assert!(joined.is_err(), "the failing test did not fail");
    let dir = rx.recv().unwrap();
    assert!(
        dir.join("a/b/file.txt").is_file(),
        "{} was removed although its test failed",
        dir.display()
    );
    // The kept folder is this test's own output now: recorded here, it goes when this test passes.
    scratch::own(&dir);
}

#[test]
fn a_thread_with_no_name_may_record_no_folder() {
    let label = format!("unnamed-{}", std::process::id());
    let prefix = format!("{label}-{}-", std::process::id());
    let joined = thread::spawn({
        let label = label.clone();
        move || scratch::fresh(GROUP, &label)
    })
    .join();
    let panic = joined.expect_err("a folder was recorded on a thread with no name");
    let message = panic.downcast_ref::<String>().cloned().unwrap_or_default();
    assert!(message.contains("on a thread with no name"), "{message}");
    // Refused before anything was made.
    let made: Vec<String> = std::fs::read_dir(scratch::root().join(GROUP))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with(&prefix))
        .collect();
    assert_eq!(made, Vec::<String>::new());
}
