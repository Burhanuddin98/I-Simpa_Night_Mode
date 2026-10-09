//! Parity M17 and M48: the project's own spectrum library (upstream's user spectra,
//! `tree_scene/e_scene_bdd_spectrums_user.h`) and a source linked to an entry of it, which follows
//! the entry when the entry is edited (`generic_element/e_property_freq.cpp:151-175`).
//!
//! What this holds, on every committed room fixture:
//! - a project without a library saves without a `spectra` key and with no `library` link, so a
//!   project saved before the library existed loads and saves unchanged, and its solver input
//!   does not move;
//! - a link reaches no solver: a source linked to an entry writes exactly what the same shape
//!   unlinked writes;
//! - editing the entry moves every linked source's shape in the one edit, and no other; undone,
//!   the project and its config.xml are what they were;
//! - an entry in use cannot be removed, a linked spectrum whose shape is not its entry's is
//!   refused, and a band change keeps every link.

use std::path::{Path, PathBuf};

use serde_json::Value;
use simpa_core::config_xml::{SolverKind, write};
use simpa_core::schema::{self, BandKind, BandSet, F64, Op, Project, SpectrumShape};

fn repo(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(rel)
}

fn workdir() -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(r"C:\runs\library test")
    } else {
        PathBuf::from("/runs/library test")
    }
}

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
    assert!(out.len() >= 10, "fixtures not found in {}", dir.display());
    out
}

fn configs(p: &Project) -> [String; 2] {
    [SolverKind::Spps, SolverKind::Tcr].map(|k| {
        write(p, k, None, Path::new(&workdir())).unwrap_or_else(|e| panic!("{}: {e}", e.code()))
    })
}

fn op(v: Value) -> Op {
    let text = v.to_string();
    Op::from_json(&text).unwrap_or_else(|e| panic!("{text}: {e}"))
}

const ENTRY: &str = "6a1b0e5c-0000-4000-8000-0000000000a1";

/// A library entry on `n` bands, levels rising 2 dB a band from 60 dB.
fn entry(name: &str, n: usize, step: f64) -> Value {
    let levels: Vec<f64> = (0..n).map(|i| 60.0 + step * i as f64).collect();
    serde_json::json!({ "id": ENTRY, "name": name, "levels_db": levels })
}

/// The source `s` (as JSON) linked to the entry: its shape the entry's levels.
fn linked(s: &Value, entry: &Value) -> Value {
    let mut s = s.clone();
    s["power"]["shape"] =
        serde_json::json!({ "kind": "custom", "relative_db": entry["levels_db"] });
    s["power"]["library"] = entry["id"].clone();
    s
}

#[test]
fn a_project_without_a_library_saves_as_before() {
    for (name, p) in fixtures() {
        let json = schema::to_json(&p);
        assert!(!json.contains("\"spectra\""), "{name}: no spectra key");
        assert!(!json.contains("\"library\""), "{name}: no link");
        let back = schema::from_json(&json).unwrap();
        assert_eq!(back, p, "{name}: reads back");
        assert_eq!(schema::to_json(&back), json, "{name}: saves the same bytes");
    }
}

#[test]
fn a_link_reaches_no_solver_and_an_edited_entry_moves_its_sources_only() {
    let mut checked = 0;
    for (name, original) in fixtures() {
        if original.sources.is_empty() {
            continue;
        }
        let n = original.bands.len();
        let e = entry("Speech", n, 2.0);
        let before = configs(&original);
        let mut p = original.clone();
        let mut undo = vec![
            op(serde_json::json!({ "op": "add_spectrum", "index": 0, "spectrum": e }))
                .apply(&mut p)
                .unwrap(),
        ];
        assert_eq!(configs(&p), before, "{name}: an entry alone writes nothing");
        // The first source unlinked with the entry's shape, then linked: the same bytes.
        let s0 = serde_json::to_value(&original.sources[0]).unwrap();
        let mut plain = linked(&s0, &e);
        plain["power"].as_object_mut().unwrap().remove("library");
        let mut q = p.clone();
        op(serde_json::json!({ "op": "replace_source", "source": plain }))
            .apply(&mut q)
            .unwrap();
        undo.push(
            op(serde_json::json!({ "op": "replace_source", "source": linked(&s0, &e) }))
                .apply(&mut p)
                .unwrap(),
        );
        assert_eq!(
            p.sources[0].power.library.map(|i| i.to_string()).as_deref(),
            Some(ENTRY)
        );
        let with_link = configs(&p);
        assert_eq!(with_link, configs(&q), "{name}: the link reaches no solver");

        // The entry edited: the linked source follows, every other source keeps its shape.
        let edited = entry("Speech, edited", n, -1.5);
        let mut r = p.clone();
        let back = op(serde_json::json!({ "op": "replace_spectrum", "spectrum": edited }))
            .apply(&mut r)
            .unwrap();
        assert_eq!(
            r.sources[0].power.shape,
            SpectrumShape::Custom {
                relative_db: (0..n).map(|i| F64::new(60.0 - 1.5 * i as f64)).collect()
            },
            "{name}: the linked source takes the entry's new levels"
        );
        assert_eq!(
            r.sources[0].power.global_db, p.sources[0].power.global_db,
            "{name}: its own level kept"
        );
        for (a, b) in r.sources.iter().zip(&p.sources).skip(1) {
            assert_eq!(a, b, "{name}: an unlinked source does not move");
        }
        r.check_integrity().unwrap();
        if n > 1 {
            let moved = configs(&r);
            assert_ne!(moved, with_link, "{name}: the solver gets the new spectrum");
        }
        back.apply(&mut r).unwrap();
        assert_eq!(r, p, "{name}: the edit undone");

        // In use, the entry cannot be removed; the link is named.
        let id: schema::SpectrumId = serde_json::from_value(Value::from(ENTRY)).unwrap();
        let refused = Op::RemoveSpectrum { id }.apply(&mut p.clone()).unwrap_err();
        assert_eq!(refused.code(), "in_use", "{name}");
        assert!(
            refused.to_string().contains(&original.sources[0].name),
            "{name}: {refused}"
        );

        // A linked spectrum with another shape is refused, and so is a link to no entry.
        let mut wrong = linked(&s0, &e);
        wrong["power"]["shape"]["relative_db"][0] = Value::from(1.0);
        let e1 = op(serde_json::json!({ "op": "replace_source", "source": wrong }))
            .apply(&mut p.clone())
            .unwrap_err();
        assert!(
            matches!(&e1, schema::OpError::Integrity(i) if i.code() == "spectrum_link"),
            "{name}: {e1}"
        );
        let mut dangling = linked(&s0, &e);
        dangling["power"]["library"] = Value::from("6a1b0e5c-0000-4000-8000-0000000000ff");
        let e2 = op(serde_json::json!({ "op": "replace_source", "source": dangling }))
            .apply(&mut p.clone())
            .unwrap_err();
        assert!(
            matches!(&e2, schema::OpError::Integrity(i) if i.code() == "dangling_reference"),
            "{name}: {e2}"
        );

        // Saved and read back, the library and the link are kept.
        let json = schema::to_json(&p);
        assert_eq!(schema::from_json(&json).unwrap(), p, "{name}: round trip");

        for u in undo.into_iter().rev() {
            u.apply(&mut p).unwrap();
        }
        assert_eq!(p, original, "{name}: undone whole");
        assert_eq!(configs(&p), before, "{name}: undone, byte-identical");
        checked += 1;
    }
    assert!(checked >= 10, "{checked}");
}

#[test]
fn a_band_change_keeps_every_link() {
    let (name, original) = fixtures()
        .into_iter()
        .find(|(_, p)| p.bands.kind == BandKind::Octave && !p.sources.is_empty())
        .expect("an octave fixture with a source");
    let n = original.bands.len();
    let e = entry("Speech", n, 2.0);
    let mut p = original.clone();
    op(serde_json::json!({ "op": "add_spectrum", "index": 0, "spectrum": e }))
        .apply(&mut p)
        .unwrap();
    let s0 = serde_json::to_value(&original.sources[0]).unwrap();
    op(serde_json::json!({ "op": "replace_source", "source": linked(&s0, &e) }))
        .apply(&mut p)
        .unwrap();
    let thirds = BandSet::range(BandKind::ThirdOctave, 100, 5000).unwrap();
    let rebanded = p.rebanded(thirds.clone()).expect("rebanded");
    let mut q = p.clone();
    let undo = rebanded.apply(&mut q).unwrap();
    q.check_integrity().unwrap();
    assert_eq!(q.spectra[0].levels_db.len(), thirds.len(), "{name}");
    assert_eq!(
        q.sources[0].power.shape,
        q.spectra[0].shape(),
        "{name}: still linked"
    );
    undo.apply(&mut q).unwrap();
    assert_eq!(q, p, "{name}: undone");
}
