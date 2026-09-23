//! [`super::verify_dir`]: what a mesh folder shows beyond its `.mbin`.
//!
//! File names are matched ignoring ASCII case, as Windows opens them.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::{DirReport, VolumeIds, verify_mesh};
use crate::formats::{self, FormatError, cbin, mbin, tetgen};

/// Suffixes of the files TetGen writes for input `<base>.poly`; any one of them names the base.
const TETGEN_SUFFIXES: [&str; 6] = [
    ".1.node",
    ".1.ele",
    ".1.face",
    ".1.neigh",
    "_skipped.face",
    "_skipped.node",
];

/// The folder's mesh manifest (`docs/m5-m6-design.md`, "Layout").
const MANIFEST: &str = "mesh.json";
/// The manifest field holding the `.mbin`'s sha256 as hex; the only field read here.
const MANIFEST_MBIN_SHA256: &str = "mbin_sha256";

/// The folder's regular files, keyed by lowercased name.
struct Listing {
    dir: PathBuf,
    files: BTreeMap<String, (String, PathBuf)>,
}

impl Listing {
    fn read(dir: &Path) -> Result<Self, FormatError> {
        let entries = std::fs::read_dir(dir).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => FormatError::NotFound(dir.to_path_buf()),
            _ => FormatError::Io(e),
        })?;
        let mut files = BTreeMap::new();
        for entry in entries {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            files.insert(name.to_ascii_lowercase(), (name, entry.path()));
        }
        Ok(Listing {
            dir: dir.to_path_buf(),
            files,
        })
    }

    fn get(&self, lower_name: &str) -> Option<&Path> {
        self.files.get(lower_name).map(|(_, p)| p.as_path())
    }

    /// The TetGen basename, lowercased and as written, if any file carries one.
    fn tetgen_base(&self) -> Result<Option<(String, String)>, FormatError> {
        // Lowercasing keeps byte lengths, so the base's length cuts the written name too.
        let mut bases: BTreeMap<String, String> = BTreeMap::new();
        for (lower, (name, _)) in &self.files {
            for suffix in TETGEN_SUFFIXES {
                if let Some(base) = lower.strip_suffix(suffix).filter(|b| !b.is_empty()) {
                    bases
                        .entry(base.to_string())
                        .or_insert_with(|| name[..base.len()].to_string());
                }
            }
        }
        if bases.len() > 1 {
            let names: Vec<&str> = bases.values().map(String::as_str).collect();
            return Err(FormatError::Invalid(format!(
                "{}: TetGen output under several basenames: {}",
                self.dir.display(),
                names.join(", ")
            )));
        }
        Ok(bases.into_iter().next())
    }

    /// `preferred`, else the only file with extension `ext` (both lowercase), else none.
    fn pick(&self, preferred: &str, ext: &str) -> Result<Option<PathBuf>, FormatError> {
        if let Some(p) = self.get(preferred) {
            return Ok(Some(p.to_path_buf()));
        }
        let found: Vec<&(String, PathBuf)> = self
            .files
            .iter()
            .filter(|(lower, _)| lower.ends_with(ext))
            .map(|(_, v)| v)
            .collect();
        match found.as_slice() {
            [] => Ok(None),
            [(_, p)] => Ok(Some(p.clone())),
            many => Err(FormatError::Invalid(format!(
                "{}: {} {ext} files and no {preferred}: {}",
                self.dir.display(),
                many.len(),
                many.iter()
                    .map(|(n, _)| n.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))),
        }
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(64);
    for b in Sha256::digest(bytes) {
        let _ = write!(s, "{b:02x}");
    }
    s
}

pub(super) fn verify(dir: &Path, ids: &VolumeIds) -> Result<DirReport, FormatError> {
    let listing = Listing::read(dir)?;
    let mut report = DirReport {
        dir: dir.to_path_buf(),
        ..DirReport::default()
    };
    let mut codes: Vec<&str> = Vec::new();

    // TetGen's own output.
    let base = listing.tetgen_base()?;
    if let Some((lower, name)) = &base {
        report.basename = Some(name.clone());
        let has = |suffix: &str| listing.get(&format!("{lower}{suffix}")).is_some();
        if let Some(path) = listing.get(&format!("{lower}_skipped.face")) {
            // Rows are `id a b c marker`, the marker being the facet's `shellmark`
            // (tetgen.cxx:20462, 21338, written at 36038-36085).
            let skipped = tetgen::read_face(&formats::read_file(path)?)?;
            report.skipped_facets = skipped.faces.len();
            report.skipped_markers = skipped.markers.unwrap_or_default();
            codes.push("tetgen_skipped_facets");
        }
        if !has(".1.neigh") {
            codes.push("neigh_missing");
        }
        if !(has(".1.node") && has(".1.ele") && has(".1.face")) {
            codes.push("tetgen_output_missing");
        }
    }

    // The `.mbin` and the `.cbin` its markers index.
    let mbin_path = listing.pick("tetramesh.mbin", ".mbin")?;
    let cbin_path = listing.pick("mesh.cbin", ".cbin")?;
    if let Some(path) = &mbin_path {
        let bytes = formats::read_file(path)?;
        report.mbin_sha256 = Some(sha256_hex(&bytes));
        let mesh = mbin::read(&bytes)?;
        match &cbin_path {
            Some(cpath) => {
                let scene = cbin::read_file(cpath)?;
                report.mesh = Some(verify_mesh(&mesh, &scene, ids));
            }
            None => codes.push("cbin_missing"),
        }
    }
    report.mbin_file = mbin_path;
    report.cbin_file = cbin_path;

    // The manifest's record of the `.mbin`, when it keeps one.
    if let Some(path) = listing.get(MANIFEST) {
        let bytes = formats::read_file(path)?;
        // A UTF-8 BOM is tolerated: this repo's `solvers/manifest.json` carries one.
        let json = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(&bytes);
        let manifest: serde_json::Value = serde_json::from_slice(json)
            .map_err(|e| FormatError::Invalid(format!("{}: not JSON: {e}", path.display())))?;
        let recorded = manifest.get(MANIFEST_MBIN_SHA256);
        let matches = match recorded {
            None | Some(serde_json::Value::Null) => true,
            Some(serde_json::Value::String(hex)) => report
                .mbin_sha256
                .as_deref()
                .is_some_and(|h| h.eq_ignore_ascii_case(hex)),
            Some(_) => false,
        };
        if !matches {
            codes.push("manifest_mismatch");
        }
    }

    if base.is_none() && report.mbin_file.is_none() {
        codes.push("nothing_to_verify");
    }

    report.codes = codes.into_iter().map(str::to_string).collect();
    if let Some(mesh) = &report.mesh {
        report.codes.extend(mesh.codes.iter().cloned());
    }
    Ok(report)
}
