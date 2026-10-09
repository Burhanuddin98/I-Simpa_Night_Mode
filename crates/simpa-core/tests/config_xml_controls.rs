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

// A31: the project's name and description are the project's own; neither reaches a solver.
#[test]
fn the_project_name_and_description_never_reach_the_solver_input() {
    for (name, original) in fixtures() {
        let configs = both_configs(&original);
        let mesh = mesh_input(&original);
        let mut p = original.clone();
        for text in [
            r#"{"op":"set_project_name","name":"Renamed hall, stage left"}"#,
            r#"{"op":"set_description","description":"Seats empty; curtains open."}"#,
        ] {
            Op::from_json(text).unwrap().apply(&mut p).unwrap();
        }
        assert_eq!(p.name, "Renamed hall, stage left");
        assert_eq!(p.description, "Seats empty; curtains open.");
        assert_eq!(
            both_configs(&p),
            configs,
            "{name}: config.xml is byte-identical"
        );
        assert_eq!(
            mesh_input(&p),
            mesh,
            "{name}: TetGen's input is byte-identical"
        );
    }
}

// ---- M43: the on/off switches of surface receivers, cutting planes and fitting zones ----------
//
// The app sends the receiver or zone back whole with `enabled` changed (`replace_surface_receiver`,
// `replace_fitting_zone`, actions.ts). Off, config.xml loses exactly that element (solver ids do
// not depend on the enabled flags, so nothing else renumbers); on again, it is byte-identical.

/// `xml` without the element `<tag id="<id>" ...>` (and its children), an emptied list folded to
/// `<list/>` as the writer folds one.
fn without_element(xml: &str, tag: &str, id: i32) -> String {
    let start = format!("<{tag} id=\"{id}\"");
    let lines: Vec<&str> = xml.lines().collect();
    let at = lines
        .iter()
        .position(|l| l.trim_start().starts_with(&start))
        .unwrap_or_else(|| panic!("{start} is written"));
    let end = if lines[at].trim_end().ends_with("/>") {
        at
    } else {
        at + lines[at..]
            .iter()
            .position(|l| l.trim() == format!("</{tag}>"))
            .expect("the element closes")
    };
    let mut kept: Vec<String> = lines[..at].iter().map(|l| l.to_string()).collect();
    kept.extend(lines[end + 1..].iter().map(|l| l.to_string()));
    // An emptied list: `<list>` straight followed by `</list>` folds to `<list/>`.
    let mut out: Vec<String> = Vec::new();
    for l in kept {
        let close = l
            .trim()
            .strip_prefix("</")
            .and_then(|s| s.strip_suffix('>'));
        if let (Some(name), Some(prev)) = (close, out.last())
            && prev.trim() == format!("<{name}>")
        {
            let folded = prev.replace(&format!("<{name}>"), &format!("<{name}/>"));
            *out.last_mut().unwrap() = folded;
            continue;
        }
        out.push(l);
    }
    let mut s = out.join("\n");
    if xml.ends_with('\n') {
        s.push('\n');
    }
    s
}

/// `a == b`, or a panic naming the first line that differs (not the whole of two files).
fn same_text(a: &str, b: &str, what: &str) {
    if a == b {
        return;
    }
    let (la, lb): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    let i = (0..la.len().max(lb.len()))
        .find(|&i| la.get(i) != lb.get(i))
        .unwrap_or(0);
    panic!(
        "{what}: line {} differs:\n  got  {:?}\n  want {:?}",
        i + 1,
        la.get(i),
        lb.get(i)
    );
}

fn replace_op(kind: &str, field: &str, item: Value) -> Op {
    let text = serde_json::json!({ "op": kind, field: item }).to_string();
    Op::from_json(&text).unwrap_or_else(|e| panic!("{text}: {e}"))
}

#[test]
fn every_enabled_switch_turned_back_leaves_the_solver_input_byte_identical() {
    let mut checked = 0;
    for (name, original) in fixtures() {
        let ids = simpa_core::config_xml::SolverIds::assign(&original).unwrap();
        let configs = both_configs(&original);
        let mesh = mesh_input(&original);
        let mut items: Vec<(String, &str, &str, i32, Value)> = Vec::new();
        for r in &original.surface_receivers {
            let tag = match r.shape {
                schema::SurfaceReceiverShape::Scene { .. } => "recepteur_surfacique",
                schema::SurfaceReceiverShape::CuttingPlane { .. } => "recepteur_surfacique_coupe",
            };
            items.push((
                r.name.clone(),
                "replace_surface_receiver",
                tag,
                ids.surface_receiver_id(r.id).unwrap(),
                serde_json::json!({ "receiver": r }),
            ));
        }
        for z in &original.fitting_zones {
            items.push((
                z.name.clone(),
                "replace_fitting_zone",
                "encombrement",
                ids.fitting_zone_id(z.id).unwrap(),
                serde_json::json!({ "zone": z }),
            ));
        }
        for (what, op, tag, id, holder) in items {
            let field = if op == "replace_fitting_zone" {
                "zone"
            } else {
                "receiver"
            };
            let mut item = holder[field].clone();
            assert_eq!(item["enabled"], Value::Bool(true), "{name} {what}");
            item["enabled"] = Value::Bool(false);
            let mut p = original.clone();
            let undo = replace_op(op, field, item.clone()).apply(&mut p).unwrap();
            let off = both_configs(&p);
            // The last enabled box zone off takes the drawn zones' material 0 with it (config.xml
            // declares it only while drawn zone triangles exist, write.rs).
            let drawn_left = p
                .fitting_zones
                .iter()
                .any(|z| z.enabled && matches!(z.shape, schema::FittingShape::Box { .. }));
            let drawn_went = tag == "encombrement" && !drawn_left;
            for (k, solver) in [SolverKind::Spps, SolverKind::Tcr].into_iter().enumerate() {
                assert_ne!(off[k], configs[k], "{name} {what} off ({solver:?})");
                let mut want = without_element(&configs[k], tag, id);
                if drawn_went {
                    want = without_element(&want, "type_surface", 0);
                }
                same_text(
                    &off[k],
                    &want,
                    &format!("{name} {what} off ({solver:?}): exactly its element goes"),
                );
            }
            item["enabled"] = Value::Bool(true);
            let mut back = p.clone();
            replace_op(op, field, item).apply(&mut back).unwrap();
            assert_eq!(back, original, "{name} {what}: switched back");
            assert_eq!(
                both_configs(&back),
                configs,
                "{name} {what}: switched back, byte-identical"
            );
            assert_eq!(mesh_input(&back), mesh, "{name} {what}: TetGen's input");
            undo.apply(&mut p).unwrap();
            assert_eq!(both_configs(&p), configs, "{name} {what}: undone");
            println!("{name}: M43 {what} ({tag} id {id}) off and on, bytes identical");
            checked += 1;
        }
    }
    assert!(checked >= 15, "{checked}");
    println!("{checked} switches checked");
}

// ---- M27: a source's group is the project's own; it reaches no solver --------------------------
//
// The Sources step sends the source back whole with its group set (`replace_source`, M27 in
// SourcesPanel.tsx). Upstream writes a group's sources in its place, as the writer writes every
// source, so the solver input does not move.

#[test]
fn a_source_group_never_reaches_the_solver_input() {
    let mut checked = 0;
    for (name, original) in fixtures() {
        let configs = both_configs(&original);
        let mesh = mesh_input(&original);
        let mut p = original.clone();
        for (i, s) in original.sources.iter().enumerate() {
            let mut item = serde_json::to_value(s).unwrap();
            item["group"] = Value::from(if i % 2 == 0 { "Stage / Left" } else { "Pit" });
            replace_op("replace_source", "source", item)
                .apply(&mut p)
                .unwrap();
            checked += 1;
        }
        assert!(p.sources.iter().all(|s| s.group.is_some()), "{name}");
        assert_eq!(
            both_configs(&p),
            configs,
            "{name}: config.xml is byte-identical"
        );
        assert_eq!(
            mesh_input(&p),
            mesh,
            "{name}: TetGen's input is byte-identical"
        );
    }
    println!("{checked} sources grouped, solver input unchanged");
}

// ---- M40: a surface map added over a surface, and taken away again -----------------------------
//
// "+ Surface map" sends `add_surface_receiver` with a scene receiver over the picked groups
// (`newSceneReceiver`, ops.ts), at the end of the list. Not added, nothing changes; added,
// config.xml gains exactly its one `recepteur_surfacique`; undone, byte-identical.

#[test]
fn a_surface_map_added_and_undone_leaves_the_solver_input_byte_identical() {
    let mut checked = 0;
    for (name, original) in fixtures() {
        let configs = both_configs(&original);
        let mesh = mesh_input(&original);
        let mapped: Vec<schema::GroupId> = original
            .surface_receivers
            .iter()
            .filter(|r| r.enabled)
            .filter_map(|r| match &r.shape {
                schema::SurfaceReceiverShape::Scene { groups } => Some(groups.clone()),
                _ => None,
            })
            .flatten()
            .collect();
        let Some(group) = original.surface_groups.iter().find(|g| {
            !mapped.contains(&g.id) && original.geometry.faces.iter().any(|f| f.group == g.id)
        }) else {
            println!("{name}: every surface is mapped already");
            continue;
        };
        let id = "6d0b7a3e-1f2c-4e5d-9a8b-7c6d5e4f3a2b";
        let receiver = serde_json::json!({
            "id": id, "name": "Surface map 1", "enabled": true,
            "shape": { "kind": "scene", "groups": [group.id] }, "solver_id": null
        });
        let text = serde_json::json!({
            "op": "add_surface_receiver",
            "index": original.surface_receivers.len(),
            "receiver": receiver
        })
        .to_string();
        let mut p = original.clone();
        let undo = Op::from_json(&text).unwrap().apply(&mut p).unwrap();
        let new_id = simpa_core::config_xml::SolverIds::assign(&p)
            .unwrap()
            .surface_receiver_id(p.surface_receivers.last().unwrap().id)
            .unwrap();
        let added = both_configs(&p);
        for k in 0..2 {
            assert_ne!(added[k], configs[k], "{name}: the map is written");
            assert_eq!(
                added[k]
                    .matches(&format!(
                        "<recepteur_surfacique id=\"{new_id}\" name=\"Surface map 1\"/>"
                    ))
                    .count(),
                1,
                "{name}: once, as upstream writes a surface receiver"
            );
            same_text(
                &without_element(&added[k], "recepteur_surfacique", new_id),
                &configs[k],
                &format!("{name}: only the map's line is new"),
            );
        }
        undo.apply(&mut p).unwrap();
        assert_eq!(p, original, "{name}: undone");
        assert_eq!(both_configs(&p), configs, "{name}: undone, byte-identical");
        assert_eq!(mesh_input(&p), mesh, "{name}: undone, TetGen's input");
        println!(
            "{name}: M40 map over '{}' (id {new_id}) added and undone",
            group.name
        );
        checked += 1;
    }
    assert!(checked >= 10, "{checked}");
}

// ---- G28/G29: fitting zones added and edited ----------------------------------------------------
//
// "+ Box zone" sends `add_fitting_zone` with upstream's new zone (zones.ts `newBoxZone`: absorption
// 0, mean free path 1 m, uniform diffusion in every band, a 1 m box on the floor); a band is
// `set_fitting_band`, every band at once or the box `replace_fitting_zone`. Not used, nothing
// changes; used, config.xml moves by exactly what upstream writes for it; turned back or undone,
// byte-identical.

/// The one `bfreq` line of encombrement `id` at band `band`.
fn zone_band_line(xml: &str, id: i32, band: usize) -> String {
    let start = format!("<encombrement id=\"{id}\">");
    let at = xml.find(&start).expect("the zone is written");
    xml[at..]
        .lines()
        .skip(1)
        .nth(band)
        .expect("one line a band")
        .to_string()
}

#[test]
fn a_new_box_zone_added_and_undone_leaves_the_solver_input_byte_identical() {
    let mut checked = 0;
    for (name, original) in fixtures() {
        let configs = both_configs(&original);
        let mesh = mesh_input(&original);
        let n = original.bands.len();
        let zone = serde_json::json!({
            "id": "1b2c3d4e-5f60-4718-8a9b-0c1d2e3f4a5b", "name": "Fitting zone 1", "enabled": true,
            "shape": { "kind": "box", "min": [0.5, 0.5, 0.0], "max": [1.5, 1.5, 1.0], "destination": null },
            "absorption": vec![0.0; n], "mean_free_path_m": vec![1.0; n],
            "diffusion_law": vec!["uniform"; n], "solver_id": null
        });
        let text = serde_json::json!({
            "op": "add_fitting_zone", "index": original.fitting_zones.len(), "zone": zone
        })
        .to_string();
        let mut p = original.clone();
        let undo = Op::from_json(&text).unwrap().apply(&mut p).unwrap();
        let id = simpa_core::config_xml::SolverIds::assign(&p)
            .unwrap()
            .fitting_zone_id(p.fitting_zones.last().unwrap().id)
            .unwrap();
        let drawn_before = original
            .fitting_zones
            .iter()
            .any(|z| z.enabled && matches!(z.shape, schema::FittingShape::Box { .. }));
        let added = both_configs(&p);
        for k in 0..2 {
            let line = zone_band_line(&added[k], id, 0);
            assert!(
                line.contains(" alpha=\"0\" lambda=\"1\" loi_diff=\"0\""),
                "{name}: upstream's new zone: {line}"
            );
            let mut back = without_element(&added[k], "encombrement", id);
            if !drawn_before {
                back = without_element(&back, "type_surface", 0);
            }
            same_text(&back, &configs[k], &format!("{name}: only the zone is new"));
        }
        assert_ne!(mesh_input(&p), mesh, "{name}: the box is meshed");
        undo.apply(&mut p).unwrap();
        assert_eq!(p, original, "{name}: undone");
        assert_eq!(both_configs(&p), configs, "{name}: undone, byte-identical");
        assert_eq!(mesh_input(&p), mesh, "{name}: undone, TetGen's input");
        checked += 1;
    }
    println!("{checked} fixtures: a box zone added and undone");
}

#[test]
fn a_zone_band_edited_and_turned_back_changes_only_that_band() {
    let (name, original) = fixtures()
        .into_iter()
        .find(|(_, p)| !p.fitting_zones.is_empty())
        .expect("a fixture with a fitting zone");
    let z = &original.fitting_zones[0];
    let id = simpa_core::config_xml::SolverIds::assign(&original)
        .unwrap()
        .fitting_zone_id(z.id)
        .unwrap();
    let configs = both_configs(&original);
    let mesh = mesh_input(&original);
    let band = 3;
    for (quantity, attr, to) in [
        ("absorption", "alpha", 0.35),
        ("mean_free_path", "lambda", 2.5),
    ] {
        let stored = match quantity {
            "absorption" => z.absorption[band].get(),
            _ => z.mean_free_path_m[band].get(),
        };
        let op = |v: f64| {
            let text = serde_json::json!({
                "op": "set_fitting_band", "zone": z.id, "quantity": quantity, "band": band, "value": v
            })
            .to_string();
            Op::from_json(&text).unwrap()
        };
        let mut p = original.clone();
        op(to).apply(&mut p).unwrap();
        let turned = both_configs(&p);
        for k in 0..2 {
            let was = zone_band_line(&configs[k], id, band);
            let now = zone_band_line(&turned[k], id, band);
            assert_eq!(
                now,
                was.replace(
                    &format!(" {attr}=\"{stored}\""),
                    &format!(" {attr}=\"{to}\"")
                ),
                "{name}: {quantity} band {band}"
            );
            assert_eq!(
                turned[k],
                configs[k].replacen(&was, &now, 1),
                "{name}: only that line"
            );
        }
        assert_eq!(mesh_input(&p), mesh, "{name}: a band value is not meshed");
        op(stored).apply(&mut p).unwrap();
        assert_eq!(p, original, "{name}: turned back");
        assert_eq!(
            both_configs(&p),
            configs,
            "{name}: turned back, byte-identical"
        );
        println!(
            "{name}: G28 {quantity} band {band} {stored} -> {to} -> {stored}, one attribute, bytes identical"
        );
    }

    // Every band's law at once (the "Diffusion law" choice), and back.
    let mut item = serde_json::to_value(z).unwrap();
    let laws = item["diffusion_law"].clone();
    item["diffusion_law"] = Value::from(vec!["lambert_reflection"; original.bands.len()]);
    let mut p = original.clone();
    replace_op("replace_fitting_zone", "zone", item.clone())
        .apply(&mut p)
        .unwrap();
    for (k, t) in both_configs(&p).iter().enumerate() {
        assert_eq!(
            *t,
            configs[k].replace(" loi_diff=\"0\"/>", " loi_diff=\"2\"/>"),
            "{name}: every band's loi_diff, nothing else"
        );
    }
    item["diffusion_law"] = laws;
    replace_op("replace_fitting_zone", "zone", item.clone())
        .apply(&mut p)
        .unwrap();
    assert_eq!(both_configs(&p), configs, "{name}: the law turned back");

    // The box moved: TetGen's input moves, config.xml does not; moved back, byte-identical.
    let min = item["shape"]["min"].clone();
    item["shape"]["min"][2] = Value::from(0.25);
    let mut p = original.clone();
    replace_op("replace_fitting_zone", "zone", item.clone())
        .apply(&mut p)
        .unwrap();
    assert_eq!(
        both_configs(&p),
        configs,
        "{name}: a box is not in config.xml"
    );
    assert_ne!(
        mesh_input(&p),
        mesh,
        "{name}: the moved box is meshed again"
    );
    item["shape"]["min"] = min;
    replace_op("replace_fitting_zone", "zone", item)
        .apply(&mut p)
        .unwrap();
    assert_eq!(p, original, "{name}: moved back");
    assert_eq!(mesh_input(&p), mesh, "{name}: moved back, TetGen's input");
}

// ---- M45: a material's display colour is the project's own; it reaches no solver -----------------
//
// The Materials step sends the material back whole with its colour changed (`replace_material`,
// `actions.setMaterialColor`). No solver file carries a colour: config.xml's `type_surface` and the
// TetGen input are byte-identical with every material recoloured.

#[test]
fn a_material_colour_never_reaches_the_solver_input() {
    let mut checked = 0;
    for (name, original) in fixtures() {
        let configs = both_configs(&original);
        let mesh = mesh_input(&original);
        let mut p = original.clone();
        for (i, m) in original.materials.iter().enumerate() {
            let mut item = serde_json::to_value(m).unwrap();
            let color = format!(
                "#{:02x}{:02x}{:02x}",
                255 - (i * 37 % 256),
                (i * 91) % 256,
                17
            );
            assert_ne!(item["color"], Value::from(color.clone()), "{name}");
            item["color"] = Value::from(color);
            replace_op("replace_material", "material", item)
                .apply(&mut p)
                .unwrap();
            checked += 1;
        }
        assert!(
            p.materials
                .iter()
                .zip(&original.materials)
                .all(|(a, b)| a.color != b.color),
            "{name}: every material recoloured"
        );
        assert_eq!(
            both_configs(&p),
            configs,
            "{name}: config.xml is byte-identical"
        );
        assert_eq!(
            mesh_input(&p),
            mesh,
            "{name}: TetGen's input is byte-identical"
        );
    }
    assert!(checked >= 10, "{checked}");
    println!("{checked} materials recoloured, solver input unchanged");
}

// ---- M37: a point receiver's group is the project's own; it reaches no solver --------------------
//
// The Sources step sends the receiver back whole with its group set (`replace_point_receiver`, M37 in
// SourcesPanel.tsx). Upstream writes a group's receivers in its place (`e_scene_recepteursp.h:152-161`),
// as the writer writes every receiver, so the solver input does not move; cleared again, the project
// is the one it was.

#[test]
fn a_receiver_group_never_reaches_the_solver_input() {
    let mut checked = 0;
    for (name, original) in fixtures() {
        let configs = both_configs(&original);
        let mesh = mesh_input(&original);
        let mut p = original.clone();
        for (i, r) in original.point_receivers.iter().enumerate() {
            let mut item = serde_json::to_value(r).unwrap();
            item["group"] = Value::from(if i % 2 == 0 {
                "Stalls / Front"
            } else {
                "Balcony"
            });
            replace_op("replace_point_receiver", "receiver", item)
                .apply(&mut p)
                .unwrap();
            checked += 1;
        }
        assert!(
            p.point_receivers.iter().all(|r| r.group.is_some()),
            "{name}"
        );
        assert_eq!(
            both_configs(&p),
            configs,
            "{name}: config.xml is byte-identical"
        );
        assert_eq!(
            mesh_input(&p),
            mesh,
            "{name}: TetGen's input is byte-identical"
        );
        for r in &original.point_receivers {
            let item = serde_json::to_value(r).unwrap();
            replace_op("replace_point_receiver", "receiver", item)
                .apply(&mut p)
                .unwrap();
        }
        assert_eq!(p, original, "{name}: every group cleared again");
    }
    assert!(checked >= 10, "{checked}");
    println!("{checked} receivers grouped, solver input unchanged");
}

// ---- G50: a marker's colour and its name shown or not are the project's own -----------------------
//
// The Sources step and the Fitting zones section send the element back whole with its `display`
// set (`replace_source`, `replace_point_receiver`, `replace_fitting_zone`). Upstream's render
// properties reach no solver, and neither does this: every source, receiver and zone of every
// fixture coloured and its name hidden, both solvers' config.xml and TetGen's input are
// byte-identical; set back to the defaults, the project is the one it was.

#[test]
fn a_marker_display_never_reaches_the_solver_input() {
    let display = serde_json::json!({ "color": "#3fa7d6", "show_name": false });
    let mut checked = 0;
    for (name, original) in fixtures() {
        let configs = both_configs(&original);
        let mesh = mesh_input(&original);
        let mut p = original.clone();
        let mut items: Vec<(&str, &str, Value)> = Vec::new();
        for s in &original.sources {
            items.push(("replace_source", "source", serde_json::to_value(s).unwrap()));
        }
        for r in &original.point_receivers {
            items.push((
                "replace_point_receiver",
                "receiver",
                serde_json::to_value(r).unwrap(),
            ));
        }
        for z in &original.fitting_zones {
            items.push((
                "replace_fitting_zone",
                "zone",
                serde_json::to_value(z).unwrap(),
            ));
        }
        for (op, field, item) in &items {
            let mut changed = item.clone();
            changed["display"] = display.clone();
            replace_op(op, field, changed).apply(&mut p).unwrap();
            checked += 1;
        }
        assert!(
            p.sources.iter().all(|s| s.display.is_some())
                && p.point_receivers.iter().all(|r| r.display.is_some())
                && p.fitting_zones.iter().all(|z| z.display.is_some()),
            "{name}"
        );
        assert_eq!(
            both_configs(&p),
            configs,
            "{name}: config.xml is byte-identical"
        );
        assert_eq!(
            mesh_input(&p),
            mesh,
            "{name}: TetGen's input is byte-identical"
        );
        for (op, field, item) in items {
            replace_op(op, field, item).apply(&mut p).unwrap();
        }
        assert_eq!(p, original, "{name}: every display set back");
    }
    assert!(checked >= 20, "{checked}");
    println!("{checked} markers coloured and named off, solver input unchanged");
}

// ---- M41: a cutting plane's corners, any orientation --------------------------------------------
//
// The Sources step sends the plane back whole with one coordinate of one corner moved
// (`replace_surface_receiver`, `withCorner` in planes.ts). config.xml writes the three corners as
// upstream's GUI writes them (`recepteur_surfacique_coupe@ax`..`@cz`), and SPPS lays its grid along
// BC and BA whatever their direction (`base_core_configuration.cpp:289-292`), so a tilted plane
// moves exactly the one attribute; turned back, byte-identical. A plane is no mesh input.

/// The one `<recepteur_surfacique_coupe id="<id>" ...>` line of `xml`.
fn plane_line(xml: &str, id: i32) -> String {
    let start = format!("<recepteur_surfacique_coupe id=\"{id}\"");
    let lines: Vec<&str> = xml
        .lines()
        .filter(|l| l.trim_start().starts_with(&start))
        .collect();
    assert_eq!(lines.len(), 1, "{start} is written once");
    lines[0].to_string()
}

#[test]
fn a_tilted_cutting_plane_moves_only_its_corner_and_turned_back_is_byte_identical() {
    let mut checked = 0;
    for (name, original) in fixtures() {
        let ids = simpa_core::config_xml::SolverIds::assign(&original).unwrap();
        let configs = both_configs(&original);
        let mesh = mesh_input(&original);
        for r in &original.surface_receivers {
            let schema::SurfaceReceiverShape::CuttingPlane { a, .. } = &r.shape else {
                continue;
            };
            if !r.enabled {
                continue;
            }
            let id = ids.surface_receiver_id(r.id).unwrap();
            let az = a.to_array()[2] + 1.25;
            let mut item = serde_json::to_value(r).unwrap();
            item["shape"]["a"][2] = Value::from(az);
            let mut p = original.clone();
            replace_op("replace_surface_receiver", "receiver", item)
                .apply(&mut p)
                .unwrap();
            let tilted = both_configs(&p);
            for k in 0..2 {
                let before = plane_line(&configs[k], id);
                let after = plane_line(&tilted[k], id);
                let old_az = format!(" az=\"{}\"", only_attr(&before, "az"));
                let new_az = format!(" az=\"{}\"", only_attr(&after, "az"));
                assert_ne!(old_az, new_az, "{name} {}: the corner moved", r.name);
                assert_eq!(
                    after,
                    before.replace(&old_az, &new_az),
                    "{name} {}: only A's z moves on its line",
                    r.name
                );
                assert_eq!(
                    only_attr(&after, "az").parse::<f64>().unwrap(),
                    az,
                    "{name} {}: written as typed, shortest round trip",
                    r.name
                );
                same_text(
                    &tilted[k].replace(&after, &before),
                    &configs[k],
                    &format!("{name} {}: nothing else moves", r.name),
                );
            }
            assert_eq!(mesh_input(&p), mesh, "{name} {}: TetGen's input", r.name);
            let back = serde_json::to_value(r).unwrap();
            replace_op("replace_surface_receiver", "receiver", back)
                .apply(&mut p)
                .unwrap();
            assert_eq!(p, original, "{name} {}: turned back", r.name);
            assert_eq!(
                both_configs(&p),
                configs,
                "{name} {}: turned back, byte-identical",
                r.name
            );
            println!("{name}: M41 {} (id {id}) tilted, az {az}, and back", r.name);
            checked += 1;
        }
    }
    assert!(checked >= 2, "{checked}");
}

// ---- C26+: Select all / Unselect all on the bands -----------------------------------------------
//
// The settings editor sends one `set_band_computed` per band that changes, in one batch
// (`settings.ts` `bandsToSwitch`). Unselect all writes every `bfreq` of that solver with
// `docalc="0"` and moves nothing else; Select all writes every one with `docalc="1"`; the bands set
// back as they were, config.xml is byte-identical.

fn every_band_op(p: &Project, solver: &str, on: bool) -> Option<Op> {
    let flags = &serde_json::to_value(&p.solvers).unwrap()[solver]["bands_computed"];
    let ops: Vec<Value> = flags
        .as_array()
        .unwrap()
        .iter()
        .enumerate()
        .filter(|(_, f)| f.as_bool() != Some(on))
        .map(|(i, _)| serde_json::json!({ "op": "set_band_computed", "solver": solver, "band": i, "computed": on }))
        .collect();
    (!ops.is_empty()).then(|| {
        let text = serde_json::json!({ "op": "batch", "ops": ops }).to_string();
        Op::from_json(&text).unwrap_or_else(|e| panic!("{text}: {e}"))
    })
}

/// The `freq_enum` bands of `xml` (a material's `bfreq` carries no `docalc`).
fn bfreq_lines(xml: &str) -> Vec<&str> {
    xml.lines()
        .filter(|l| l.trim_start().starts_with("<bfreq ") && l.contains(" docalc="))
        .collect()
}

#[test]
fn every_band_selected_or_none_and_set_back_leaves_the_solver_input_byte_identical() {
    let mut checked = 0;
    for (name, original) in fixtures() {
        for (solver, kind) in [("spps", SolverKind::Spps), ("tcr", SolverKind::Tcr)] {
            let before = config(&original, kind);
            let other = match kind {
                SolverKind::Spps => SolverKind::Tcr,
                SolverKind::Tcr => SolverKind::Spps,
            };
            let other_before = config(&original, other);
            let mut p = original.clone();
            let mut undo: Vec<Op> = Vec::new();
            if let Some(op) = every_band_op(&p, solver, false) {
                undo.push(op.clone().apply(&mut p).unwrap());
            }
            let none = config(&p, kind);
            let bands = bfreq_lines(&none);
            assert!(
                bands.len() == original.bands.frequencies_hz.len()
                    && bands.iter().all(|l| l.contains(r#"docalc="0""#)),
                "{name} {solver}: every band off"
            );
            let strip = |x: &str| {
                x.lines()
                    .filter(|l| !l.contains(" docalc="))
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            same_text(
                &strip(&none),
                &strip(&before),
                &format!("{name} {solver}: only the bands move"),
            );
            assert_eq!(
                config(&p, other),
                other_before,
                "{name} {solver}: the other solver's input is untouched"
            );
            let op = every_band_op(&p, solver, true).expect("every band was off");
            undo.push(op.apply(&mut p).unwrap());
            assert!(
                bfreq_lines(&config(&p, kind))
                    .iter()
                    .all(|l| l.contains(r#"docalc="1""#)),
                "{name} {solver}: every band on"
            );
            assert!(
                every_band_op(&p, solver, true).is_none(),
                "{name} {solver}: Select all again sends nothing"
            );
            for u in undo.into_iter().rev() {
                u.apply(&mut p).unwrap();
            }
            same_text(
                &config(&p, kind),
                &before,
                &format!("{name} {solver}: set back, byte-identical"),
            );
            assert_eq!(p, original, "{name} {solver}: the project as it was");
            checked += 1;
        }
    }
    assert!(checked >= 20, "{checked}");
}

// ---- M5: a material's reflection law edited in one band (v1.1-backlog 20) -------------------------
//
// The Materials step's Law per band sends the material back whole with `reflection_law` one per
// band, that band changed (`withBandLaw`, `actions.setBandLaw`). Upstream writes the law per band,
// `type_surface/bfreq@loi` (`e_data_row_materiau.h:65-81`), and both solvers read it per band
// (`lib_interface/data_manager/base_core_configuration.cpp:220`): config.xml must move by exactly
// that attribute of that band, in every `type_surface` of a group that has the material, with the
// solver code; a list with one law in every band must write what the single law writes; and set
// back, the input is byte-identical.

/// The solver code of a law spelled as in the project JSON.
fn law_code(law: &str) -> u32 {
    [
        "specular",
        "uniform",
        "lambert",
        "w2",
        "w3",
        "w4",
        "semi_diffuse",
    ]
    .iter()
    .position(|l| *l == law)
    .unwrap_or_else(|| panic!("law {law}")) as u32
}

#[test]
fn a_reflection_law_edited_in_one_band_moves_only_that_bands_loi() {
    let mut checked = 0;
    for (name, original) in fixtures() {
        let configs = both_configs(&original);
        let n = original.bands.len();
        let band = n / 2;
        let hz = original.bands.frequencies_hz[band];
        for m in &original.materials {
            let item = serde_json::to_value(m).unwrap();
            let laws: Vec<String> = match &item["reflection_law"] {
                Value::String(l) => vec![l.clone(); n],
                Value::Array(a) => a.iter().map(|l| l.as_str().unwrap().to_string()).collect(),
                other => panic!("{name}: reflection_law {other}"),
            };
            if laws.iter().any(|l| l == "semi_diffuse") {
                continue; // refused at export (v1.1-backlog 22); nothing is written to compare
            }
            // One law in every band spelled as a list writes what the single law writes.
            let mut spelled = item.clone();
            spelled["reflection_law"] = Value::from(laws.clone());
            let mut p = original.clone();
            replace_op("replace_material", "material", spelled)
                .apply(&mut p)
                .unwrap();
            assert_eq!(
                both_configs(&p),
                configs,
                "{name} '{}': the same laws as a list",
                m.name
            );

            let old = laws[band].clone();
            let new = if old == "lambert" { "w2" } else { "lambert" };
            let mut changed = laws.clone();
            changed[band] = new.to_string();
            let mut edited = item.clone();
            edited["reflection_law"] = Value::from(changed);
            let mut p = original.clone();
            let undo = replace_op("replace_material", "material", edited)
                .apply(&mut p)
                .unwrap();
            let groups = original
                .surface_groups
                .iter()
                .filter(|g| g.material == m.id)
                .count();
            let after = both_configs(&p);
            for (k, solver) in [SolverKind::Spps, SolverKind::Tcr].into_iter().enumerate() {
                let (a, b): (Vec<&str>, Vec<&str>) =
                    (after[k].lines().collect(), configs[k].lines().collect());
                assert_eq!(a.len(), b.len(), "{name} {solver:?}: the same lines");
                let from = format!(" loi=\"{}\"", law_code(&old));
                let to = format!(" loi=\"{}\"", law_code(new));
                let mut moved = 0;
                for (x, y) in a.iter().zip(&b) {
                    if x == y {
                        continue;
                    }
                    assert!(
                        y.contains(&format!("<bfreq freq=\"{hz}\" "))
                            && *x == y.replace(&from, &to),
                        "{name} '{}' {solver:?}: only {hz} Hz's loi moves:\n  got  {x}\n  was  {y}",
                        m.name
                    );
                    moved += 1;
                }
                assert_eq!(
                    moved, groups,
                    "{name} '{}' {solver:?}: one line per group that has it",
                    m.name
                );
            }
            undo.apply(&mut p).unwrap();
            assert_eq!(p, original, "{name} '{}': undone", m.name);
            assert_eq!(
                both_configs(&p),
                configs,
                "{name} '{}': undone, byte-identical",
                m.name
            );
            if groups > 0 {
                checked += 1;
            }
        }
    }
    assert!(checked >= 10, "{checked}");
    println!("{checked} materials edited in one band, only that band's loi moved");
}

// ---- M46: a source's band attenuated, and set back --------------------------------------------
//
// The Sources step's spectrum table sends the source back whole with `power.attenuation_db` set
// (`withBandAttenuation`, `emission.ts`), or without it once every band is back at 0. Upstream
// writes a source's band as its Lw minus its attenuation (`e_data_row_ext_bandefreq.h:114-117`):
// config.xml must move by exactly that source's band `db`, by the attenuation; an attenuation of
// 0 in every band must write what none writes; and set back, the input is byte-identical.

/// The line and `db` of band `band` of the `<source` element named `name`.
fn source_band_db(xml: &str, name: &str, band: usize) -> (usize, f64) {
    let lines: Vec<&str> = xml.lines().collect();
    let named = format!(" name=\"{name}\"");
    let at = lines
        .iter()
        .position(|l| l.trim_start().starts_with("<source ") && l.contains(&named))
        .unwrap_or_else(|| panic!("source {name} is written"));
    let line = at + 1 + band;
    let text = lines[line];
    let db = text
        .split(" db=\"")
        .nth(1)
        .and_then(|r| r.split('"').next())
        .unwrap_or_else(|| panic!("no db in {text}"));
    (line, db.parse().unwrap())
}

#[test]
fn a_source_band_attenuated_moves_only_that_bands_db() {
    let mut checked = 0;
    for (name, original) in fixtures() {
        let configs = both_configs(&original);
        let n = original.bands.len();
        let band = n / 2;
        // The first enabled source: the k-th source written.
        let Some(k) = original.sources.iter().position(|s| s.enabled) else {
            continue;
        };
        let item = serde_json::to_value(&original.sources[k]).unwrap();
        assert!(item["power"].get("attenuation_db").is_none(), "{name}");

        // 0 dB in every band, spelled out: the bytes of no attenuation.
        let mut zeros = item.clone();
        zeros["power"]["attenuation_db"] = Value::from(vec![0.0; n]);
        let mut p = original.clone();
        replace_op("replace_source", "source", zeros)
            .apply(&mut p)
            .unwrap();
        assert_eq!(both_configs(&p), configs, "{name}: 0 dB everywhere");

        let mut att = vec![0.0; n];
        att[band] = 6.0;
        let mut edited = item.clone();
        edited["power"]["attenuation_db"] = Value::from(att);
        let mut p = original.clone();
        let undo = replace_op("replace_source", "source", edited)
            .apply(&mut p)
            .unwrap();
        let after = both_configs(&p);
        let source = original.sources[k].name.as_str();
        for (i, solver) in [SolverKind::Spps, SolverKind::Tcr].into_iter().enumerate() {
            let (line, was) = source_band_db(&configs[i], source, band);
            let (_, now) = source_band_db(&after[i], source, band);
            assert!(
                (was - 6.0 - now).abs() < 1e-4,
                "{name} {solver:?}: {was} dB less 6 dB is {now} dB"
            );
            let (a, b): (Vec<&str>, Vec<&str>) =
                (after[i].lines().collect(), configs[i].lines().collect());
            assert_eq!(a.len(), b.len(), "{name} {solver:?}");
            let moved: Vec<usize> = (0..a.len()).filter(|&j| a[j] != b[j]).collect();
            assert_eq!(
                moved,
                vec![line],
                "{name} {solver:?}: only that band's db moves"
            );
        }
        undo.apply(&mut p).unwrap();
        assert_eq!(p, original, "{name}: undone");
        assert_eq!(both_configs(&p), configs, "{name}: undone, byte-identical");
        checked += 1;
    }
    assert!(checked >= 10, "{checked}");
    println!("{checked} sources attenuated in one band, only that band's db moved");
}
