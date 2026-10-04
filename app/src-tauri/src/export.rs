//! W9 export (wow list; parity R56, R63; docs/investigations/2026-10-04-wow-w2w3w9/PLAN.md): the
//! one place the app writes a file the person asked for, other than the project itself.
//!
//! The bytes are the UI's (`app/ui/src/features/export/exportModel.ts` for the parameters, the
//! frame read back for the view); this module reads nothing of a run. The path is the one the save
//! dialog returned (the e2e passes one as its save-as hook does). What is checked here, so a wrong
//! call writes nothing:
//! - the kind is one of `csv`, `json`, `png`, and the path is absolute with that extension;
//! - its folder exists, and the path is not a folder;
//! - the bytes are what the kind says: UTF-8 for CSV, a JSON document for JSON, a PNG signature;
//! - the file is written whole or not at all: a temporary file beside it, then renamed over it.

use std::path::{Path, PathBuf};

use crate::guard::{CmdError, CmdResult};

pub const KINDS: [&str; 3] = ["csv", "json", "png"];
const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// Decodes a header's `encodeURIComponent` text: `%XX` bytes, then UTF-8.
pub fn percent_decode(s: &str) -> CmdResult<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = s
                .get(i + 1..i + 3)
                .and_then(|h| u8::from_str_radix(h, 16).ok())
                .ok_or_else(|| {
                    CmdError::new(
                        "EXPORT_PATH",
                        format!("the path header is not percent-encoded at byte {i}"),
                    )
                })?;
            out.push(hex);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).map_err(|_| CmdError::new("EXPORT_PATH", "the path is not UTF-8"))
}

/// The path to write, refused unless absolute, with the kind's extension, in a folder that exists.
pub fn checked_path(kind: &str, path: &str) -> CmdResult<PathBuf> {
    if !KINDS.contains(&kind) {
        return Err(CmdError::new(
            "EXPORT_KIND",
            format!("'{kind}' is not an export kind: csv, json or png"),
        ));
    }
    let p = PathBuf::from(path);
    if !p.is_absolute() {
        return Err(CmdError::new(
            "EXPORT_PATH",
            format!("'{path}' is not an absolute path"),
        ));
    }
    let ext = p
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    if ext.as_deref() != Some(kind) {
        return Err(CmdError::new(
            "EXPORT_PATH",
            format!(
                "'{}' does not end in .{kind}: a {kind} export is written only to a .{kind} file",
                p.display()
            ),
        ));
    }
    if p.is_dir() {
        return Err(CmdError::new(
            "EXPORT_PATH",
            format!("'{}' is a folder", p.display()),
        ));
    }
    match p.parent() {
        Some(dir) if dir.is_dir() => Ok(p),
        _ => Err(CmdError::new(
            "EXPORT_PATH",
            format!("the folder of '{}' does not exist", p.display()),
        )),
    }
}

/// Refuses bytes that are not what the kind says.
pub fn check_content(kind: &str, bytes: &[u8]) -> CmdResult<()> {
    let bad = |why: &str| {
        Err(CmdError::new(
            "EXPORT_CONTENT",
            format!("not a {kind} file: {why}"),
        ))
    };
    match kind {
        "png" if !bytes.starts_with(&PNG_SIGNATURE) => bad("no PNG signature"),
        "csv" if std::str::from_utf8(bytes).is_err() => bad("not UTF-8 text"),
        "json" if serde_json::from_slice::<serde_json::Value>(bytes).is_err() => {
            bad("not a JSON document")
        }
        _ if bytes.is_empty() => bad("empty"),
        _ => Ok(()),
    }
}

/// Writes `bytes` to `path` whole: a temporary file in the same folder, then renamed over the path.
pub fn write(kind: &str, path: &str, bytes: &[u8]) -> CmdResult<u64> {
    let p = checked_path(kind, path)?;
    check_content(kind, bytes)?;
    let io = |what: &str, e: std::io::Error| {
        CmdError::new("EXPORT_IO", format!("{what} '{}': {e}", p.display()))
    };
    let tmp = temp_beside(&p);
    std::fs::write(&tmp, bytes).map_err(|e| io("could not write beside", e))?;
    if let Err(e) = std::fs::rename(&tmp, &p) {
        let _ = std::fs::remove_file(&tmp);
        return Err(io("could not replace", e));
    }
    Ok(bytes.len() as u64)
}

fn temp_beside(p: &Path) -> PathBuf {
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    p.with_file_name(format!(".{name}.{}.part", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let d = std::env::temp_dir().join(format!(
            "simpa-app-export-{tag}-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn files(d: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(d)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    #[test]
    fn a_good_path_is_written_whole_and_nothing_else_is_left() {
        let d = dir("ok");
        let p = d.join("Box - run 1 - parameters.csv");
        let body = b"receiver,value\r\nR1,60.04\r\n";
        assert_eq!(
            write("csv", p.to_str().unwrap(), body).unwrap(),
            body.len() as u64
        );
        assert_eq!(std::fs::read(&p).unwrap(), body);
        // Again over it: replaced, still one file.
        write("csv", p.to_str().unwrap(), b"a\r\n").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"a\r\n");
        assert_eq!(files(&d), vec!["Box - run 1 - parameters.csv".to_string()]);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_wrong_extension_a_relative_path_or_a_missing_folder_writes_nothing() {
        let d = dir("no");
        let exe = d.join("view.exe");
        let e = write("png", exe.to_str().unwrap(), &PNG_SIGNATURE).unwrap_err();
        assert_eq!(e.code, "EXPORT_PATH");
        assert!(e.message.contains("does not end in .png"), "{}", e.message);
        assert_eq!(
            write("csv", "relative.csv", b"a").unwrap_err().code,
            "EXPORT_PATH"
        );
        let missing = d.join("nowhere").join("x.json");
        assert_eq!(
            write("json", missing.to_str().unwrap(), b"{}")
                .unwrap_err()
                .code,
            "EXPORT_PATH"
        );
        assert_eq!(
            write("exe", exe.to_str().unwrap(), b"a").unwrap_err().code,
            "EXPORT_KIND"
        );
        // A .csv that names a folder.
        std::fs::create_dir_all(d.join("folder.csv")).unwrap();
        assert_eq!(
            write("csv", d.join("folder.csv").to_str().unwrap(), b"a")
                .unwrap_err()
                .code,
            "EXPORT_PATH"
        );
        assert_eq!(files(&d), vec!["folder.csv".to_string()]);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn bytes_that_are_not_the_kind_are_refused() {
        let d = dir("content");
        let png = d.join("v.png");
        assert_eq!(
            write("png", png.to_str().unwrap(), b"not a png")
                .unwrap_err()
                .code,
            "EXPORT_CONTENT"
        );
        assert_eq!(
            write("json", d.join("p.json").to_str().unwrap(), b"{\"a\":")
                .unwrap_err()
                .code,
            "EXPORT_CONTENT"
        );
        assert_eq!(
            write(
                "csv",
                d.join("p.csv").to_str().unwrap(),
                &[0xff, 0xfe, 0x00]
            )
            .unwrap_err()
            .code,
            "EXPORT_CONTENT"
        );
        assert!(files(&d).is_empty());
        let mut ok = PNG_SIGNATURE.to_vec();
        ok.extend_from_slice(b"rest");
        assert_eq!(write("png", png.to_str().unwrap(), &ok).unwrap(), 12);
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn the_path_header_is_percent_decoded_as_utf8() {
        assert_eq!(
            percent_decode("C%3A%5Ctmp%5Cr%C3%A9sultats.csv").unwrap(),
            "C:\\tmp\\résultats.csv"
        );
        assert_eq!(percent_decode("plain").unwrap(), "plain");
        assert_eq!(percent_decode("bad%zz").unwrap_err().code, "EXPORT_PATH");
        assert_eq!(percent_decode("%ff").unwrap_err().code, "EXPORT_PATH");
    }
}
