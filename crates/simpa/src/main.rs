use std::path::Path;
use std::process::ExitCode;

use simpa_core::{config_xml, formats, schema, validate};

const USAGE: &str = "usage:
  simpa --version
  simpa dump <cbin|mbin|poly|tetgen|gabe|csbin|pbin> <file>   canonical dump of a solver file
  simpa gen <cbin|mbin|poly> <seed> <out>                     write a generated model, print its dump
                                                              (poly: as upstream's reader sees it)
  simpa validate <project.simpa> [--json] [--mesh-hash <hex>] check a project; exit 2 on any error
  simpa export-config <project.simpa> <run-dir> [--solver spps|tcr] [--variant <name>]
                                                              write config.xml and mesh.cbin for a run";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();
    match argv.as_slice() {
        [] | ["--version"] | ["version"] => {
            println!(
                "simpa {} (solvers from upstream {})",
                simpa_core::VERSION,
                &simpa_core::SOLVER_COMMIT[..7]
            );
            ExitCode::SUCCESS
        }
        ["dump", format, file] => dump(format, Path::new(file)),
        ["gen", format, seed, out] => match seed.parse::<u64>() {
            Ok(seed) => generate(format, seed, Path::new(out)),
            Err(_) => fail(&format!("seed must be an unsigned integer, got '{seed}'")),
        },
        ["validate", rest @ ..] => validate_cmd(rest),
        ["export-config", rest @ ..] => export_config(rest),
        [command, ..] => fail(&format!("unknown command '{command}'\n{USAGE}")),
    }
}

fn fail(msg: &str) -> ExitCode {
    eprintln!("simpa: {msg}");
    ExitCode::from(2)
}

fn dump(format: &str, file: &Path) -> ExitCode {
    let text = match format {
        "cbin" => formats::cbin::dump_file(file),
        "mbin" => formats::mbin::dump_file(file),
        "poly" => formats::poly::dump_file(file),
        "tetgen" => formats::tetgen::dump_file(file),
        "gabe" => formats::gabe::dump_file(file),
        "csbin" => formats::csbin::dump_file(file),
        "pbin" => formats::pbin::dump_file(file),
        other => return fail(&format!("unknown format '{other}'")),
    };
    print!("{text}");
    if text.starts_with("error") {
        ExitCode::from(1)
    } else {
        ExitCode::SUCCESS
    }
}

/// `validate <project> [--json] [--mesh-hash <hex>]`: exit 0 when clean or warnings only, 2 when
/// any error. The file is read with `validate::read_project`, so a structurally broken project is
/// reported under its contract code rather than refused at load.
fn validate_cmd(args: &[&str]) -> ExitCode {
    let mut json = false;
    let mut mesh_hash = None;
    let mut file = None;
    let mut it = args.iter();
    while let Some(&a) = it.next() {
        match a {
            "--json" => json = true,
            "--mesh-hash" => match it.next() {
                Some(h) => mesh_hash = Some(h.to_string()),
                None => return fail("--mesh-hash needs a value"),
            },
            _ if file.is_none() => file = Some(Path::new(a)),
            _ => return fail(&format!("unexpected argument '{a}'\n{USAGE}")),
        }
    }
    let Some(file) = file else {
        return fail(&format!("validate needs a project file\n{USAGE}"));
    };
    let issues = match validate::read_project(file) {
        Ok(project) => {
            let mut ctx = validate::Context::for_project_file(file);
            ctx.mesh_input_hash = mesh_hash;
            validate::validate_with(&project, &ctx)
        }
        Err(e) => {
            let msg = e.to_string();
            if json {
                let issue = serde_json::json!([{
                    "code": e.code(), "severity": "error", "message": msg, "path": ""
                }]);
                println!("{issue}");
            } else {
                println!("error {} at : {msg}", e.code());
            }
            return ExitCode::from(2);
        }
    };
    if json {
        println!(
            "{}",
            serde_json::to_string(&issues).expect("issues serialise")
        );
    } else {
        for issue in &issues {
            println!("{issue}");
        }
    }
    if validate::has_errors(&issues) {
        ExitCode::from(2)
    } else {
        ExitCode::SUCCESS
    }
}

/// `export-config <project> <run-dir> [--solver spps|tcr] [--variant <name>]`: writes the run
/// folder's `config.xml` and scene mesh. The tetrahedral mesh comes from the mesher (M5).
fn export_config(args: &[&str]) -> ExitCode {
    let mut solver = config_xml::SolverKind::Spps;
    let mut variant = None;
    let mut positional = Vec::new();
    let mut it = args.iter();
    while let Some(&a) = it.next() {
        match a {
            "--solver" => match it.next().copied() {
                Some("spps") => solver = config_xml::SolverKind::Spps,
                Some("tcr") => solver = config_xml::SolverKind::Tcr,
                other => return fail(&format!("--solver must be spps or tcr, got {other:?}")),
            },
            "--variant" => match it.next() {
                Some(v) => variant = Some(*v),
                None => return fail("--variant needs a name"),
            },
            _ => positional.push(a),
        }
    }
    let [project_file, run_dir] = positional.as_slice() else {
        return fail(&format!("export-config needs <project> <run-dir>\n{USAGE}"));
    };
    let project = match schema::load(Path::new(project_file)) {
        Ok(p) => p,
        Err(e) => return fail(&format!("{project_file}: {e}")),
    };
    let run_dir = Path::new(run_dir);
    if let Err(e) = std::fs::create_dir_all(run_dir) {
        return fail(&format!("{}: {e}", run_dir.display()));
    }
    let run_dir = match std::path::absolute(run_dir) {
        Ok(p) => p,
        Err(e) => return fail(&format!("{}: {e}", run_dir.display())),
    };
    let config = match config_xml::write_file(&project, solver, variant, &run_dir) {
        Ok(p) => p,
        Err(e) => return fail(&format!("config.xml: {e}")),
    };
    let mesh = match config_xml::scene_mesh(&project) {
        Ok(m) => m,
        Err(e) => return fail(&format!("scene mesh: {e}")),
    };
    let mesh_path = run_dir.join(config_xml::names::SCENE_MESH);
    if let Err(e) = formats::cbin::write_file(&mesh, &mesh_path) {
        return fail(&format!("{}: {e}", mesh_path.display()));
    }
    println!("{}", config.display());
    println!("{}", mesh_path.display());
    ExitCode::SUCCESS
}

fn generate(format: &str, seed: u64, out: &Path) -> ExitCode {
    let written = match format {
        "cbin" => {
            let m = formats::cbin::generate(seed);
            formats::cbin::write_file(&m, out).map(|_| formats::cbin::dump(&m))
        }
        "mbin" => {
            let m = formats::mbin::generate(seed);
            formats::mbin::write_file(&m, out).map(|_| formats::mbin::dump(&m))
        }
        "poly" => {
            // Printed as upstream's ImportPOLY reads it back: its bug at poly.cpp:406-424 gives
            // every user facet after the first the first one's marker. See formats::poly docs.
            let m = formats::poly::generate(seed);
            formats::poly::write_file(&m, out)
                .map(|_| formats::poly::dump(&formats::poly::upstream_import_view(&m)))
        }
        other => return fail(&format!("no writer for format '{other}'")),
    };
    match written {
        Ok(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(e) => fail(&format!("writing {}: {e}", out.display())),
    }
}
