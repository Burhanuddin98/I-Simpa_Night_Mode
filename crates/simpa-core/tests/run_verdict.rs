//! The run verdict, one reason code at a time: each test starts from a run that is OK in every
//! signal (tutorial 1's expectation, clean lines, every file, clean statistics or finite tables)
//! and breaks exactly one thing, and the verdict must name exactly that code.

#[path = "run_support.rs"]
mod support;

use simpa_core::process::{Line, Outcome, Stream};
use simpa_core::run::verdict::codes::*;
use simpa_core::run::{
    BandStats, ExpectError, Expectation, LINE_RULES, LineClass, Outputs, StatsError, Status,
    Verdict, classify_all, judge,
};
use simpa_core::run::{DEFAULT_LOSS_LIMIT, Evidence};
use simpa_core::schema::SolverKind;
use support::*;

fn exited(code: u32) -> Outcome {
    Outcome {
        exit_code: Some(code),
        cancelled: false,
        elapsed_ms: 10.0,
    }
}

fn clean_lines(solver: SolverKind) -> Vec<Line> {
    match solver {
        SolverKind::Spps => clean_spps_lines(),
        SolverKind::Tcr => clean_tcr_lines(),
    }
}

/// Judges `solver`'s tutorial-1 run with the given parts.
fn verdict_of(
    solver: SolverKind,
    outcome: &Outcome,
    lines: &[Line],
    exp: &Expectation,
    outputs: &Outputs,
    loss_limit: f64,
) -> Verdict {
    let classified = classify_all(lines);
    judge(&Evidence {
        solver,
        outcome,
        lines: &classified,
        expectation: Ok(exp),
        outputs,
        loss_limit,
    })
}

/// The clean run of `solver` with `break_it` applied to its outputs.
fn broken(solver: SolverKind, break_it: impl FnOnce(&Expectation, &mut Outputs)) -> Verdict {
    let exp = tutorial1_expectation(solver);
    let mut out = good_outputs(&exp);
    break_it(&exp, &mut out);
    verdict_of(
        solver,
        &exited(0),
        &clean_lines(solver),
        &exp,
        &out,
        DEFAULT_LOSS_LIMIT,
    )
}

fn assert_only(v: &Verdict, status: Status, code: &str) {
    assert_eq!(v.codes(), [code], "{v:#?}");
    assert_eq!(v.status, status, "{v:#?}");
}

#[test]
fn a_clean_spps_run_is_ok_with_no_reasons() {
    let v = broken(SolverKind::Spps, |_, _| {});
    assert_eq!(v.status, Status::Ok, "{v:#?}");
    assert!(v.reasons.is_empty() && v.warnings.is_empty());
    assert!(v.is_ok());
}

#[test]
fn a_clean_tcr_run_is_ok_with_no_reasons() {
    let v = broken(SolverKind::Tcr, |_, _| {});
    assert_eq!(v.status, Status::Ok, "{v:#?}");
    assert!(v.reasons.is_empty() && v.warnings.is_empty());
}

// Signal 1: the exit.

fn with_outcome(solver: SolverKind, outcome: Outcome) -> Verdict {
    let exp = tutorial1_expectation(solver);
    verdict_of(
        solver,
        &outcome,
        &clean_lines(solver),
        &exp,
        &good_outputs(&exp),
        DEFAULT_LOSS_LIMIT,
    )
}

#[test]
fn cancelled_is_never_ok_even_after_exit_0() {
    let v = with_outcome(
        SolverKind::Spps,
        Outcome {
            exit_code: Some(0),
            cancelled: true,
            elapsed_ms: 5.0,
        },
    );
    assert_only(&v, Status::Cancelled, CANCELLED);
    let v = with_outcome(
        SolverKind::Tcr,
        Outcome {
            exit_code: None,
            cancelled: true,
            elapsed_ms: 5.0,
        },
    );
    assert_only(&v, Status::Cancelled, CANCELLED);
}

#[test]
fn crash_access_violation() {
    let v = with_outcome(SolverKind::Spps, exited(0xC000_0005));
    assert_only(&v, Status::Crash, CRASH_ACCESS_VIOLATION);
    assert!(v.reasons[0].detail.contains("0xC0000005"));
}

#[test]
fn crash_abort() {
    assert_only(
        &with_outcome(SolverKind::Tcr, exited(0xC000_0409)),
        Status::Crash,
        CRASH_ABORT,
    );
}

#[test]
fn crash_other() {
    // STATUS_HEAP_CORRUPTION, and a run that reported no exit code at all.
    assert_only(
        &with_outcome(SolverKind::Spps, exited(0xC000_0374)),
        Status::Crash,
        CRASH_OTHER,
    );
    assert_only(
        &with_outcome(
            SolverKind::Spps,
            Outcome {
                exit_code: None,
                cancelled: false,
                elapsed_ms: 1.0,
            },
        ),
        Status::Crash,
        CRASH_OTHER,
    );
}

#[test]
fn exit_nonzero() {
    // TCR's exit 1, and the solvers' -1 on an undeclared material: FAIL, not CRASH.
    assert_only(
        &with_outcome(SolverKind::Tcr, exited(1)),
        Status::Fail,
        EXIT_NONZERO,
    );
    assert_only(
        &with_outcome(SolverKind::Spps, exited(u32::MAX)),
        Status::Fail,
        EXIT_NONZERO,
    );
}

#[test]
fn outputs_are_not_judged_after_a_crash() {
    // Nothing on disk, but the crash is the only reason.
    let exp = tutorial1_expectation(SolverKind::Spps);
    let v = verdict_of(
        SolverKind::Spps,
        &exited(0xC000_0005),
        &clean_spps_lines(),
        &exp,
        &Outputs::default(),
        DEFAULT_LOSS_LIMIT,
    );
    assert_only(&v, Status::Crash, CRASH_ACCESS_VIOLATION);
}

#[test]
fn end_of_calculation_missing() {
    let exp = tutorial1_expectation(SolverKind::Spps);
    let lines: Vec<Line> = clean_spps_lines()
        .into_iter()
        .filter(|l| l.text != "End of calculation.")
        .collect();
    let v = verdict_of(
        SolverKind::Spps,
        &exited(0),
        &lines,
        &exp,
        &good_outputs(&exp),
        DEFAULT_LOSS_LIMIT,
    );
    assert_only(&v, Status::Fail, END_OF_CALCULATION_MISSING);
    // TCR has no such line to miss.
    assert!(broken(SolverKind::Tcr, |_, _| {}).is_ok());
}

// Signal 2: the lines.

/// A line of each FAIL row, as the solver prints it (with its continuation line).
fn fail_sample(id: &str) -> Vec<Line> {
    let out = |t: &str| line(Stream::Stdout, t);
    let err = |t: &str| line(Stream::Stderr, t);
    match id {
        "xml_property_missing" => vec![out("Xml Property directivities_directory doesn't exist !")],
        "scene_mesh_unreadable" => vec![
            out("Unable to read the scene mesh file :"),
            out(r"C:\runs\solve\mesh.cbin"),
        ],
        "tetra_mesh_unreadable" => vec![out(
            "Unable to read the tetrahedalization of the scene mesh file, calculation canceled.",
        )],
        "tetra_mesh_empty" => vec![
            out(r"C:\runs\solve\tetramesh.mbin"),
            out("Tetrahedron file is empty, the calculation can't be done !"),
        ],
        "config_path_missing" => {
            vec![out(
                "The path of the XML configuration file must be specified!",
            )]
        }
        "directivity_not_open" => vec![out("DirectivityBalloon : File not open")],
        "material_missing" => vec![err(
            "Wrong project configuration, a face is defined but no materials are attached to it \
             ! Check your group/material associations",
        )],
        "degenerate_tetrahedron" => vec![Line {
            terminated: false,
            ..err(
                "Error in input mesh, a tetrahedra have at least the same two vertices \
                 idTetra:5 vertices:1 1 2 3",
            )
        }],
        "source_on_surface" => vec![err(
            "A sound source position is intersecting with the 3D model. Move the sound source \
             inside the 3D model",
        )],
        "source_not_located" => vec![Line {
            terminated: false,
            ..err("Unable to find the source position!")
        }],
        "particle_loss_reported" => vec![Line {
            terminated: false,
            ..err(
                "Warning 4000 particles has been in error on 4000 particles. The computation \
                 result may be wrong, please check the particles statitics file for more details.",
            )
        }],
        other => panic!("no sample for {other}"),
    }
}

#[test]
fn each_fail_line_is_its_own_reason_code() {
    let fails: Vec<&str> = LINE_RULES
        .iter()
        .filter(|r| r.class == LineClass::Fail)
        .map(|r| r.id)
        .collect();
    assert_eq!(fails.len(), 11);
    for solver in [SolverKind::Spps, SolverKind::Tcr] {
        let exp = tutorial1_expectation(solver);
        for id in &fails {
            let mut lines = clean_lines(solver);
            lines.splice(1..1, fail_sample(id));
            let v = verdict_of(
                solver,
                &exited(0),
                &lines,
                &exp,
                &good_outputs(&exp),
                DEFAULT_LOSS_LIMIT,
            );
            assert_only(&v, Status::Fail, id);
        }
    }
}

#[test]
fn a_continuation_path_is_part_of_the_reason() {
    for (id, path) in [
        ("scene_mesh_unreadable", r"C:\runs\solve\mesh.cbin"),
        ("tetra_mesh_empty", r"C:\runs\solve\tetramesh.mbin"),
    ] {
        let exp = tutorial1_expectation(SolverKind::Spps);
        let mut lines = clean_spps_lines();
        lines.splice(1..1, fail_sample(id));
        lines.splice(1..1, fail_sample(id)); // twice: one reason, counted
        let v = verdict_of(
            SolverKind::Spps,
            &exited(0),
            &lines,
            &exp,
            &good_outputs(&exp),
            DEFAULT_LOSS_LIMIT,
        );
        assert_only(&v, Status::Fail, id);
        assert!(v.reasons[0].detail.contains(path), "{v:#?}");
        assert!(v.reasons[0].detail.ends_with("(2 lines)"), "{v:#?}");
    }
}

#[test]
fn fail_lines_are_reported_whatever_the_exit() {
    let exp = tutorial1_expectation(SolverKind::Spps);
    let mut lines = clean_spps_lines();
    lines.extend(fail_sample("material_missing"));
    let v = verdict_of(
        SolverKind::Spps,
        &exited(u32::MAX),
        &lines,
        &exp,
        &Outputs::default(),
        DEFAULT_LOSS_LIMIT,
    );
    assert_eq!(v.codes(), [EXIT_NONZERO, "material_missing"]);
    assert_eq!(v.status, Status::Fail);
}

#[test]
fn warn_lines_are_recorded_and_do_not_fail_the_run() {
    let exp = tutorial1_expectation(SolverKind::Spps);
    let mut lines = clean_spps_lines();
    lines.insert(
        1,
        line(
            Stream::Stdout,
            "Source at tetrahedron vertex, move source position from [1;2;3] to [1.01;2;3]",
        ),
    );
    lines.insert(2, line(Stream::Stderr, "a line nobody documented"));
    let v = verdict_of(
        SolverKind::Spps,
        &exited(0),
        &lines,
        &exp,
        &good_outputs(&exp),
        DEFAULT_LOSS_LIMIT,
    );
    assert_eq!(v.status, Status::Ok, "{v:#?}");
    let warned: Vec<(&str, &str)> = v
        .warnings
        .iter()
        .map(|w| (w.code.as_str(), w.detail.as_str()))
        .collect();
    assert_eq!(
        warned,
        [
            (
                "source_moved_off_vertex",
                "Source at tetrahedron vertex, move source position from [1;2;3] to [1.01;2;3]"
            ),
            ("unclassified_line", "a line nobody documented"),
        ]
    );
}

#[test]
fn config_attribute_missing_when_the_config_cannot_be_read() {
    let classified = classify_all(&clean_spps_lines());
    let err = ExpectError::NotXml("unexpected end of stream".into());
    let v = judge(&Evidence {
        solver: SolverKind::Spps,
        outcome: &exited(0),
        lines: &classified,
        expectation: Err(&err),
        outputs: &Outputs::default(),
        loss_limit: DEFAULT_LOSS_LIMIT,
    });
    assert_only(&v, Status::Fail, CONFIG_ATTRIBUTE_MISSING);
}

// Signal 3: the statistics.

#[test]
fn stats_unreadable() {
    let v = broken(SolverKind::Spps, |_, o| {
        o.stats = Some(Err(StatsError::Layout(
            "column 1 is not an integer column".into(),
        )));
    });
    assert_only(&v, Status::Fail, STATS_UNREADABLE);
    let v = broken(SolverKind::Spps, |_, o| o.stats = None);
    assert_only(&v, Status::Fail, STATS_UNREADABLE);
}

fn stats_mut(o: &mut Outputs) -> &mut Vec<BandStats> {
    &mut o.stats.as_mut().unwrap().as_mut().unwrap().bands
}

#[test]
fn stats_band_mismatch() {
    // A band missing, and a band twice (P2 dup_freq).
    let v = broken(SolverKind::Spps, |_, o| {
        stats_mut(o).remove(3);
    });
    assert_only(&v, Status::Fail, STATS_BAND_MISMATCH);
    let v = broken(SolverKind::Spps, |_, o| {
        let b = stats_mut(o)[5];
        stats_mut(o).push(b);
    });
    assert_only(&v, Status::Fail, STATS_BAND_MISMATCH);
}

#[test]
fn a_malformed_docalc_is_a_band_mismatch() {
    // P2 docalc_true: the solver skips a band whose docalc is not exactly "1", so its column and
    // its files are absent although config.xml asks for it.
    let text = tutorial1_config(SolverKind::Spps).replacen(
        r#"freq="1000" docalc="1""#,
        r#"freq="1000" docalc="true""#,
        1,
    );
    let exp = Expectation::from_config(&text, SolverKind::Spps)
        .unwrap()
        .with_scene(&tutorial1_scene());
    let mut out = good_outputs(&exp);
    stats_mut(&mut out).retain(|b| b.freq_hz != 1000);
    out.files.retain(|p, _| !p.contains("/1000 Hz/"));
    let v = verdict_of(
        SolverKind::Spps,
        &exited(0),
        &clean_spps_lines(),
        &exp,
        &out,
        DEFAULT_LOSS_LIMIT,
    );
    assert_eq!(v.codes(), [STATS_BAND_MISMATCH, EXPECTED_FILE_MISSING]);
}

#[test]
fn particle_total_short() {
    let v = broken(SolverKind::Spps, |_, o| {
        let b = &mut stats_mut(o)[0];
        b.total -= 1;
        b.remaining -= 1;
    });
    assert_only(&v, Status::Fail, PARTICLE_TOTAL_SHORT);
    // A source that emitted nothing in a band (P2 dir_partial): total 0.
    let v = broken(SolverKind::Spps, |_, o| {
        let b = &mut stats_mut(o)[7];
        *b = BandStats {
            freq_hz: b.freq_hz,
            absorbed_by_atmosphere: 0,
            absorbed_by_materials: 0,
            absorbed_by_fittings: 0,
            lost_by_infinite_loops: 0,
            lost_by_meshing_problems: 0,
            remaining: 0,
            total: 0,
        };
    });
    assert_only(&v, Status::Fail, PARTICLE_TOTAL_SHORT);
    // More than nbparticules x sources (energetic transmission copies) is not short.
    assert!(
        broken(SolverKind::Spps, |_, o| {
            let b = &mut stats_mut(o)[0];
            b.total += 7;
            b.remaining += 7;
        })
        .is_ok()
    );
}

fn with_loss(loops: u32, meshing: u32, limit: f64) -> Verdict {
    let exp = tutorial1_expectation(SolverKind::Spps);
    let mut out = good_outputs(&exp);
    let b = &mut stats_mut(&mut out)[2];
    b.lost_by_infinite_loops = loops;
    b.lost_by_meshing_problems = meshing;
    b.remaining -= loops + meshing;
    verdict_of(
        SolverKind::Spps,
        &exited(0),
        &clean_spps_lines(),
        &exp,
        &out,
        limit,
    )
}

#[test]
fn particle_loss_excess() {
    // 10,000 particles per band; the limit is 1 %.
    assert_only(
        &with_loss(1, 100, DEFAULT_LOSS_LIMIT),
        Status::Fail,
        PARTICLE_LOSS_EXCESS,
    );
    assert_only(
        &with_loss(101, 0, DEFAULT_LOSS_LIMIT),
        Status::Fail,
        PARTICLE_LOSS_EXCESS,
    );
    // Exactly at the limit passes; the limit is a parameter; a NaN limit fails closed.
    assert!(with_loss(50, 50, DEFAULT_LOSS_LIMIT).is_ok());
    assert!(with_loss(50, 51, 0.02).is_ok());
    assert_only(&with_loss(0, 1, 0.0), Status::Fail, PARTICLE_LOSS_EXCESS);
    assert_only(
        &with_loss(0, 0, f64::NAN),
        Status::Fail,
        PARTICLE_LOSS_EXCESS,
    );
}

#[test]
fn only_loops_and_meshing_count_as_loss() {
    // Every particle absorbed by the atmosphere or the fittings is not a loss.
    let v = broken(SolverKind::Spps, |_, o| {
        let b = &mut stats_mut(o)[4];
        b.absorbed_by_atmosphere = b.remaining;
        b.remaining = 0;
    });
    assert!(v.is_ok(), "{v:#?}");
}

// Signal 4: the files.

#[test]
fn expected_file_missing() {
    for solver in [SolverKind::Spps, SolverKind::Tcr] {
        let v = broken(solver, |e, o| {
            let last = e.expected_files().pop().unwrap();
            o.files.remove(&last);
        });
        assert_only(&v, Status::Fail, EXPECTED_FILE_MISSING);
        assert!(v.reasons[0].detail.starts_with("1 of "), "{v:#?}");
        // Present but empty.
        let v = broken(solver, |e, o| {
            o.files.insert(e.expected_files()[1].clone(), 0);
        });
        assert_only(&v, Status::Fail, EXPECTED_FILE_MISSING);
        assert!(v.reasons[0].detail.contains("(empty)"), "{v:#?}");
    }
}

#[test]
fn nonfinite_result() {
    for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let v = broken(SolverKind::Tcr, |_, o| {
            let t = o.tables.get_mut("Main results.gabe").unwrap();
            let g = t.as_mut().unwrap();
            if let simpa_core::formats::gabe::ColumnData::Float { values, .. } =
                &mut g.columns[2].data
            {
                values[4] = value;
            }
        });
        assert_only(&v, Status::Fail, NONFINITE_RESULT);
    }
    // In a point receiver's table as well.
    let v = broken(SolverKind::Tcr, |_, o| {
        let g = o
            .tables
            .get_mut("Punctual receivers/Receiver 1.gabe")
            .unwrap()
            .as_mut()
            .unwrap();
        if let simpa_core::formats::gabe::ColumnData::Float { values, .. } = &mut g.columns[1].data
        {
            values[0] = f32::NEG_INFINITY;
        }
    });
    assert_only(&v, Status::Fail, NONFINITE_RESULT);
}

/// Receiver 1's table with its first data column labelled `Direct`, as TCR labels it, and
/// `value` in the 125 Hz row.
fn direct_level(label: &[u8], value: f32) -> Verdict {
    broken(SolverKind::Tcr, |_, o| {
        let g = o
            .tables
            .get_mut("Punctual receivers/Receiver 1.gabe")
            .unwrap()
            .as_mut()
            .unwrap();
        g.columns[1].label = label.to_vec();
        if let simpa_core::formats::gabe::ColumnData::Float { values, .. } = &mut g.columns[1].data
        {
            values[4] = value;
        }
    })
}

#[test]
fn an_occluded_receivers_direct_level_is_minus_infinity_by_design() {
    // TCR adds no direct energy from a source it cannot see, then takes 10 log10 of 0.
    assert!(direct_level(b"Direct\rdB SPL", f32::NEG_INFINITY).is_ok());
    // Only -inf, and only in that column.
    assert_only(
        &direct_level(b"Direct\rdB SPL", f32::NAN),
        Status::Fail,
        NONFINITE_RESULT,
    );
    assert_only(
        &direct_level(b"Direct\rdB SPL", f32::INFINITY),
        Status::Fail,
        NONFINITE_RESULT,
    );
    assert_only(
        &direct_level(b"Total (Sabine)\rdB SPL", f32::NEG_INFINITY),
        Status::Fail,
        NONFINITE_RESULT,
    );
}

#[test]
fn the_global_row_and_uncomputed_bands_are_not_displayed() {
    // good_outputs already has NaN in every Global row. A band that is not computed may hold
    // anything.
    let text = tutorial1_config(SolverKind::Tcr).replacen(
        r#"freq="1000" docalc="1""#,
        r#"freq="1000" docalc="0""#,
        1,
    );
    let exp = Expectation::from_config(&text, SolverKind::Tcr)
        .unwrap()
        .with_scene(&tutorial1_scene());
    let mut out = good_outputs(&exp);
    let row = exp.bands.iter().position(|b| b.freq_hz == 1000).unwrap();
    for t in out.tables.values_mut() {
        if let simpa_core::formats::gabe::ColumnData::Float { values, .. } =
            &mut t.as_mut().unwrap().columns[1].data
        {
            values[row] = f32::NAN;
        }
    }
    let v = verdict_of(
        SolverKind::Tcr,
        &exited(0),
        &clean_tcr_lines(),
        &exp,
        &out,
        DEFAULT_LOSS_LIMIT,
    );
    assert!(v.is_ok(), "{v:#?}");
}

#[test]
fn result_unreadable() {
    let v = broken(SolverKind::Tcr, |_, o| {
        o.tables.insert(
            "Main results.gabe".into(),
            Err("truncated gabe: data ends at byte 20".into()),
        );
    });
    assert_only(&v, Status::Fail, RESULT_UNREADABLE);
    // A table present on disk but never decoded fails closed.
    let v = broken(SolverKind::Tcr, |_, o| {
        o.tables.clear();
    });
    assert_only(&v, Status::Fail, RESULT_UNREADABLE);
    // A table without a requested band's row.
    let v = broken(SolverKind::Tcr, |_, o| {
        let g = o
            .tables
            .get_mut("Main results.gabe")
            .unwrap()
            .as_mut()
            .unwrap();
        if let simpa_core::formats::gabe::ColumnData::ShortString(rows) = &mut g.columns[0].data {
            rows[3] = b"999 Hz".to_vec();
        }
    });
    assert_only(&v, Status::Fail, RESULT_UNREADABLE);
}

#[test]
fn every_verdict_code_is_exercised_above() {
    // The tests above cover every code of `codes::ALL`; this keeps the list and the tests in
    // step when a code is added.
    assert_eq!(
        ALL,
        [
            CANCELLED,
            CRASH_ACCESS_VIOLATION,
            CRASH_ABORT,
            CRASH_OTHER,
            EXIT_NONZERO,
            END_OF_CALCULATION_MISSING,
            CONFIG_ATTRIBUTE_MISSING,
            STATS_UNREADABLE,
            STATS_BAND_MISMATCH,
            PARTICLE_TOTAL_SHORT,
            PARTICLE_LOSS_EXCESS,
            EXPECTED_FILE_MISSING,
            NONFINITE_RESULT,
            RESULT_UNREADABLE,
        ]
    );
}

#[test]
fn the_verdict_round_trips_through_json() {
    let v = broken(SolverKind::Spps, |_, o| o.stats = None);
    let json = serde_json::to_string(&v).unwrap();
    assert!(json.contains(r#""status":"FAIL""#), "{json}");
    assert!(json.contains(r#""code":"stats_unreadable""#), "{json}");
    let back: Verdict = serde_json::from_str(&json).unwrap();
    assert_eq!(back, v);
    assert!(
        serde_json::from_str::<Verdict>(r#"{"status":"OK","reasons":[],"warnings":[],"extra":1}"#)
            .is_err()
    );
}
