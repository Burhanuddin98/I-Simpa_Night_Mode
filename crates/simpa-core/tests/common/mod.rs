//! Shared helpers for the format tests.
//!
//! Fuzz tests install [`Tracking`] as the global allocator of their own test binary and measure
//! the peak between [`reset_peak`] and [`peak_since_reset`]. The counters are process-wide, so run
//! fuzz binaries with `--test-threads=1`.
#![allow(dead_code)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Tracking;

static CURRENT: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn note_alloc(size: usize) {
    let now = CURRENT.fetch_add(size, Ordering::SeqCst) + size;
    PEAK.fetch_max(now, Ordering::SeqCst);
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
        CURRENT.fetch_sub(layout.size(), Ordering::SeqCst);
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let p = unsafe { System.realloc(ptr, layout, new_size) };
        if !p.is_null() {
            CURRENT.fetch_sub(layout.size(), Ordering::SeqCst);
            note_alloc(new_size);
        }
        p
    }
}

/// Starts a measurement; returns the bytes live at this moment (the baseline).
pub fn reset_peak() -> usize {
    let now = CURRENT.load(Ordering::SeqCst);
    PEAK.store(now, Ordering::SeqCst);
    now
}

/// Peak bytes allocated above `baseline` since [`reset_peak`].
pub fn peak_since_reset(baseline: usize) -> usize {
    PEAK.load(Ordering::SeqCst).saturating_sub(baseline)
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
