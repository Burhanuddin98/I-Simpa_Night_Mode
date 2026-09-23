//! A stand-in solver for the run-folder tests (`docs/m5-m6-design.md`, "Stub solver"). It plays
//! back `stub.json` from its working folder, exactly:
//!
//! ```json
//! {
//!   "exit_code": 0,
//!   "lines": [{"stream": "stdout|stderr", "text": "...", "newline": true, "delay_ms": 0}],
//!   "files": {"relative/path": "contents"}
//! }
//! ```
//!
//! Each line is written to its stream after its delay, with a `\n` only when `newline` is true,
//! and flushed at once, so a final line without a newline reaches the reader as a partial line.
//! Then each file is written (its folders created), and the process exits with `exit_code` as
//! the raw 32-bit value (`0xC0000005` and -1 alike). It takes exactly one argument,
//! `config.xml`, as the solvers are launched (contract Part B, "Launch"); anything else is exit
//! 97 with a message on stderr, so a launcher that passes another argument fails its test.

use std::io::Write;
use std::path::Path;
use std::time::Duration;

use serde_json::Value;

/// The exit code for a stub that cannot play back its script.
const STUB_ERROR: i32 = 97;

fn fail(msg: &str) -> ! {
    eprintln!("simpa-stub-solver: {msg}");
    std::process::exit(STUB_ERROR)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args != ["config.xml"] {
        fail(&format!(
            "expected the single argument config.xml, got {args:?}"
        ));
    }
    let text = std::fs::read_to_string("stub.json")
        .unwrap_or_else(|e| fail(&format!("stub.json in the working folder: {e}")));
    let spec: Value =
        serde_json::from_str(&text).unwrap_or_else(|e| fail(&format!("stub.json: {e}")));

    let lines = spec["lines"]
        .as_array()
        .unwrap_or_else(|| fail("stub.json: \"lines\" is not an array"));
    for (i, l) in lines.iter().enumerate() {
        let field = |name: &str| &l[name];
        let text = field("text")
            .as_str()
            .unwrap_or_else(|| fail(&format!("line {i}: no text")));
        let newline = field("newline")
            .as_bool()
            .unwrap_or_else(|| fail(&format!("line {i}: newline must be true or false")));
        let delay = field("delay_ms").as_u64().unwrap_or(0);
        if delay > 0 {
            std::thread::sleep(Duration::from_millis(delay));
        }
        let mut bytes = text.as_bytes().to_vec();
        if newline {
            bytes.push(b'\n');
        }
        let written = match field("stream").as_str() {
            Some("stdout") => {
                let mut out = std::io::stdout().lock();
                out.write_all(&bytes).and_then(|()| out.flush())
            }
            Some("stderr") => {
                let mut err = std::io::stderr().lock();
                err.write_all(&bytes).and_then(|()| err.flush())
            }
            other => fail(&format!(
                "line {i}: stream {other:?} is not stdout or stderr"
            )),
        };
        if let Err(e) = written {
            fail(&format!("line {i}: {e}"));
        }
    }

    if let Some(files) = spec.get("files").and_then(Value::as_object) {
        for (rel, contents) in files {
            let contents = contents
                .as_str()
                .unwrap_or_else(|| fail(&format!("file {rel}: contents must be a string")));
            let path = Path::new(rel);
            if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
                std::fs::create_dir_all(dir).unwrap_or_else(|e| fail(&format!("{rel}: {e}")));
            }
            std::fs::write(path, contents).unwrap_or_else(|e| fail(&format!("{rel}: {e}")));
        }
    }

    // The raw 32-bit exit code: a u32 such as 3221225477 (0xC0000005) or an i32 such as -1.
    let code = match &spec["exit_code"] {
        Value::Number(n) => n
            .as_u64()
            .and_then(|u| u32::try_from(u).ok())
            .map(|u| u as i32)
            .or_else(|| n.as_i64().and_then(|i| i32::try_from(i).ok())),
        _ => None,
    }
    .unwrap_or_else(|| fail("stub.json: exit_code must be a 32-bit integer"));
    std::process::exit(code)
}
