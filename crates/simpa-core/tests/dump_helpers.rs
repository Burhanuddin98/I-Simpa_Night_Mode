//! The canonical-dump helpers must spell values exactly as the C++ oracle does
//! (oracle/dump_selftest.cpp prints the same lines).
use simpa_core::formats::{f32_hex, f64_hex, str_token};

pub fn selftest_dump() -> String {
    format!(
        "selftest 1\nf32 {} {} {} {}\nf64 {} {}\nstr {} {}\n",
        f32_hex(1.0),
        f32_hex(-0.0),
        f32_hex(0.1),
        f32_hex(3.402_823_5e38),
        f64_hex(1.0),
        f64_hex(0.1),
        str_token(b"Plane Receiver\\\xe9"),
        str_token(b""),
    )
}

#[test]
fn spellings_are_fixed() {
    assert_eq!(
        selftest_dump(),
        "selftest 1\nf32 3f800000 80000000 3dcccccd 7f7fffff\nf64 3ff0000000000000 3fb999999999999a\nstr Plane\\x20Receiver\\x5c\\xe9 \\x\n"
    );
}

#[test]
fn oracle_agrees_when_built() {
    let exe =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/oracle/all/oracle.exe");
    if !exe.exists() {
        eprintln!("oracle not built; skipping cross-check");
        return;
    }
    let out = std::process::Command::new(exe)
        .args(["dump", "selftest", "-"])
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n"),
        selftest_dump()
    );
}
