//! Shared helpers for the format tests.
//!
//! Fuzz tests install [`Tracking`] as the global allocator of their own test binary and measure
//! the peak between [`reset_peak`] and [`peak_since_reset`]. The counters are per thread, so the
//! measurement covers the reader call on the test's own thread and sibling tests running in
//! parallel do not count against it (they did while the counters were process-wide).
//!
//! Scope limit: an allocation made on ANOTHER thread is invisible to the measurement. Every
//! reader in `formats` is single-threaded today; a reader that spawns threads would need a
//! different measurement before its budget assertion means anything.
#![allow(dead_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;
use std::path::PathBuf;

pub struct Tracking;

thread_local! {
    // Const-initialised: touching them never allocates, so the allocator cannot recurse.
    static CURRENT: Cell<usize> = const { Cell::new(0) };
    static PEAK: Cell<usize> = const { Cell::new(0) };
}

fn note_alloc(size: usize) {
    // try_with: during thread teardown the slots may already be gone; skip rather than panic.
    let _ = CURRENT.try_with(|c| {
        let now = c.get() + size;
        c.set(now);
        let _ = PEAK.try_with(|p| {
            if now > p.get() {
                p.set(now);
            }
        });
    });
}

fn note_free(size: usize) {
    // Memory allocated on another thread may be freed here; saturate instead of wrapping.
    let _ = CURRENT.try_with(|c| c.set(c.get().saturating_sub(size)));
}

unsafe impl GlobalAlloc for Tracking {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            note_alloc(layout.size());
        }
        p
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        note_free(layout.size());
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let p = unsafe { System.realloc(ptr, layout, new_size) };
        if !p.is_null() {
            note_free(layout.size());
            note_alloc(new_size);
        }
        p
    }
}

/// Starts a measurement on this thread; returns the bytes live here now (the baseline).
pub fn reset_peak() -> usize {
    let now = CURRENT.with(Cell::get);
    PEAK.with(|p| p.set(now));
    now
}

/// Peak bytes this thread allocated above `baseline` since [`reset_peak`].
pub fn peak_since_reset(baseline: usize) -> usize {
    PEAK.with(Cell::get).saturating_sub(baseline)
}

/// Allocation budget a reader may use for an input of `len` bytes (gate M2(c)).
pub fn budget(len: usize) -> usize {
    2 * len + 64 * 1024
}

/// Path of a file under `tests/fixtures/`.
pub fn fixture(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(rel)
}
