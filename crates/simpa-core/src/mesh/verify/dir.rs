//! [`super::verify_dir`]: what a mesh folder shows beyond its `.mbin`.
//!
//! File names are matched ignoring ASCII case, as Windows opens them.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

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

/// True when every record `manifest` keeps of the `.mbin` agrees with the folder, whose `.mbin`
/// hashes to `mbin_sha256` (`None`: the folder has no `.mbin`). Two records are read, and nothing
/// else in the manifest:
/// - `files.mbin`, the mesher's (`docs/formats/mesh-manifest.md`, "Layout"): the hex sha256, or
///   null when it wrote no `.mbin`, so null beside a `.mbin` is a partial or foreign file;
/// - a top-level `mbin_sha256`, hex, with null meaning "not recorded".
///
/// An absent record is not checked. A record of the wrong JSON type does not agree.
fn manifest_agrees(manifest: &serde_json::Value, mbin_sha256: Option<&str>) -> bool {
    use serde_json::Value;
    let is_the_mbin = |hex: &str| mbin_sha256.is_some_and(|h| h.eq_ignore_ascii_case(hex));
    let files = match manifest.get("files") {
        None | Some(Value::Null) => true,
        Some(Value::Object(files)) => match files.get("mbin") {
            None => true,
            Some(Value::Null) => mbin_sha256.is_none(),
            Some(Value::String(hex)) => is_the_mbin(hex),
            Some(_) => false,
        },
        Some(_) => false,
    };
    let top_level = match manifest.get("mbin_sha256") {
        None | Some(Value::Null) => true,
        Some(Value::String(hex)) => is_the_mbin(hex),
        Some(_) => false,
    };
    files && top_level
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
    // Only a `.mbin` needs a `.cbin`, so several of them are ambiguous only then.
    let cbin_path = match mbin_path {
        Some(_) => listing.pick("mesh.cbin", ".cbin")?,
        None => None,
    };
    if let Some(path) = &mbin_path {
        let bytes = formats::read_file(path)?;
        report.mbin_sha256 = Some(crate::run::manifest::sha256_bytes(&bytes));
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
        if !manifest_agrees(&manifest, report.mbin_sha256.as_deref()) {
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
