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
                                                              write config.xml and mesh.cbin for a run
  simpa import <mesh.ply|obj|stl> <out.simpa> --unit m|cm|mm|ft|in --up y|z
               [--weld by-format|exact|off] [--keep-groups <previous.simpa>] [--json]
  simpa import-proj <file.proj> <out.simpa> [--json]         import an upstream I-Simpa project
  simpa check <model|project.simpa> [--unit ..] [--up ..] [--json]   exit 3 when refused
  simpa repair <in> <out.simpa> [--weld-tolerance <m>] [--json]      exit 3 when refused";

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
        ["import", rest @ ..] => import_cmd(rest),
        ["import-proj", rest @ ..] => import_proj_cmd(rest),
        ["check", rest @ ..] => check_cmd(rest),
        ["repair", rest @ ..] => repair_cmd(rest),
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

/// Options shared by the geometry commands: positional arguments and `--flag value` pairs.
struct GeoArgs<'a> {
    positional: Vec<&'a str>,
    json: bool,
    options: std::collections::BTreeMap<&'a str, &'a str>,
}

fn geo_args<'a>(args: &[&'a str], flags_with_values: &[&str]) -> Result<GeoArgs<'a>, String> {
    let mut out = GeoArgs {
        positional: Vec::new(),
        json: false,
        options: Default::default(),
    };
    let mut it = args.iter();
    while let Some(&a) = it.next() {
        if a == "--json" {
            out.json = true;
        } else if let Some(name) = a.strip_prefix("--") {
            // --units is accepted for --unit.
            let name = if name == "units" { "unit" } else { name };
            if !flags_with_values.contains(&name) {
                return Err(format!("unknown option '{a}'\n{USAGE}"));
            }
            match it.next() {
                Some(&v) => {
                    out.options.insert(name, v);
                }
                None => return Err(format!("{a} needs a value")),
            }
        } else {
            out.positional.push(a);
        }
    }
    Ok(out)
}

/// A project from a `.simpa` file, or from a mesh file imported with the given options.
fn project_from(path: &Path, a: &GeoArgs) -> Result<schema::Project, String> {
    use simpa_core::geometry::import::{ImportOptions, Unit, Up, Weld};
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("simpa"))
    {
        return schema::load(path).map_err(|e| format!("{}: {e}", path.display()));
    }
    let unit = a
        .options
        .get("unit")
        .ok_or("--unit is required for a mesh file (m, cm, mm, ft, in)")?;
    let unit = Unit::from_symbol(unit).ok_or(format!("unknown unit '{unit}'"))?;
    let up = a
        .options
        .get("up")
        .ok_or("--up is required for a mesh file (y or z)")?;
    let up = Up::from_name(up).ok_or(format!("unknown up axis '{up}'"))?;
    let mut options = ImportOptions::new(unit, up);
    if let Some(w) = a.options.get("weld") {
        options.weld = match *w {
            "by-format" => Weld::ByFormat,
            "exact" => Weld::Exact,
            "off" => Weld::Off,
            other => return Err(format!("unknown --weld '{other}'")),
        };
    }
    let model = simpa_core::geometry::import::import_file(path, &options)
        .map_err(|e| format!("{}: {} ({e})", path.display(), e.code()))?;
    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(model.to_project(&name))
}

/// Faces of each surface group, in group order.
fn group_faces(p: &schema::Project) -> Vec<(schema::GroupId, String, Vec<u32>)> {
    p.surface_groups
        .iter()
        .map(|g| {
            let faces = (0..p.geometry.faces.len() as u32)
                .filter(|&i| p.geometry.faces[i as usize].group == g.id)
                .collect();
            (g.id, g.name.clone(), faces)
        })
        .collect()
}

fn project_summary(p: &schema::Project) -> serde_json::Value {
    use simpa_core::geometry::check;
    let report = check::check(&p.geometry);
    let groups = group_faces(p);
    let surface_receivers: Vec<_> = p
        .surface_receivers
        .iter()
        .filter_map(|r| match &r.shape {
            schema::SurfaceReceiverShape::Scene { groups: ids } => {
                let mut faces: Vec<u32> = groups
                    .iter()
                    .filter(|(id, _, _)| ids.contains(id))
                    .flat_map(|(_, _, f)| f.iter().copied())
                    .collect();
                faces.sort_unstable();
                Some(serde_json::json!({ "name": r.name, "faces": faces }))
            }
            schema::SurfaceReceiverShape::CuttingPlane { .. } => None,
        })
        .collect();
    serde_json::json!({
        "vertices": p.geometry.vertices.len(),
        "faces": p.geometry.faces.len(),
        "groups": groups.iter().map(|(_, n, f)| serde_json::json!({ "name": n, "faces": f })).collect::<Vec<_>>(),
        "surface_receivers": surface_receivers,
        "area_m2": report.measures.area_m2,
        "volume_m3": report.measures.signed_volume_m3,
        "sources": p.sources.iter().map(|s| s.position.to_array()).collect::<Vec<_>>(),
        "receivers": p.point_receivers.iter().map(|r| r.position.to_array()).collect::<Vec<_>>(),
    })
}

/// `import <mesh> <out.simpa> --unit .. --up .. [--weld ..] [--keep-groups prev] [--json]`
fn import_cmd(args: &[&str]) -> ExitCode {
    let a = match geo_args(args, &["unit", "up", "weld", "keep-groups", "tolerance"]) {
        Ok(a) => a,
        Err(e) => return fail(&e),
    };
    let [input, out] = a.positional.as_slice() else {
        return fail(&format!("import needs <mesh> <out.simpa>\n{USAGE}"));
    };
    let mut project = match project_from(Path::new(input), &a) {
        Ok(p) => p,
        Err(e) => return fail(&e),
    };
    if let Some(prev) = a.options.get("keep-groups") {
        use simpa_core::geometry::import;
        let previous = match schema::load(Path::new(prev)) {
            Ok(p) => p,
            Err(e) => return fail(&format!("{prev}: {e}")),
        };
        let tolerance = match a.options.get("tolerance").map(|t| t.parse::<f64>()) {
            None => import::DEFAULT_REASSIGN_TOLERANCE_M,
            Some(Ok(t)) => t,
            Some(Err(_)) => return fail("--tolerance must be a number of metres"),
        };
        let unit = import::Unit::from_symbol(a.options["unit"]).expect("checked by project_from");
        let up = import::Up::from_name(a.options["up"]).expect("checked by project_from");
        let model =
            match import::import_file(Path::new(input), &import::ImportOptions::new(unit, up)) {
                Ok(m) => m,
                Err(e) => return fail(&format!("{input}: {e}")),
            };
        match import::reassign(&previous, &model, tolerance) {
            Ok(r) => {
                eprintln!(
                    "kept groups: {} faces matched, {} unmatched",
                    r.matched_faces, r.unmatched_faces
                );
                project = r.project;
            }
            Err(e) => return fail(&format!("keep-groups: {e}")),
        }
    }
    if let Err(e) = schema::save(&project, Path::new(out)) {
        return fail(&format!("{out}: {e}"));
    }
    print_summary(&project, a.json)
}

fn print_summary(project: &schema::Project, json: bool) -> ExitCode {
    let s = project_summary(project);
    if json {
        println!("{s}");
    } else {
        println!(
            "{} vertices, {} faces, {} groups, area {:.3} m2, volume {:.3} m3",
            s["vertices"],
            s["faces"],
            project.surface_groups.len(),
            s["area_m2"].as_f64().unwrap_or(0.0),
            s["volume_m3"].as_f64().unwrap_or(0.0)
        );
    }
    ExitCode::SUCCESS
}

/// `import-proj <file.proj> <out.simpa> [--json]`
fn import_proj_cmd(args: &[&str]) -> ExitCode {
    let a = match geo_args(args, &[]) {
        Ok(a) => a,
        Err(e) => return fail(&e),
    };
    let [input, out] = a.positional.as_slice() else {
        return fail(&format!(
            "import-proj needs <file.proj> <out.simpa>\n{USAGE}"
        ));
    };
    let mut imported = match simpa_core::geometry::import::import_proj_file(Path::new(input)) {
        Ok(i) => i,
        Err(e) => return fail(&format!("{input}: {} ({e})", e.code())),
    };
    // Name the project after its file rather than leaving the importer's placeholder.
    if (imported.project.name.is_empty() || imported.project.name == "New project")
        && let Some(stem) = Path::new(input).file_stem()
    {
        imported.project.name = stem.to_string_lossy().into_owned();
    }
    if let Err(e) = schema::save(&imported.project, Path::new(out)) {
        return fail(&format!("{out}: {e}"));
    }
    print_summary(&imported.project, a.json)
}

/// `check <model|project> [--unit ..] [--up ..] [--weld ..] [--json]`: exit 0 ok, 3 refused.
fn check_cmd(args: &[&str]) -> ExitCode {
    use simpa_core::geometry::check;
    let a = match geo_args(args, &["unit", "up", "weld"]) {
        Ok(a) => a,
        Err(e) => return fail(&e),
    };
    let [input] = a.positional.as_slice() else {
        return fail(&format!("check needs one model or project\n{USAGE}"));
    };
    let project = match project_from(Path::new(input), &a) {
        Ok(p) => p,
        Err(e) => return fail(&e),
    };
    let r = check::check(&project.geometry);
    let ok = r.verdict == check::Verdict::Ok;
    if a.json {
        let out = serde_json::json!({
            "verdict": if ok { "ok" } else { "refused" },
            "reasons": r.reasons.iter().map(|x| x.code.as_str()).collect::<Vec<_>>(),
            "vertices": r.counts.vertices,
            "faces": r.counts.faces,
            "edges": r.counts.edges,
            "open_edges": r.counts.open_edges,
            "manifold_edges": r.counts.manifold_edges,
            "nonmanifold_edges": r.counts.nonmanifold_edges,
            "self_intersections": r.counts.self_intersecting_pairs,
            "intersecting_pairs": r.self_intersections,
            "signed_volume_m3": r.measures.signed_volume_m3,
            "area_m2": r.measures.area_m2,
            "report": r,
        });
        println!("{out}");
    } else {
        println!(
            "{}: {} faces, {} edges ({} open, {} non-manifold), {} self-intersecting pairs, volume {:.3} m3",
            if ok { "ok" } else { "refused" },
            r.counts.faces,
            r.counts.edges,
            r.counts.open_edges,
            r.counts.nonmanifold_edges,
            r.counts.self_intersecting_pairs,
            r.measures.signed_volume_m3
        );
        for reason in &r.reasons {
            println!("  {}: {}", reason.code.as_str(), reason.message);
        }
    }
    if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(3)
    }
}

/// `repair <in> <out.simpa> [--weld-tolerance m] [--json]`: every change logged; exit 3 refused.
fn repair_cmd(args: &[&str]) -> ExitCode {
    use simpa_core::geometry::repair::{self, RepairOptions, RepairStatus};
    let a = match geo_args(args, &["unit", "up", "weld", "weld-tolerance"]) {
        Ok(a) => a,
        Err(e) => return fail(&e),
    };
    let [input, out] = a.positional.as_slice() else {
        return fail(&format!("repair needs <in> <out.simpa>\n{USAGE}"));
    };
    let mut project = match project_from(Path::new(input), &a) {
        Ok(p) => p,
        Err(e) => return fail(&e),
    };
    let tolerance = match a.options.get("weld-tolerance").map(|t| t.parse::<f64>()) {
        None => repair::DEFAULT_WELD_TOLERANCE_M,
        Some(Ok(t)) => t,
        Some(Err(_)) => return fail("--weld-tolerance must be a number of metres"),
    };
    let outcome = match repair::repair(
        &project.geometry,
        &RepairOptions {
            weld_tolerance_m: tolerance,
        },
    ) {
        Ok(o) => o,
        Err(e) => return fail(&format!("repair: {} ({e})", e.code())),
    };
    if a.json {
        println!(
            "{}",
            serde_json::to_string(&outcome).expect("outcome serialises")
        );
    } else {
        for c in &outcome.changes {
            println!("{}", serde_json::to_string(c).expect("change serialises"));
        }
    }
    if outcome.status == RepairStatus::Refused {
        return ExitCode::from(3);
    }
    project.geometry = outcome.geometry;
    if let Err(e) = schema::save(&project, Path::new(out)) {
        return fail(&format!("{out}: {e}"));
    }
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
