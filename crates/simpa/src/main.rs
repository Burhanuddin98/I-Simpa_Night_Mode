use std::path::Path;
use std::process::ExitCode;

use simpa_core::{config_xml, formats, schema, validate};

mod aural_cmd;
mod bed_cmd;
mod extra_bed_cmd;
mod mesh_run;
mod results_cmd;

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
  simpa repair <in> <out.simpa> [--weld-tolerance <m>] [--json]      exit 3 when refused
  simpa mesh <project.simpa | file.poly> --out <dir> [--json] [--tetgen <exe>]
             [--preprocess <exe>] [--parity] [--from-tetgen <dir> [--basename <b>]]
             [--cancel-after-ms <n>] [--tetgen-timeout-ms <n>] [--preprocess-timeout-ms <n>]
      TetGen's and preprocess.exe's lines on stderr; exit 0, 2, 3, 4, 130. A project whose mesh
      settings ask for upstream's scene correction goes through preprocess.exe first; when it
      saves nothing, the .poly as written is meshed, as upstream's GUI meshes it, and a note on
      stderr says so. --parity keeps preprocess.exe's facet markers byte for byte, as upstream's
      GUI meshes them, fails the mesh when they do not verify, and writes the .mbin for byte
      comparison only; without the scene correction it has no effect, and a note says so.
      --cancel-after-ms cancels preprocess.exe or TetGen that long after it starts; each is
      stopped at its time limit (default 1 h for TetGen, 10 min for preprocess.exe).
  simpa mesh-verify <dir> [--json] [--room-id <n>] [--fittings <a,b,..>]   exit 4 when it fails
  simpa run <project.simpa> --solver spps|tcr [--device cpu|gpu] [--variant <v> | --base] [--mesh <dir>]
            [--runs <root>] [--loss-limit <f>] [--cancel-after-ms <n>] [--cancel-after-progress <p>]
            [--solver-exe <exe>] [--tetgen <exe>] [--preprocess <exe>] [--json]
      run exports the file's active variant, as the app and simpa validate use it; --variant <v>
      (an id or a name) exports another, --base the project's own materials; not both.
  simpa run-folder <dir> --solver spps|tcr [--device cpu|gpu] [--runs <root>] [--solver-exe <exe>]
            [--loss-limit <f>] [--cancel-after-ms <n>] [--cancel-after-progress <p>] [--json]
      --device gpu runs SPPS on the GPU (spps-gpu.exe, which must be the verified build and find a
      CUDA device: exit 2 with the reason otherwise); run.json records the device. Default cpu.
      run and run-folder print each solver line on stderr as 'CLASS  text', and the run manifest
      (--json) or one verdict line naming the run folder on stdout. The runs root defaults to
      'runs' beside the project (run) or in the current folder (run-folder). Exit 0 OK,
      2 usage or validation, 3 geometry refused, 4 mesh, 5 solver FAIL or CRASH, 130 cancelled.
  simpa results <run-folder> [--json]                        a verified run's results and parameters
            [--decay-range <dB,...>] [--clarity-ms <ms,...>] [--definition-ms <ms,...>]
      exit 0; 2 usage; 5 the run is FAIL, CRASH or CANCELLED; 6 its results do not verify.
      The lists add decay times from -5 dB down each range (10 to 60 dB) and C and D at each
      time limit (5 to 1000 ms) to every SPPS series, as parameters.custom
  simpa bed-extra r15|r20|r27 [--out <file>]                M12c's closed-form beds of the numbers
      parity added: the cases as JSON on stdout (and in <file>). Exit 0 only when every case holds;
      8 not passed; 2 usage
  simpa results --schema                                     the JSON Schemas of results --json
  simpa reband <project.simpa> <out.simpa> --kind octave|third_octave --lo <hz> --hi <hz>
      the project moved onto every nominal band of that kind from --lo to --hi, each new band
      taking every per-band value of the nearest current band (the app's band presets, PQ3 C26);
      exit 2 for a range that is not nominal frequencies, lowest first
  simpa advise <project.simpa> [--json]                      the run-quality advisor before a run:
      meshing that splits the walls, receivers small for the room, a run shorter than its decay,
      fewer particles than the noise model was measured with; each names a setting and the value
      Apply sets (after a run the same advice is `advice` in results --json). Exit 0, advice
      or none; 2 usage or a project that does not load.
  simpa bed <bed.json> --out <root> [--jobs <n>] [--from <earlier>] [--upstream <dir>] [--json]
      M8a's T30 physics bed (docs/investigations/2026-09-29-m8a/SPEC.md): checks the solvers
      against solvers/manifest.json (exit 2 when not the verified build), runs the matrix
      (--jobs at once, default 4) or reads an earlier bed's runs (--from), and writes
      report.json, summary.json and decays/ under <root>/<UTC stamp>/. Exit 0 only when
      report.pass is true; 8 not passed; 5 a run was not OK; 2 usage or an invalid bed file.
  simpa bed --schema | --canonical                            report.json's schema; M8a's bed file
  simpa auralize <run-folder> --receiver <name> [--source <name> | --summed] [--source-audio <wav>]
            --out <wav> [--pcm24] [--seed <n>]
      a receiver's impulse response synthesised from the SPPS energy echogram (not a measured or
      wave-based one), or with --source-audio a dry or anechoic recording convolved with it (resampled to 48 kHz
      first); mono, 48 kHz, peak -1 dBFS, 32-bit float or 24-bit PCM; the seed and the gain to the
      run's scale in the WAV's comment. The sources summed unless --source names one. Exit 0; 2
      usage or refused; 5, 6 as results
  Executables: --solver-exe / --tetgen, else $SIMPA_SOLVERS_DIR, else beside simpa.exe (its
  solvers/ folder first), else the nearest target/solvers/bin above it.";

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
        ["mesh", rest @ ..] => mesh_run::mesh_cmd(rest),
        ["mesh-verify", rest @ ..] => mesh_run::mesh_verify_cmd(rest),
        ["run", rest @ ..] => mesh_run::run_cmd(rest),
        ["run-folder", rest @ ..] => mesh_run::run_folder_cmd(rest),
        ["results", rest @ ..] => results_cmd::results_cmd(rest),
        ["advise", rest @ ..] => advise_cmd(rest),
        ["reband", rest @ ..] => reband_cmd(rest),
        ["bed", rest @ ..] => bed_cmd::bed_cmd(rest),
        ["bed-extra", rest @ ..] => extra_bed_cmd::extra_bed_cmd(rest),
        ["auralize", rest @ ..] => aural_cmd::auralize_cmd(rest),
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

/// `reband <in> <out> --kind octave|third_octave --lo <hz> --hi <hz>`: the app's band preset as a
/// command (`Project::rebanded`, through the same op the app applies), so a project can be moved
/// onto another band set without the UI. Writes `out` as the app writes a project.
fn reband_cmd(args: &[&str]) -> ExitCode {
    use simpa_core::schema::{BandKind, BandSet};
    let (mut input, mut output, mut kind, mut lo, mut hi) = (None, None, None, None, None);
    let mut i = 0;
    while i < args.len() {
        match args[i] {
            "--kind" | "--lo" | "--hi" if i + 1 < args.len() => {
                match args[i] {
                    "--kind" => kind = Some(args[i + 1]),
                    "--lo" => lo = args[i + 1].parse::<u32>().ok(),
                    _ => hi = args[i + 1].parse::<u32>().ok(),
                }
                i += 2;
            }
            a if input.is_none() => {
                input = Some(a);
                i += 1;
            }
            a if output.is_none() => {
                output = Some(a);
                i += 1;
            }
            a => return fail(&format!("unexpected argument '{a}'\n{USAGE}")),
        }
    }
    let (Some(input), Some(output), Some(kind), Some(lo), Some(hi)) = (input, output, kind, lo, hi)
    else {
        return fail(&format!(
            "reband needs <in> <out> --kind --lo --hi\n{USAGE}"
        ));
    };
    let kind = match kind {
        "octave" => BandKind::Octave,
        "third_octave" => BandKind::ThirdOctave,
        other => {
            return fail(&format!(
                "unknown band kind '{other}': octave or third_octave"
            ));
        }
    };
    let mut project = match validate::read_project(Path::new(input)) {
        Ok(p) => p,
        Err(e) => return fail(&format!("{input}: {e} ({})", e.code())),
    };
    let Some(bands) = BandSet::range(kind, lo, hi) else {
        return fail(&format!(
            "{lo} Hz to {hi} Hz is not a range of nominal {kind:?} frequencies, lowest first"
        ));
    };
    let Some(op) = project.rebanded(bands) else {
        return fail("the project's bands cannot be mapped onto that set");
    };
    if let Err(e) = op.apply(&mut project) {
        return fail(&format!("reband refused: {e}"));
    }
    if let Err(e) = std::fs::write(output, schema::to_json(&project)) {
        return fail(&format!("{output}: {e}"));
    }
    println!(
        "{output}: {} bands, {} Hz to {} Hz",
        project.bands.frequencies_hz.len(),
        project.bands.frequencies_hz.first().copied().unwrap_or(0),
        project.bands.frequencies_hz.last().copied().unwrap_or(0)
    );
    ExitCode::SUCCESS
}

/// `advise <project> [--json]`: the run-quality advisor before a run (`simpa_core::advise::before`).
/// `--json` prints the items as `results --json` prints `advice`; otherwise one block per item.
/// Exit 0 whatever it advises: advice never blocks a run.
fn advise_cmd(args: &[&str]) -> ExitCode {
    let mut json = false;
    let mut file = None;
    for &a in args {
        match a {
            "--json" => json = true,
            _ if file.is_none() && !a.starts_with("--") => file = Some(Path::new(a)),
            _ => {
                return fail(&format!(
                    "unexpected argument '{a}'
{USAGE}"
                ));
            }
        }
    }
    let Some(file) = file else {
        return fail(&format!(
            "advise needs a project file
{USAGE}"
        ));
    };
    let project = match schema::load(file) {
        Ok(p) => p,
        Err(e) => return fail(&format!("{}: {e}", file.display())),
    };
    let advice = simpa_core::advise::before(&project);
    if json {
        println!(
            "{}",
            serde_json::to_string(&advice).expect("advice serialises")
        );
        return ExitCode::SUCCESS;
    }
    if advice.is_empty() {
        println!(
            "no advice: nothing in the project's settings is known to make its values noisy or refused"
        );
    }
    for a in &advice {
        println!("{}: {}", a.code, a.cause);
        println!("  fix: {}", a.fix.words);
        if let (Some(label), Some(from), Some(to)) = (&a.fix.label, &a.fix.from, &a.fix.to) {
            println!(
                "  apply: {label} {} -> {}",
                setting_text(from),
                setting_text(to)
            );
        }
        if let Some(why) = &a.fix.why_no_apply {
            println!("  no apply: {why}");
        }
        if let Some(note) = &a.fix.note {
            println!("  note: {note}");
        }
    }
    ExitCode::SUCCESS
}

fn setting_text(v: &simpa_core::advise::SettingValue) -> String {
    match v {
        simpa_core::advise::SettingValue::Bool(b) => (if *b { "on" } else { "off" }).to_string(),
        simpa_core::advise::SettingValue::Number(x) => x.to_string(),
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
        "air_volume_m3": report.measures.air_volume_m3,
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
            "{} vertices, {} faces, {} groups, area {:.3} m2, volume {:.3} m3, air {:.3} m3",
            s["vertices"],
            s["faces"],
            project.surface_groups.len(),
            s["area_m2"].as_f64().unwrap_or(0.0),
            s["volume_m3"].as_f64().unwrap_or(0.0),
            s["air_volume_m3"].as_f64().unwrap_or(0.0)
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
    // Name the project after its file rather than leaving upstream's default name (the app's
    // File › Open… applies the same rule).
    simpa_core::geometry::import::name_after_file(&mut imported.project, Path::new(input));
    if let Err(e) = schema::save(&imported.project, Path::new(out)) {
        return fail(&format!("{out}: {e}"));
    }
    let report = &imported.report;
    if !a.json {
        for note in &report.notes {
            eprintln!("note: {note}");
        }
        return print_summary(&imported.project, false);
    }
    // The summary, and what the import recorded: upstream's element id of each entity it made,
    // which it also pins as that entity's `solver_id` (`geometry::import::proj`, "Element ids"),
    // and its notes.
    let mut s = project_summary(&imported.project);
    s["upstream_ids"] = report
        .upstream_ids
        .iter()
        .map(|u| {
            serde_json::json!({
                "kind": u.kind.name(),
                "config_element": u.kind.config_element(),
                "name": u.name,
                "id": u.entity.to_string(),
                "upstream": u.upstream,
            })
        })
        .collect();
    s["notes"] = serde_json::json!(report.notes);
    println!("{s}");
    ExitCode::SUCCESS
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
            "air_volume_m3": r.measures.air_volume_m3,
            "area_m2": r.measures.area_m2,
            "report": r,
        });
        println!("{out}");
    } else {
        println!(
            "{}: {} faces, {} edges ({} open, {} non-manifold), {} self-intersecting pairs, volume {:.3} m3, air {:.3} m3",
            if ok { "ok" } else { "refused" },
            r.counts.faces,
            r.counts.edges,
            r.counts.open_edges,
            r.counts.nonmanifold_edges,
            r.counts.self_intersecting_pairs,
            r.measures.signed_volume_m3,
            r.measures.air_volume_m3
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
