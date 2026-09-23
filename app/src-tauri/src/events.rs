//! Run events for the UI: one `ipc::Channel` per run, events batched about every 50 ms.
//!
//! A solver can print thousands of lines a second; one IPC message per line would flood the
//! WebView2 UI thread. The [`Batcher`] collects events on its own thread and sends a
//! [`RunEventBatch`] at most [`BATCH_PERIOD`] after the first event of the batch arrived, so the
//! UI sees every line, in order, at most one period late. The last batch has `last: true`, sent
//! when the producer finishes, so the UI knows the stream is complete.
//!
//! The run manager (M6) will be the producer; until then `run_events_probe` drives it with
//! synthetic lines.

use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use schemars::JsonSchema;
use serde::Serialize;

/// How long the first event of a batch may wait before the batch is sent.
pub const BATCH_PERIOD: Duration = Duration::from_millis(50);

/// Where a line came from. Lines are classified by content, never by stream (laydown, Traps).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Stream {
    Stdout,
    Stderr,
}

/// The Console's classes (docs/design/README.md: FAIL, INFO and OK; WARN for the amber lines).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum LineClass {
    Fail,
    Warn,
    Info,
    Ok,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct RunEvent {
    /// 0, 1, 2, ... per run, so the UI can prove it lost nothing and kept the order.
    pub seq: u64,
    /// Milliseconds since the run started.
    pub t_ms: f64,
    pub stream: Stream,
    pub class: LineClass,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, JsonSchema)]
pub struct RunEventBatch {
    /// 0, 1, 2, ... per run.
    pub batch: u64,
    pub events: Vec<RunEvent>,
    /// The producer has finished; no batch follows.
    pub last: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, JsonSchema)]
pub struct BatchStats {
    pub events: u64,
    pub batches: u64,
    /// Batches the sink refused (the webview went away, for example).
    pub send_failures: u64,
}

/// Collects events on its own thread and hands them to a sink in batches.
pub struct Batcher {
    tx: mpsc::Sender<RunEvent>,
    thread: JoinHandle<BatchStats>,
}

impl Batcher {
    /// Starts the batching thread. `sink` returns whether the batch was delivered.
    pub fn spawn<S>(period: Duration, mut sink: S) -> Self
    where
        S: FnMut(RunEventBatch) -> bool + Send + 'static,
    {
        let (tx, rx) = mpsc::channel::<RunEvent>();
        let thread = std::thread::spawn(move || {
            let mut stats = BatchStats::default();
            let mut pending: Vec<RunEvent> = Vec::new();
            let mut deadline: Option<Instant> = None;
            let mut flush = |events: Vec<RunEvent>, last: bool, stats: &mut BatchStats| {
                stats.events += events.len() as u64;
                let batch = RunEventBatch {
                    batch: stats.batches,
                    events,
                    last,
                };
                stats.batches += 1;
                if !sink(batch) {
                    stats.send_failures += 1;
                }
            };
            loop {
                let received = match deadline {
                    None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
                    Some(d) => rx.recv_timeout(d.saturating_duration_since(Instant::now())),
                };
                match received {
                    Ok(event) => {
                        if pending.is_empty() {
                            deadline = Some(Instant::now() + period);
                        }
                        pending.push(event);
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => {
                        flush(std::mem::take(&mut pending), true, &mut stats);
                        return stats;
                    }
                }
                // A steady stream never times out, so the deadline is checked after every event.
                if deadline.is_some_and(|d| Instant::now() >= d) {
                    flush(std::mem::take(&mut pending), false, &mut stats);
                    deadline = None;
                }
            }
        });
        Batcher { tx, thread }
    }

    pub fn push(&self, event: RunEvent) {
        // The receiver lives until `finish`, which consumes `self`, so this cannot fail.
        let _ = self.tx.send(event);
    }

    /// Sends what is left as the `last` batch and returns the totals.
    pub fn finish(self) -> BatchStats {
        drop(self.tx);
        self.thread
            .join()
            .unwrap_or_else(|p| std::panic::resume_unwind(p))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn event(seq: u64) -> RunEvent {
        RunEvent {
            seq,
            t_ms: 0.0,
            stream: Stream::Stdout,
            class: LineClass::Info,
            text: format!("line {seq}"),
        }
    }

    type Received = Arc<Mutex<Vec<(Instant, RunEventBatch)>>>;

    fn collect(period: Duration) -> (Batcher, Received) {
        let got = Arc::new(Mutex::new(Vec::new()));
        let sink_got = got.clone();
        let b = Batcher::spawn(period, move |batch| {
            sink_got.lock().unwrap().push((Instant::now(), batch));
            true
        });
        (b, got)
    }

    #[test]
    fn every_event_arrives_once_in_order_and_the_last_batch_is_marked() {
        let (b, got) = collect(Duration::from_millis(20));
        let start = Instant::now();
        for i in 0..400 {
            let due = start + Duration::from_micros(250 * i);
            std::thread::sleep(due.saturating_duration_since(Instant::now()));
            b.push(event(i));
        }
        let stats = b.finish();
        let got = got.lock().unwrap();
        let seqs: Vec<u64> = got
            .iter()
            .flat_map(|(_, b)| b.events.iter().map(|e| e.seq))
            .collect();
        assert_eq!(seqs, (0..400).collect::<Vec<_>>());
        assert_eq!(stats.events, 400);
        assert_eq!(stats.batches as usize, got.len());
        assert!(got.last().unwrap().1.last);
        assert_eq!(got.iter().filter(|(_, b)| b.last).count(), 1);
        // 100 ms of events in 20 ms batches: a handful of batches, not 400 messages.
        assert!(
            (3..=15).contains(&got.len()),
            "{} batches for 100 ms",
            got.len()
        );
        let numbers: Vec<u64> = got.iter().map(|(_, b)| b.batch).collect();
        assert_eq!(numbers, (0..got.len() as u64).collect::<Vec<_>>());
    }

    #[test]
    fn a_lone_event_waits_at_most_one_period() {
        let (b, got) = collect(Duration::from_millis(30));
        let t0 = Instant::now();
        b.push(event(0));
        std::thread::sleep(Duration::from_millis(150));
        {
            let got = got.lock().unwrap();
            assert_eq!(got.len(), 1, "sent before the producer finished");
            let waited = got[0].0 - t0;
            assert!(waited >= Duration::from_millis(30), "{waited:?}");
            assert!(waited < Duration::from_millis(120), "{waited:?}");
            assert!(!got[0].1.last);
        }
        let stats = b.finish();
        assert_eq!(stats.batches, 2, "the data batch, then an empty last batch");
    }

    #[test]
    fn a_refused_batch_is_counted() {
        let b = Batcher::spawn(Duration::from_millis(5), |_| false);
        b.push(event(0));
        assert_eq!(b.finish().send_failures, 1);
    }
}
