//! E1 of M8a (`docs/investigations/2026-09-29-m8a/SPEC.md`, section 5.2): the solver executables
//! the bed runs must be the verified build, `solvers/manifest.json`, checked on every run
//! (decision row 17, call 5).
//!
//! [`code_sha256`] is `solvers/pe-fingerprint.ps1`'s `Get-CodeSha256` in Rust: the sha256 of the
//! PE32+ file with `IMAGE_FILE_HEADER.TimeDateStamp` and every `IMAGE_DEBUG_DIRECTORY`
//! `TimeDateStamp` set to zero, the headers parsed, never read at offsets taken from one file. A
//! file of another shape is refused, as the script refuses it: not PE32+, a `CheckSum` set, or a
//! debug entry of a type other than POGO (13), which the measured builds never have.

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use sha2::{Digest, Sha256};

/// Why a file has no code sha256.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeError(pub String);

impl fmt::Display for PeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PeError {}

fn u16_at(d: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([d[at], d[at + 1]])
}

fn u32_at(d: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([d[at], d[at + 1], d[at + 2], d[at + 3]])
}

/// One section header: its virtual address, raw size and raw pointer, and its name.
struct Section {
    name: String,
    va: u32,
    rsize: u32,
    rptr: u32,
}

struct Headers {
    coff: usize,
    opt: usize,
    opt_size: usize,
    sections: Vec<Section>,
}

fn read_headers(d: &[u8], what: &str) -> Result<Headers, PeError> {
    let need = |end: usize, part: &str| -> Result<(), PeError> {
        if end > d.len() {
            Err(PeError(format!(
                "{what}: not a PE file of the measured shape: {part} runs past the end of the \
                 file ({} bytes)",
                d.len()
            )))
        } else {
            Ok(())
        }
    };
    need(0x40, "the DOS header")?;
    if d[0] != 0x4D || d[1] != 0x5A {
        return Err(PeError(format!("{what}: not a PE file: no MZ signature")));
    }
    let pe = u32_at(d, 0x3C) as usize;
    need(pe + 24, "the PE signature and COFF header")?;
    if u32_at(d, pe) != 0x4550 {
        return Err(PeError(format!(
            "{what}: not a PE file: no PE signature at 0x{pe:X}"
        )));
    }
    let coff = pe + 4;
    let nsec = usize::from(u16_at(d, coff + 2));
    let opt_size = usize::from(u16_at(d, coff + 16));
    let opt = coff + 20;
    need(opt + opt_size, "the optional header")?;
    if opt_size < 112 {
        return Err(PeError(format!(
            "{what}: not a PE file of the measured shape: optional header of {opt_size} bytes"
        )));
    }
    let magic = u16_at(d, opt);
    if magic != 0x20B {
        return Err(PeError(format!(
            "{what}: not PE32+ (optional header magic 0x{magic:X}): the code sha256 is defined \
             for the x64 solvers only"
        )));
    }
    let table = opt + opt_size;
    need(table + 40 * nsec, "the section table")?;
    let sections = (0..nsec)
        .map(|i| {
            let s = table + 40 * i;
            Section {
                name: String::from_utf8_lossy(&d[s..s + 8])
                    .trim_end_matches('\0')
                    .to_string(),
                va: u32_at(d, s + 12),
                rsize: u32_at(d, s + 16),
                rptr: u32_at(d, s + 20),
            }
        })
        .collect();
    Ok(Headers {
        coff,
        opt,
        opt_size,
        sections,
    })
}

/// The offsets of the link-time fields (4 bytes each), as `Get-PeLinkTimeFields` finds them.
pub fn link_time_fields(d: &[u8], what: &str) -> Result<Vec<usize>, PeError> {
    let h = read_headers(d, what)?;
    let opt = h.opt;
    let sum = u32_at(d, opt + 64);
    if sum != 0 {
        return Err(PeError(format!(
            "{what}: its CheckSum is set (0x{sum:08X}), which the measured builds never have: it \
             covers the link time, so the code sha256 is not defined for this link"
        )));
    }
    let mut fields = vec![h.coff + 4];
    let ndirs = u32_at(d, opt + 108);
    if ndirs < 7 || h.opt_size < 112 + 8 * 7 {
        return Ok(fields);
    }
    let dbg_rva = u32_at(d, opt + 112 + 48);
    let dbg_size = u32_at(d, opt + 112 + 52);
    if dbg_size == 0 {
        return Ok(fields);
    }
    if !dbg_size.is_multiple_of(28) {
        return Err(PeError(format!(
            "{what}: its debug directory is {dbg_size} bytes, not a whole number of 28-byte \
             entries"
        )));
    }
    let Some(sec) = h.sections.iter().find(|s| {
        dbg_rva >= s.va
            && u64::from(dbg_rva) + u64::from(dbg_size) <= u64::from(s.va) + u64::from(s.rsize)
    }) else {
        return Err(PeError(format!(
            "{what}: its debug directory (RVA 0x{dbg_rva:X}, {dbg_size} bytes) lies in no \
             section's file data"
        )));
    };
    let off = sec.rptr as usize + (dbg_rva - sec.va) as usize;
    if off + dbg_size as usize > d.len() {
        return Err(PeError(format!(
            "{what}: not a PE file of the measured shape: the debug directory runs past the end \
             of the file ({} bytes)",
            d.len()
        )));
    }
    for k in 0..(dbg_size / 28) as usize {
        let e = off + 28 * k;
        let kind = u32_at(d, e + 12);
        if kind != 13 {
            return Err(PeError(format!(
                "{what}: debug entry {k} is of type {kind}, which the measured builds do not have \
                 (they have POGO only): the code sha256 is not defined for this link"
            )));
        }
        fields.push(e + 4);
    }
    Ok(fields)
}

/// Lower-case hex of `bytes`' sha256.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// The code sha256 of the PE32+ image `bytes` (`Get-CodeSha256`).
pub fn code_sha256_of(bytes: &[u8], what: &str) -> Result<String, PeError> {
    let mut d = bytes.to_vec();
    for at in link_time_fields(&d, what)? {
        d[at..at + 4].fill(0);
    }
    Ok(sha256_hex(&d))
}

/// A file's code sha256 and raw sha256.
pub fn file_hashes(path: &Path) -> Result<(String, String), PeError> {
    let bytes = std::fs::read(path).map_err(|e| PeError(format!("{}: {e}", path.display())))?;
    let what = path.display().to_string();
    Ok((code_sha256_of(&bytes, &what)?, sha256_hex(&bytes)))
}

/// The copy of a PE file with the middle byte of its `.text` section inverted: the say-NO input
/// of E1 (`Copy-PeFlippedText`), for tests.
pub fn flip_text_byte(bytes: &[u8]) -> Result<Vec<u8>, PeError> {
    let h = read_headers(bytes, "the file")?;
    let text: Vec<&Section> = h.sections.iter().filter(|s| s.name == ".text").collect();
    let [text] = text.as_slice() else {
        return Err(PeError("no single .text section".into()));
    };
    if text.rsize == 0 {
        return Err(PeError("the .text section has no file data".into()));
    }
    let mut d = bytes.to_vec();
    let i = text.rptr as usize + text.rsize as usize / 2;
    d[i] ^= 0xFF;
    Ok(d)
}

/// The executables the verified build lists, by file name: their code sha256 and raw sha256.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SolverManifest {
    pub code_sha256: BTreeMap<String, String>,
    pub sha256: BTreeMap<String, String>,
}

impl SolverManifest {
    /// Reads `solvers/manifest.json`'s text.
    pub fn parse(text: &str) -> Result<SolverManifest, String> {
        let v = crate::schema::parse_json(text).map_err(|e| e.to_string())?;
        let map = |key: &str| -> Result<BTreeMap<String, String>, String> {
            v.get(key)
                .and_then(|m| m.as_object())
                .ok_or(format!("the manifest has no object '{key}'"))?
                .iter()
                .map(|(k, h)| {
                    h.as_str()
                        .map(|h| (k.clone(), h.to_string()))
                        .ok_or(format!("{key}.{k} is not a string"))
                })
                .collect()
        };
        Ok(SolverManifest {
            code_sha256: map("code_sha256")?,
            sha256: map("sha256")?,
        })
    }
}

/// One executable checked against the manifest.
#[derive(
    Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct SolverCheck {
    pub name: String,
    pub path: String,
    /// The file's code sha256 (link times zeroed), or why it has none.
    pub code_sha256: Option<String>,
    pub raw_sha256: Option<String>,
    pub manifest_code_sha256: Option<String>,
    pub matches: bool,
    pub detail: Option<String>,
}

/// Each of `exes` (`name`, path) against `manifest`'s code sha256.
pub fn check_solvers(exes: &[(&str, &Path)], manifest: &SolverManifest) -> Vec<SolverCheck> {
    exes.iter()
        .map(|(name, path)| {
            let want = manifest.code_sha256.get(*name).cloned();
            match file_hashes(path) {
                Ok((code, raw)) => {
                    let matches = want.as_deref() == Some(code.as_str());
                    SolverCheck {
                        name: name.to_string(),
                        path: path.display().to_string(),
                        detail: (!matches).then(|| match &want {
                            Some(w) => format!(
                                "code sha256 {code} is not the verified build's {w} \
                                 (solvers/manifest.json)"
                            ),
                            None => "solvers/manifest.json lists no such executable".into(),
                        }),
                        code_sha256: Some(code),
                        raw_sha256: Some(raw),
                        manifest_code_sha256: want,
                        matches,
                    }
                }
                Err(e) => SolverCheck {
                    name: name.to_string(),
                    path: path.display().to_string(),
                    code_sha256: None,
                    raw_sha256: None,
                    manifest_code_sha256: want,
                    matches: false,
                    detail: Some(e.to_string()),
                },
            }
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A minimal PE32+ image: DOS header, PE signature, COFF header, a 240-byte optional header
    /// with 16 data directories, and two sections, `.text` and `.rdata`; the debug directory, when
    /// given, holds `debug_types.len()` entries in `.rdata`.
    pub(crate) fn tiny_pe(stamp: u32, debug_types: &[u32], checksum: u32) -> Vec<u8> {
        let mut d = vec![0u8; 0x400];
        d[0] = 0x4D;
        d[1] = 0x5A;
        let pe = 0x80usize;
        d[0x3C..0x40].copy_from_slice(&(pe as u32).to_le_bytes());
        d[pe..pe + 4].copy_from_slice(&0x4550u32.to_le_bytes());
        let coff = pe + 4;
        d[coff + 2..coff + 4].copy_from_slice(&2u16.to_le_bytes());
        d[coff + 4..coff + 8].copy_from_slice(&stamp.to_le_bytes());
        d[coff + 16..coff + 18].copy_from_slice(&240u16.to_le_bytes());
        let opt = coff + 20;
        d[opt..opt + 2].copy_from_slice(&0x20Bu16.to_le_bytes());
        d[opt + 64..opt + 68].copy_from_slice(&checksum.to_le_bytes());
        d[opt + 108..opt + 112].copy_from_slice(&16u32.to_le_bytes());
        let table = opt + 240;
        // .text: VA 0x1000, raw 0x200 bytes at 0x200; .rdata: VA 0x2000, raw 0x100 at 0x300.
        for (i, (name, va, rsize, rptr)) in [
            (b".text\0\0\0", 0x1000u32, 0x100u32, 0x200u32),
            (b".rdata\0\0", 0x2000, 0x100, 0x300),
        ]
        .into_iter()
        .enumerate()
        {
            let s = table + 40 * i;
            d[s..s + 8].copy_from_slice(name);
            d[s + 8..s + 12].copy_from_slice(&rsize.to_le_bytes());
            d[s + 12..s + 16].copy_from_slice(&va.to_le_bytes());
            d[s + 16..s + 20].copy_from_slice(&rsize.to_le_bytes());
            d[s + 20..s + 24].copy_from_slice(&rptr.to_le_bytes());
        }
        for (i, b) in d[0x200..0x300].iter_mut().enumerate() {
            *b = i as u8;
        }
        if !debug_types.is_empty() {
            let size = 28 * debug_types.len() as u32;
            d[opt + 112 + 48..opt + 112 + 52].copy_from_slice(&0x2010u32.to_le_bytes());
            d[opt + 112 + 52..opt + 112 + 56].copy_from_slice(&size.to_le_bytes());
            for (k, t) in debug_types.iter().enumerate() {
                let e = 0x310 + 28 * k;
                d[e + 4..e + 8].copy_from_slice(&stamp.to_le_bytes());
                d[e + 12..e + 16].copy_from_slice(&t.to_le_bytes());
            }
        }
        d
    }

    #[test]
    fn the_code_sha256_ignores_the_link_times_and_nothing_else() {
        let a = tiny_pe(0x1111_1111, &[13], 0);
        let b = tiny_pe(0x2222_2222, &[13], 0);
        assert_ne!(sha256_hex(&a), sha256_hex(&b));
        assert_eq!(
            code_sha256_of(&a, "a").unwrap(),
            code_sha256_of(&b, "b").unwrap()
        );
        // The fields found: the COFF header's stamp and the one debug entry's.
        assert_eq!(link_time_fields(&a, "a").unwrap(), vec![0x88, 0x314]);
        // Says no: one code byte flipped is another code sha256.
        let flipped = flip_text_byte(&a).unwrap();
        assert_eq!(flipped.len(), a.len());
        assert_eq!(a.iter().zip(&flipped).filter(|(x, y)| x != y).count(), 1);
        assert_ne!(
            code_sha256_of(&flipped, "f").unwrap(),
            code_sha256_of(&a, "a").unwrap()
        );
    }

    #[test]
    fn a_file_of_another_shape_has_no_code_sha256() {
        let e = |d: &[u8]| code_sha256_of(d, "x").unwrap_err().0;
        assert!(e(&tiny_pe(1, &[13], 0x1234)).contains("CheckSum"));
        assert!(e(&tiny_pe(1, &[2], 0)).contains("type 2"));
        assert!(e(b"MZ").contains("runs past the end"));
        let mut not_pe = tiny_pe(1, &[], 0);
        not_pe[0] = b'X';
        assert!(e(&not_pe).contains("no MZ"));
        let mut pe32 = tiny_pe(1, &[], 0);
        pe32[0x98..0x9A].copy_from_slice(&0x10Bu16.to_le_bytes());
        assert!(e(&pe32).contains("not PE32+"));
    }

    #[test]
    fn solvers_are_checked_against_the_manifest() {
        let dir = std::env::temp_dir().join(format!("simpa-bed-pe-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let good = tiny_pe(5, &[13], 0);
        let path = dir.join("spps.exe");
        std::fs::write(&path, &good).unwrap();
        let code = code_sha256_of(&good, "g").unwrap();
        let manifest = SolverManifest {
            code_sha256: [("spps.exe".to_string(), code.clone())].into(),
            sha256: Default::default(),
        };
        let ok = check_solvers(&[("spps.exe", &path)], &manifest);
        assert!(ok[0].matches, "{ok:?}");
        assert_eq!(
            ok[0].raw_sha256.as_deref(),
            Some(sha256_hex(&good).as_str())
        );
        // Says no: the flipped copy, and an executable the manifest does not list.
        std::fs::write(&path, flip_text_byte(&good).unwrap()).unwrap();
        let bad = check_solvers(&[("spps.exe", &path), ("tetgen.exe", &path)], &manifest);
        assert!(!bad[0].matches && !bad[1].matches, "{bad:?}");
        assert!(bad[0].detail.as_deref().unwrap().contains(&code));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_embedded_manifest_lists_the_four_executables() {
        let m = SolverManifest::parse(crate::bed::SOLVER_MANIFEST).unwrap();
        for name in crate::bed::SOLVER_EXES {
            let h = &m.code_sha256[name];
            assert_eq!(h.len(), 64, "{name}");
            assert!(m.sha256.contains_key(name), "{name}");
        }
    }
}
