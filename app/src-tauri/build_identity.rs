//! Parity A23: what About says this build is (see `emit`).

/// Parity A23: the commit this build was made from and its profile, for About
/// (`SIMPA_BUILD_COMMIT`, `SIMPA_BUILD_PROFILE`). A tree with no git gives "unknown"; uncommitted
/// changes are named, since such a build is not the commit alone. Rebuilt when HEAD, the branch
/// it points to or the index moves.
pub fn emit() {
    let git = |args: &[&str]| -> Option<String> {
        let out = std::process::Command::new("git").args(args).output().ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
            .filter(|s| !s.is_empty())
    };
    let commit = match git(&["rev-parse", "--short=12", "HEAD"]) {
        Some(sha) => {
            let dirty = git(&["status", "--porcelain", "--untracked-files=no"]).is_some();
            if dirty {
                format!("{sha} with uncommitted changes")
            } else {
                sha
            }
        }
        None => "unknown".to_string(),
    };
    println!("cargo:rustc-env=SIMPA_BUILD_COMMIT={commit}");
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=SIMPA_BUILD_PROFILE={profile}");
    if let (Some(dir), Some(common)) = (
        git(&["rev-parse", "--git-dir"]),
        git(&["rev-parse", "--git-common-dir"]),
    ) {
        println!("cargo:rerun-if-changed={dir}/HEAD");
        println!("cargo:rerun-if-changed={dir}/index");
        if let Some(branch) = git(&["symbolic-ref", "-q", "HEAD"]) {
            println!("cargo:rerun-if-changed={common}/{branch}");
        }
        println!("cargo:rerun-if-changed={common}/packed-refs");
    }
}
