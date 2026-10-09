//! M12c P1, "built in the engine, add the control": every control the Simulate settings gained
//! for a setting the engine already wrote sends one `set_solver_settings` op holding the whole
//! solver settings as the project has them, that one field changed
//! (`app/ui/src/features/simulate/settings.ts`). This holds the two promises the controls make,
//! on every project committed under `tests/fixtures/rooms/`:
//!
//! - turned and turned back, the solver input (`config.xml` of SPPS and of TCR) is byte-identical
//!   to what the project wrote before, so an existing project's input is unchanged unless the
//!   user changes the control;
//! - turned once, it differs from before by exactly the one attribute upstream writes for that
//!   setting, with upstream's value, and nothing else moves.

use std::path::{Path, PathBuf};

use serde_json::Value;
use simpa_core::config_xml::{SolverKind, write};
use simpa_core::schema::{self, Op, Project};

fn repo(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

fn workdir() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(r"C:\runs\controls test")
    } else {
        PathBuf::from("/runs/controls test")
    }
}

/// Every committed project fixture, by file name.
fn fixtures() -> Vec<(String, Project)> {
    let dir = repo("tests/fixtures/rooms");
    let mut out: Vec<(String, Project)> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{}: {e}", dir.display()))
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "simpa"))
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            let project = schema::load(&p).unwrap_or_else(|e| panic!("{name}: {e}"));
            (name, project)
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    assert!(
        out.len() >= 10,
        "the fixtures were not found in {}",
        dir.display()
    );
    out
}

fn config(p: &Project, solver: SolverKind) -> String {
    write(p, solver, None, Path::new(&workdir())).unwrap_or_else(|e| panic!("{}: {e}", e.code()))
}

/// One control: where its field sits under `solvers` in the project JSON, the value it sets
/// (from the value stored), and the `simulation` attribute and value upstream writes for that.
struct Control {
    row: &'static str,
    solver: SolverKind,
    field: &'static [&'static str],
    turn: fn(&Value) -> Value,
    written: fn(&Value) -> String,
    attr: &'static str,
}

fn flip(v: &Value) -> Value {
    Value::Bool(!v.as_bool().expect("a switch is a bool"))
}

fn flag(v: &Value) -> String {
    if v.as_bool().expect("a switch is a bool") {
        "1".to_string()
    } else {
        "0".to_string()
    }
}

fn other_quantity(v: &Value) -> Value {
    match v.as_str() {
        Some("intensity") => Value::from("spl"),
        Some("spl") => Value::from("intensity"),
        other => panic!("sound_map {other:?}"),
    }
}

/// Upstream's `surf_receiv_method` list (`e_core_sppscore.h:58-64`): 0 "Soundmap: intensity",
/// 1 "Soundmap: SPL".
fn quantity_code(v: &Value) -> String {
    match v.as_str() {
        Some("intensity") => "0".to_string(),
        Some("spl") => "1".to_string(),
        other => panic!("sound_map {other:?}"),
    }
}

fn controls() -> Vec<Control> {
    vec![
        Control {
            row: "C14 SPPS air absorption",
            solver: SolverKind::Spps,
            field: &["spps", "air_absorption"],
            turn: flip,
            written: flag,
            attr: "abs_atmo_calc",
        },
        Control {
            row: "C23 TCR air absorption",
            solver: SolverKind::Tcr,
            field: &["tcr", "air_absorption"],
            turn: flip,
            written: flag,
            attr: "abs_atmo_calc",
        },
        Control {
            row: "C17 SPPS transmission",
            solver: SolverKind::Spps,
            field: &["spps", "transmission"],
            turn: flip,
            written: flag,
            attr: "trans_calc",
        },
        Control {
            row: "C20 SPPS sound-map quantity",
            solver: SolverKind::Spps,
            field: &["spps", "sound_map"],
            turn: other_quantity,
            written: quantity_code,
            attr: "surf_receiv_method",
        },
    ]
}

fn get<'a>(v: &'a Value, path: &[&str]) -> &'a Value {
    path.iter().fold(v, |v, k| &v[*k])
}

/// The op the control sends: the project's solver settings, as JSON, with `field` set to `to`.
fn control_op(p: &Project, field: &[&str], to: Value) -> Op {
    let mut solvers = serde_json::to_value(&p.solvers).unwrap();
    let (last, parents) = field.split_last().unwrap();
    let slot = parents.iter().fold(&mut solvers, |v, k| &mut v[*k]);
    slot[*last] = to;
    let text = serde_json::json!({ "op": "set_solver_settings", "settings": solvers }).to_string();
    Op::from_json(&text).unwrap_or_else(|e| panic!("{text}: {e}"))
}

/// The one ` attr="value"` of `xml`'s `simulation` element; panics unless there is exactly one.
fn only_attr(xml: &str, attr: &str) -> String {
    let needle = format!(" {attr}=\"");
    assert_eq!(xml.matches(&needle).count(), 1, "{attr} is written once");
    let start = xml.find(&needle).unwrap() + needle.len();
    let len = xml[start..].find('"').unwrap();
    xml[start..start + len].to_string()
}

#[test]
fn every_control_turned_back_leaves_the_solver_input_byte_identical() {
    let mut checked = 0;
    for (name, original) in fixtures() {
        for c in controls() {
            let mut p = original.clone();
            let before = config(&p, c.solver);
            let stored = get(&serde_json::to_value(&p.solvers).unwrap(), c.field).clone();
            let to = (c.turn)(&stored);

            // Turned once: exactly the one attribute, with upstream's value for it.
            let undo_turn = control_op(&p, c.field, to.clone()).apply(&mut p).unwrap();
            let turned = config(&p, c.solver);
            let was = only_attr(&before, c.attr);
            assert_eq!(
                was,
                (c.written)(&stored),
                "{name} {}: the stored value is what was written",
                c.row
            );
            let now = (c.written)(&to);
            assert_ne!(was, now, "{name} {}", c.row);
            let expected = before.replacen(
                &format!(" {}=\"{was}\"", c.attr),
                &format!(" {}=\"{now}\"", c.attr),
                1,
            );
            assert_eq!(
                turned, expected,
                "{name} {}: only {} changes",
                c.row, c.attr
            );
            // The other solver's input does not move.
            let other = match c.solver {
                SolverKind::Spps => SolverKind::Tcr,
                SolverKind::Tcr => SolverKind::Spps,
            };
            assert_eq!(
                config(&p, other),
                config(&original, other),
                "{name} {}: the other solver",
                c.row
            );

            // Turned back by the control (a second op), and by undo: byte-identical to before.
            let mut back = p.clone();
            control_op(&back, c.field, stored.clone())
                .apply(&mut back)
                .unwrap();
            assert_eq!(
                config(&back, c.solver).as_bytes(),
                before.as_bytes(),
                "{name} {}: turned back",
                c.row
            );
            assert_eq!(
                back, original,
                "{name} {}: the project itself is as it was",
                c.row
            );
            undo_turn.apply(&mut p).unwrap();
            assert_eq!(
                config(&p, c.solver).as_bytes(),
                before.as_bytes(),
                "{name} {}: undone",
                c.row
            );
            println!(
                "{name}: {} {}=\"{was}\" -> \"{now}\" -> \"{was}\", bytes identical",
                c.row, c.attr
            );
            checked += 1;
        }
    }
    println!("{checked} control x fixture pairs checked");
}

// ---- the meshing controls: TetGen's input, not config.xml -------------------------------------
//
// G34's face size and G36's scene correction change what TetGen is given (`.poly`, `.var`, its
// flags), never config.xml. The same two promises, held on that input and on the mesh input hash
// a run records, with config.xml checked unchanged throughout.

/// What TetGen is given for `p`, as bytes: the `.poly`, the `.var` when there is one, the flags;
/// and the mesh input hash.
#[derive(Debug, PartialEq)]
struct MeshInputBytes {
    poly: Vec<u8>,
    var: Option<Vec<u8>>,
    flags: Vec<String>,
    hash: String,
}

fn mesh_input(p: &Project) -> MeshInputBytes {
    let input = simpa_core::mesh::project_input(p).unwrap_or_else(|e| panic!("{}", e.0));
    MeshInputBytes {
        poly: simpa_core::formats::poly::write(&input.poly),
        var: input.var,
        flags: simpa_core::mesh::tetgen_flags(&p.solvers.meshing),
        hash: simpa_core::validate::mesh_input_hash(p),
    }
}

fn both_configs(p: &Project) -> [String; 2] {
    [config(p, SolverKind::Spps), config(p, SolverKind::Tcr)]
}

/// The op G34's field sends (`withReceiverFaceArea`, settings.ts): the face size set or cleared;
/// setting one turns -Y off with it, clearing leaves -Y as it is.
fn face_area_op(p: &Project, area: Option<f64>) -> Op {
    let mut solvers = serde_json::to_value(&p.solvers).unwrap();
    let m = &mut solvers["meshing"];
    m["surface_receiver_max_area_m2"] = area.map_or(Value::Null, Value::from);
    if area.is_some() {
        m["preserve_boundary"] = Value::Bool(false);
    }
    let text = serde_json::json!({ "op": "set_solver_settings", "settings": solvers }).to_string();
    Op::from_json(&text).unwrap_or_else(|e| panic!("{text}: {e}"))
}

#[test]
fn the_face_size_turned_back_leaves_tetgen_input_byte_identical() {
    for (name, original) in fixtures() {
        let before = mesh_input(&original);
        let configs = both_configs(&original);
        let stored = original
            .solvers
            .meshing
            .surface_receiver_max_area_m2
            .map(|a| a.get());
        let y_was = original.solvers.meshing.preserve_boundary;
        let mut p = original.clone();
        let (to, back) = match stored {
            None => (Some(2.5), None),
            Some(a) => (None, Some(a)),
        };
        let undo = face_area_op(&p, to).apply(&mut p).unwrap();
        let turned = mesh_input(&p);
        assert_eq!(
            both_configs(&p),
            configs,
            "{name}: config.xml does not hold the mesh settings"
        );
        assert_eq!(
            turned.poly, before.poly,
            "{name}: the .poly does not change"
        );
        match to {
            Some(_) => {
                assert!(
                    turned.var.is_some() && before.var.is_none(),
                    "{name}: a .var appears"
                );
                assert!(
                    !turned.flags.contains(&"-Y".to_string()),
                    "{name}: -Y goes off with it"
                );
                assert!(!p.solvers.meshing.preserve_boundary);
            }
            None => {
                assert!(
                    turned.var.is_none() && before.var.is_some(),
                    "{name}: the .var goes"
                );
                assert_eq!(
                    turned.flags, before.flags,
                    "{name}: clearing leaves -Y as it was"
                );
            }
        }
        assert_ne!(turned.hash, before.hash, "{name}: a run would mesh again");

        // Turned back by the field.
        let mut again = p.clone();
        face_area_op(&again, back).apply(&mut again).unwrap();
        let back_input = mesh_input(&again);
        if stored.is_some() || !y_was {
            assert_eq!(
                back_input, before,
                "{name}: turned back, TetGen's input is byte-identical"
            );
            assert_eq!(again, original, "{name}: the project is as it was");
        } else {
            // -Y was on and setting a size turned it off; clearing leaves it off, the one
            // difference, said in the field's help line. Undo (below) restores both.
            let mut want = before.flags.clone();
            want.retain(|f| f != "-Y");
            assert_eq!(
                (&back_input.poly, &back_input.var, &back_input.flags),
                (&before.poly, &before.var, &want),
                "{name}"
            );
        }
        assert_eq!(both_configs(&again), configs, "{name}");
        // Undone: byte-identical, -Y included.
        undo.apply(&mut p).unwrap();
        assert_eq!(mesh_input(&p), before, "{name}: undone");
        assert_eq!(p, original, "{name}: undone, the project is as it was");
        println!(
            "{name}: G34 face size {stored:?} -> {to:?} -> {back:?} (-Y was {y_was}), TetGen input identical after undo"
        );
    }
}

#[test]
fn the_scene_correction_turned_back_leaves_tetgen_input_byte_identical() {
    for (name, original) in fixtures() {
        let before = mesh_input(&original);
        let configs = both_configs(&original);
        let stored = original.solvers.meshing.preprocess;
        let mut p = original.clone();
        let undo = control_op(&p, &["meshing", "preprocess"], Value::Bool(!stored))
            .apply(&mut p)
            .unwrap();
        assert_eq!(p.solvers.meshing.preprocess, !stored);
        let turned = mesh_input(&p);
        assert_eq!(
            both_configs(&p),
            configs,
            "{name}: config.xml does not hold it"
        );
        assert_eq!(
            turned.flags, before.flags,
            "{name}: TetGen's flags do not change"
        );
        assert_eq!(turned.var, before.var, "{name}: nor the .var");
        assert_ne!(turned.hash, before.hash, "{name}: a run would mesh again");
        let mut back = p.clone();
        control_op(&back, &["meshing", "preprocess"], Value::Bool(stored))
            .apply(&mut back)
            .unwrap();
        assert_eq!(
            mesh_input(&back),
            before,
            "{name}: turned back, byte-identical"
        );
        assert_eq!(back, original, "{name}");
        undo.apply(&mut p).unwrap();
        assert_eq!(mesh_input(&p), before, "{name}: undone");
        println!(
            "{name}: G36 preprocess {stored} -> {} -> {stored}, .poly {} on the way, TetGen input identical turned back",
            !stored,
            if turned.poly == before.poly {
                "the same"
            } else {
                "rewritten for preprocess.exe"
            }
        );
    }
}
