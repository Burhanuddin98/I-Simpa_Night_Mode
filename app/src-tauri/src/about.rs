//! Parity A23: what Help › About says this build is. The version is Cargo's, the commit and the
//! profile the build script's (`build_identity.rs`), and the solvers' build the verified manifest
//! compiled into the core (`simpa_core::bed::SOLVER_MANIFEST`, `solvers/manifest.json`), so none of
//! it is typed by hand where it could drift from the build. The licences are words, kept in the UI
//! (`chrome/aboutModel.ts`), with the full texts opened through Help (`help::PAGES`).

use schemars::JsonSchema;
use serde::Serialize;

use crate::guard::{CmdError, CmdResult};
use crate::webview2::{self, WebviewInfo};

/// The solvers this build was compiled to run, as the manifest records them.
#[derive(Clone, Debug, Serialize, JsonSchema, PartialEq, Eq)]
pub struct AboutSolvers {
    /// Upstream I-Simpa's commit the solvers are built from.
    pub upstream_commit: String,
    /// This repository's patches applied to it, by file name, in order.
    pub patches: Vec<String>,
    /// TetGen's version.
    pub tetgen_version: String,
    /// spps-gpu's source commit in this repository.
    pub spps_gpu_commit: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AboutInfo {
    pub app_version: &'static str,
    /// The commit this build was made from (12 hex digits), `… with uncommitted changes` when the
    /// tree differed from it as the build script last ran, or `unknown` with no git.
    pub commit: &'static str,
    /// `release` or `debug`.
    pub profile: &'static str,
    pub tauri_version: &'static str,
    pub webview: WebviewInfo,
    pub solvers: AboutSolvers,
}

/// The manifest could not be read: a build defect, said as one.
pub const ABOUT_MANIFEST: &str = "ABOUT_MANIFEST";

/// Reads [`AboutSolvers`] from a manifest's text (`solvers/manifest.json`'s shape).
pub fn solver_build(manifest: &str) -> CmdResult<AboutSolvers> {
    let bad = |what: &str| {
        CmdError::new(
            ABOUT_MANIFEST,
            format!("the solver manifest compiled into this build has no {what}"),
        )
    };
    let v: serde_json::Value = serde_json::from_str(manifest.trim_start_matches('\u{feff}'))
        .map_err(|e| {
            CmdError::new(
                ABOUT_MANIFEST,
                format!("the solver manifest does not read: {e}"),
            )
        })?;
    let text = |p: &str| v.pointer(p).and_then(|x| x.as_str()).map(str::to_string);
    let mut patches: Vec<String> = v
        .get("patches")
        .and_then(|p| p.as_object())
        .ok_or_else(|| bad("patches"))?
        .keys()
        .cloned()
        .collect();
    patches.sort();
    Ok(AboutSolvers {
        upstream_commit: text("/upstream_commit").ok_or_else(|| bad("upstream_commit"))?,
        patches,
        tetgen_version: text("/tetgen/version").ok_or_else(|| bad("tetgen.version"))?,
        spps_gpu_commit: text("/spps_gpu/source_commit")
            .ok_or_else(|| bad("spps_gpu.source_commit"))?,
    })
}

pub fn info() -> CmdResult<AboutInfo> {
    Ok(AboutInfo {
        app_version: env!("CARGO_PKG_VERSION"),
        commit: env!("SIMPA_BUILD_COMMIT"),
        profile: env!("SIMPA_BUILD_PROFILE"),
        tauri_version: tauri::VERSION,
        webview: webview2::info(),
        solvers: solver_build(simpa_core::bed::SOLVER_MANIFEST)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_compiled_manifest_names_the_solver_build() {
        let s = solver_build(simpa_core::bed::SOLVER_MANIFEST).unwrap();
        assert_eq!(s.upstream_commit.len(), 40, "{s:?}");
        assert!(s.upstream_commit.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(!s.patches.is_empty());
        assert!(s.patches.iter().all(|p| p.ends_with(".patch")), "{s:?}");
        assert!(s.patches.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(s.tetgen_version, "1.5.0");
        assert_eq!(s.spps_gpu_commit.len(), 40);
    }

    #[test]
    fn the_build_says_its_commit_and_profile() {
        let a = info().unwrap();
        assert_eq!(a.app_version, env!("CARGO_PKG_VERSION"));
        let sha = a.commit.split(' ').next().unwrap();
        assert!(
            a.commit == "unknown"
                || (sha.len() == 12 && sha.chars().all(|c| c.is_ascii_hexdigit())),
            "{}",
            a.commit
        );
        assert!(["debug", "release"].contains(&a.profile), "{}", a.profile);
    }

    #[test]
    fn a_manifest_missing_a_field_is_refused_by_name() {
        let err = solver_build(r#"{"upstream_commit": "x", "patches": {}}"#).unwrap_err();
        assert_eq!(err.code, ABOUT_MANIFEST);
        assert!(err.message.contains("tetgen.version"), "{}", err.message);
        assert_eq!(solver_build("not json").unwrap_err().code, ABOUT_MANIFEST);
    }
}
