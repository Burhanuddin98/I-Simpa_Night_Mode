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

fn controls() -> Vec<Control> {
    vec![Control {
        row: "C14 SPPS air absorption",
        solver: SolverKind::Spps,
        field: &["spps", "air_absorption"],
        turn: flip,
        written: flag,
        attr: "abs_atmo_calc",
    }]
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
