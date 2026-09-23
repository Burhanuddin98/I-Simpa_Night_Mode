//! `app.exe --selftest <out.json>`: the window opens, the UI measures itself (WebGL renderer,
//! IPC benchmark, panic probe, run-event channel, exact-float bridge, step bar) and reports as
//! JSON text; this module adds what only the backend knows and writes the file. Exit code 0 when
//! the UI's checks all passed, 1 when any failed, 3 when the UI never reported (a watchdog writes
//! a failure file first, so the gate never hangs and never reads a stale file).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value, json};

use crate::guard::{CmdError, CmdResult};

pub const TIMEOUT: Duration = Duration::from_secs(180);
pub const EXIT_TIMEOUT: i32 = 3;

pub struct Selftest {
    pub out: PathBuf,
    reported: AtomicBool,
}

impl Selftest {
    pub fn new(out: PathBuf) -> Self {
        Selftest {
            out,
            reported: AtomicBool::new(false),
        }
    }

    /// Writes a failure file and exits with [`EXIT_TIMEOUT`] if nothing was reported in time.
    pub fn start_watchdog(self: &std::sync::Arc<Self>, timeout: Duration) {
        let me = self.clone();
        std::thread::spawn(move || {
            std::thread::sleep(timeout);
            if !me.reported.swap(true, Ordering::SeqCst) {
                let mut report = Map::new();
                report.insert("ok".into(), Value::Bool(false));
                report.insert(
                    "error".into(),
                    json!({
                        "code": "SELFTEST_TIMEOUT",
                        "message": format!("the UI did not report within {} s", timeout.as_secs()),
                    }),
                );
                report.insert("meta".into(), meta());
                let _ = write_atomic(&me.out, &Value::Object(report));
                std::process::exit(EXIT_TIMEOUT);
            }
        });
    }

    /// Takes the UI's report (JSON text), adds `meta`, writes the file; returns the UI's `ok`.
    pub fn report(&self, ui_text: &str) -> CmdResult<bool> {
        if self.reported.swap(true, Ordering::SeqCst) {
            return Err(CmdError::new(
                "SELFTEST_ALREADY_REPORTED",
                "the self-test report was already written",
            ));
        }
        let (value, ok) = compose(ui_text, meta())?;
        write_atomic(&self.out, &value).map_err(|e| {
            CmdError::new(
                "SELFTEST_WRITE",
                format!("could not write {}: {e}", self.out.display()),
            )
        })?;
        Ok(ok)
    }
}

/// The UI's report with `meta` added. The text is read with the core's exact JSON reader, so the
/// measured milliseconds reach the file with the bits the UI sent.
pub fn compose(ui_text: &str, meta: Value) -> CmdResult<(Value, bool)> {
    let parsed = simpa_core::schema::parse_json(ui_text)
        .map_err(|e| CmdError::new("SELFTEST_REPORT_SYNTAX", format!("report is not JSON: {e}")))?;
    let Value::Object(mut report) = parsed else {
        return Err(CmdError::new(
            "SELFTEST_REPORT_SHAPE",
            "report is not a JSON object",
        ));
    };
    if report.contains_key("meta") {
        return Err(CmdError::new(
            "SELFTEST_REPORT_SHAPE",
            "'meta' is written by the backend, not the UI",
        ));
    }
    let ok = report.get("ok") == Some(&Value::Bool(true));
    report.insert("meta".into(), meta);
    Ok((Value::Object(report), ok))
}

/// What only the backend knows: machine, versions, time.
pub fn meta() -> Value {
    json!({
        "written_utc": utc_rfc3339(SystemTime::now()),
        "machine": std::env::var("COMPUTERNAME").ok(),
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "app_version": env!("CARGO_PKG_VERSION"),
        "core_version": simpa_core::VERSION,
        "solver_commit": simpa_core::SOLVER_COMMIT,
        "tauri_version": tauri::VERSION,
        "webview": crate::webview2::info(),
        "debug_build": cfg!(debug_assertions),
    })
}

fn write_atomic(path: &Path, value: &Value) -> std::io::Result<()> {
    let mut text = serde_json::to_string_pretty(value).map_err(std::io::Error::other)?;
    text.push('\n');
    let tmp = path.with_extension("json.partial");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

/// `YYYY-MM-DDTHH:MM:SSZ` for a time after 1970 (days-from-civil, H. Hinnant).
pub fn utc_rfc3339(t: SystemTime) -> String {
    let secs = t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_formatting() {
        let at = |s| utc_rfc3339(UNIX_EPOCH + Duration::from_secs(s));
        assert_eq!(at(0), "1970-01-01T00:00:00Z");
        assert_eq!(at(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(at(1_790_121_599), "2026-09-22T23:59:59Z");
    }

    #[test]
    fn compose_adds_meta_and_reads_ok() {
        let (v, ok) = compose(r#"{"ok": true, "ipc": {"single_16MB_ms": 0.1}}"#, json!(1)).unwrap();
        assert!(ok);
        assert_eq!(v["meta"], json!(1));
        assert_eq!(v["ipc"]["single_16MB_ms"], json!(0.1));
        let (_, ok) = compose(r#"{"ok": "yes"}"#, json!(1)).unwrap();
        assert!(!ok);
    }

    #[test]
    fn compose_refuses_bad_reports() {
        assert_eq!(
            compose("{", json!(1)).unwrap_err().code,
            "SELFTEST_REPORT_SYNTAX"
        );
        assert_eq!(
            compose("[1]", json!(1)).unwrap_err().code,
            "SELFTEST_REPORT_SHAPE"
        );
        assert_eq!(
            compose(r#"{"meta": 1}"#, json!(1)).unwrap_err().code,
            "SELFTEST_REPORT_SHAPE"
        );
    }

    #[test]
    fn report_writes_once() {
        let dir = std::env::temp_dir().join(format!("simpa-app-selftest-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("out.json");
        let st = Selftest::new(out.clone());
        assert_eq!(st.report(r#"{"ok": true}"#), Ok(true));
        let written: Value = serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
        assert_eq!(written["ok"], json!(true));
        assert!(written["meta"]["tauri_version"].is_string());
        assert_eq!(
            st.report(r#"{"ok": true}"#).unwrap_err().code,
            "SELFTEST_ALREADY_REPORTED"
        );
    }
}
