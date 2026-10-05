//! The advisor's rules, each with the case that must say no (PLAN.md, "Gates").

use std::path::{Path, PathBuf};

use super::after::{Ctx, RunSettings, advise_value};
use super::*;
use crate::params::noise::RangeStatus;
use crate::params::{NotEvaluable, ParamError, ParticleCount, Quantity, edt};
use crate::results::report::{Evaluated, Refused};
use crate::schema::{self, ComputationMethod, F64, Op};

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(rel)
}

fn load(rel: &str) -> Project {
    schema::load(&fixture(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

/// The Elmia fixture with `elmia_corrected()`'s edits undone (geometry_import_proj.rs): tutorial 2
/// as upstream ships it, `-q2` without `-Y`, 0.31 m receivers, echogram per source off, 1.5 s at
/// 5 ms, extinction 5, a million particles.
fn tutorial2_as_shipped() -> Project {
    let mut p = load("rooms/elmia_corrected.simpa");
    p.solvers.meshing.min_radius_edge_ratio = F64::new(2.0);
    p.solvers.meshing.preserve_boundary = false;
    p.solvers.spps.receiver_radius_m = F64::new(0.31);
    p.solvers.spps.echogram_per_source = false;
    p.solvers.spps.duration_s = F64::new(1.5);
    p.solvers.spps.time_step_s = F64::new(0.005);
    p.solvers.spps.extinction_exponent = F64::new(5.0);
    assert_eq!(p.solvers.spps.particles_per_source, 1_000_000);
    p
}

fn codes_of(a: &[Advice]) -> Vec<&str> {
    a.iter().map(|x| x.code.as_str()).collect()
}

fn find<'a>(a: &'a [Advice], code: &str) -> &'a Advice {
    a.iter()
        .find(|x| x.code == code)
        .unwrap_or_else(|| panic!("no {code} in {:?}", codes_of(a)))
}

fn num(v: Option<SettingValue>) -> f64 {
    match v {
        Some(SettingValue::Number(x)) => x,
        other => panic!("not a number: {other:?}"),
    }
}

/// The scanner trap (PLAN.md, "Traps"; m10-h / m11-h): no digit in any of the advisor's words.
fn no_digits(a: &Advice) {
    for t in [
        Some(&a.cause),
        Some(&a.fix.words),
        a.fix.why_no_apply.as_ref(),
        a.fix.note.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        assert!(
            !t.chars().any(|c| c.is_ascii_digit()),
            "{}: a digit in {t:?}",
            a.code
        );
    }
}

// ---- helpers ------------------------------------------------------------------------------------

#[test]
fn codes_are_unique_and_snake_case() {
    let mut seen = std::collections::HashSet::new();
    for c in CODES {
        assert!(seen.insert(c), "{c} twice");
        assert!(!c.is_empty() && c.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'));
    }
}

#[test]
fn a_radius_is_strictly_below_the_clearance_on_the_grid() {
    assert_eq!(radius_below(0.647), Some(0.6));
    assert_eq!(radius_below(0.65), Some(0.6));
    assert_eq!(radius_below(0.651), Some(0.65));
    assert_eq!(radius_below(0.05), None);
    assert_eq!(radius_below(f64::NAN), None);
}

#[test]
fn a_longer_duration_stays_below_the_step_counter() {
    assert_eq!(longer_duration(1.5, 0.005, None), Some(10.0));
    // At 1 ms the step limit allows 65.535 s: past 10 s it is proposed.
    let cap = longer_duration(10.0, 0.001, None).unwrap();
    assert!(step_count(cap, 0.001) < 65_536.0 && cap > 65.0, "{cap}");
    // A decay longer than 10 s skips 10 s.
    assert_eq!(longer_duration(2.0, 0.001, Some(12.0)), Some(cap));
    // Says no: already at the limit.
    assert_eq!(longer_duration(cap, 0.001, None), None);
}

// ---- before a run -------------------------------------------------------------------------------

#[test]
fn tutorial2_as_shipped_gets_both_warnings_before_its_run() {
    let a = before(&tutorial2_as_shipped());
    let mesh = find(&a, code::MESH_SPLITS_WALLS);
    assert!(
        mesh.cause
            .starts_with("This meshing splits the walls; expect lost particles")
    );
    assert_eq!(mesh.fix.setting, Some(Setting::PreserveBoundary));
    assert_eq!(mesh.fix.to, Some(SettingValue::Bool(true)));
    let small = find(&a, code::RECEIVERS_SMALL);
    assert!(small.cause.contains("receivers are small"));
    assert_eq!(small.fix.setting, Some(Setting::ReceiverRadius));
    assert_eq!(num(small.fix.from), 0.31);
    // R03 is 0.647 m from a face: 0.6, the fixture's value (decision 49).
    assert_eq!(num(small.fix.to), 0.6);
    assert_eq!(small.fix.bound, Some(RadiusBound::Clearance));
    // Elmia's 10,389 m³ is outside the noise model's rooms: the fix says it is not checked there.
    assert!(small.fix.note.is_some());
    for x in &a {
        no_digits(x);
    }
}

#[test]
fn b1_mesh_splits_walls_is_absent_with_y_and_names_the_conflict_with_a_refinement() {
    // Says no: the Elmia fixture meshes with -Y.
    let p = load("rooms/elmia_corrected.simpa");
    assert!(!codes_of(&before(&p)).contains(&code::MESH_SPLITS_WALLS));
    // Q3: tutorial 1's box has -Y off and a surface-receiver refinement: no Apply, the conflict
    // named.
    let b = load("rooms/tutorial1_box.simpa");
    assert!(b.solvers.meshing.surface_receiver_max_area_m2.is_some());
    let a = before(&b);
    let m = find(&a, code::MESH_SPLITS_WALLS);
    assert!(!m.fix.applies());
    assert!(m.fix.why_no_apply.as_ref().unwrap().contains("refined"));
}

#[test]
fn b2_receivers_small_is_absent_at_or_above_k() {
    // Arm E: 150,000 particles at 1.0 m, N r^2 / V = 14.4: absent. (The radius crosses walls, a
    // validator warning, not this rule's.)
    let mut p = load("rooms/elmia_corrected.simpa");
    p.solvers.spps.particles_per_source = 150_000;
    p.solvers.spps.receiver_radius_m = F64::new(1.0);
    assert!(!codes_of(&before(&p)).contains(&code::RECEIVERS_SMALL));
    // The Elmia fixture: a million at 0.6 m, 34.7 (arm H): absent.
    assert!(
        !codes_of(&before(&load("rooms/elmia_corrected.simpa"))).contains(&code::RECEIVERS_SMALL)
    );
    // Arm B: 150,000 at 0.31 m, 1.39: present.
    p.solvers.spps.receiver_radius_m = F64::new(0.31);
    assert!(codes_of(&before(&p)).contains(&code::RECEIVERS_SMALL));
}

#[test]
fn b2_never_proposes_a_radius_past_a_wall_and_names_particles_when_the_cap_is_reached() {
    let p = tutorial2_as_shipped();
    let clearance = super::before::min_clearance(&p).unwrap();
    assert!((clearance - 0.647).abs() < 0.001, "{clearance}");
    let a = before(&p);
    let to = num(find(&a, code::RECEIVERS_SMALL).fix.to);
    assert!(to < clearance && to <= RADIUS_MAX_M);
    // The cap is the radius: 100,000 particles at 0.6 m (3.47 < K). More particles, no count.
    let mut q = load("rooms/elmia_corrected.simpa");
    q.solvers.spps.particles_per_source = 100_000;
    let a = before(&q);
    let s = find(&a, code::RECEIVERS_SMALL);
    assert_eq!(s.fix.setting, Some(Setting::ParticlesPerSource));
    assert!(!s.fix.applies(), "no count is named before a run");
}

#[test]
fn b3_run_short_is_absent_at_the_decay_and_when_sabine_is_refused() {
    // Tutorial 2 runs 1.5 s; Elmia's slowest band decays over about 2 s by Sabine.
    let a = before(&tutorial2_as_shipped());
    let s = find(&a, code::RUN_SHORT);
    assert_eq!(s.fix.setting, Some(Setting::Duration));
    assert_eq!(num(s.fix.from), 1.5);
    assert_eq!(num(s.fix.to), 10.0);
    // Says no: 10 s.
    assert!(!codes_of(&before(&load("rooms/elmia_corrected.simpa"))).contains(&code::RUN_SHORT));
    // Says no, not guessed: every material absorbs nothing, Sabine is refused.
    let mut p = tutorial2_as_shipped();
    for m in &mut p.materials {
        for a in &mut m.absorption {
            *a = F64::new(0.0);
        }
    }
    assert!(!codes_of(&before(&p)).contains(&code::RUN_SHORT));
}

#[test]
fn b4_particles_few_names_the_calibration_minimum() {
    let mut p = load("ui/teaching_room.simpa");
    assert_eq!(p.solvers.spps.method, ComputationMethod::Random);
    // Says no: 150,000 in random mode (its minimum 50,000).
    assert!(!codes_of(&before(&p)).contains(&code::PARTICLES_FEW));
    p.solvers.spps.particles_per_source = 10_000;
    let a = before(&p);
    assert_eq!(num(find(&a, code::PARTICLES_FEW).fix.to), 50_000.0);
    p.solvers.spps.method = ComputationMethod::Energetic;
    p.solvers.spps.particles_per_source = 100_000;
    let a = before(&p);
    assert_eq!(num(find(&a, code::PARTICLES_FEW).fix.to), 150_000.0);
    // Says no: energetic at 150,000.
    p.solvers.spps.particles_per_source = 150_000;
    assert!(!codes_of(&before(&p)).contains(&code::PARTICLES_FEW));
}

#[test]
fn the_elmia_fixture_and_the_teaching_room_get_no_mesh_or_receiver_warning() {
    for rel in ["rooms/elmia_corrected.simpa", "ui/teaching_room.simpa"] {
        let a = before(&load(rel));
        for c in [code::MESH_SPLITS_WALLS, code::RECEIVERS_SMALL] {
            assert!(!codes_of(&a).contains(&c), "{rel}: {c}");
        }
    }
}

// ---- Apply --------------------------------------------------------------------------------------

#[test]
fn apply_changes_one_field_and_is_refused_once_the_project_changed() {
    let p = tutorial2_as_shipped();
    let a = before(&p);
    let f = &find(&a, code::RECEIVERS_SMALL).fix;
    let op = apply_op(&p, f.setting.unwrap(), f.from.unwrap(), f.to.unwrap()).unwrap();
    let Op::SetSolverSettings { settings } = &op else {
        panic!("{op:?}")
    };
    let mut want = p.solvers.clone();
    want.spps.receiver_radius_m = F64::new(0.6);
    assert_eq!(settings, &want);
    // The op travels as text (PQ3 trap).
    assert_eq!(Op::from_json(&op.to_json()).unwrap(), op);
    // Says no: the radius is no longer the advice's `from`.
    let mut q = p.clone();
    q.solvers.spps.receiver_radius_m = F64::new(0.5);
    assert_eq!(
        apply_op(&q, f.setting.unwrap(), f.from.unwrap(), f.to.unwrap()),
        Err(ApplyRefusal::ProjectChanged {
            setting: Setting::ReceiverRadius
        })
    );
    // -Y.
    let m = &find(&a, code::MESH_SPLITS_WALLS).fix;
    let Op::SetSolverSettings { settings } =
        apply_op(&p, m.setting.unwrap(), m.from.unwrap(), m.to.unwrap()).unwrap()
    else {
        panic!()
    };
    assert!(settings.meshing.preserve_boundary);
}

// ---- after a run --------------------------------------------------------------------------------

fn settings() -> RunSettings {
    RunSettings {
        radius_m: 0.31,
        particles: 150_000.0,
        duration_s: 10.0,
        time_step_s: 0.001,
        trans_epsilon: 7.0,
        echogram_per_source: true,
    }
}

const KNOWN_Y: RunFacts = RunFacts {
    min_clearance_m: Some(0.647),
    meshing: Meshing::Known {
        preserve_boundary: true,
        refinement: false,
    },
};

fn ctx(facts: &RunFacts, s: RunSettings) -> Ctx<'_> {
    Ctx {
        settings: Some(s),
        facts,
        radius_cap: facts
            .min_clearance_m
            .and_then(radius_below)
            .map(|c| (c.min(RADIUS_MAX_M), RadiusBound::Clearance)),
        volume_m3: Some(10_389.0),
    }
}

fn refused(q: Quantity, why: NotEvaluable) -> Evaluated {
    let e = ParamError::NotEvaluable { quantity: q, why };
    Evaluated::NotEvaluable {
        not_evaluable: Refused {
            code: e.code().into(),
            message: e.to_string(),
            error: e,
        },
    }
}

fn shown(status: RangeStatus) -> Evaluated {
    Evaluated::Value {
        value: 2.0,
        mc_sd: Some(0.1),
        status: Some(status),
        lo: Some(1.8),
        hi: Some(2.2),
        refused_resamples: None,
        straddle: None,
        lost_share_warning: None,
    }
}

fn one(ctx: &Ctx, e: &Evaluated) -> (String, Fix) {
    let v = advise_value(ctx, e);
    assert_eq!(v.len(), 1, "{v:?}");
    let (c, _, f) = v.into_iter().next().unwrap();
    (c.to_string(), f)
}

#[test]
fn a1_an_ok_value_carries_no_advice() {
    let c = ctx(&KNOWN_Y, settings());
    assert!(advise_value(&c, &shown(RangeStatus::Ok)).is_empty());
}

#[test]
fn range_below_zero_names_monte_carlo_noise_and_the_radius_first() {
    let c = ctx(&KNOWN_Y, settings());
    let (code, fix) = one(
        &c,
        &refused(
            Quantity::T30,
            NotEvaluable::RangeBelowZero {
                value: 1.96,
                lo: -0.985,
                sd: Some(1.178),
            },
        ),
    );
    assert_eq!(code, code::RANGE_BELOW_ZERO);
    assert_eq!(fix.setting, Some(Setting::ReceiverRadius));
    assert_eq!((num(fix.from), num(fix.to)), (0.31, 0.6));
    assert!(
        fix.note.is_some(),
        "Elmia is outside the calibration's rooms"
    );
    // When the receivers cannot grow, particles; RangeBelowZero names no count: no Apply.
    let c = ctx(
        &KNOWN_Y,
        RunSettings {
            radius_m: 0.6,
            ..settings()
        },
    );
    let (_, fix) = one(
        &c,
        &refused(
            Quantity::T30,
            NotEvaluable::RangeBelowZero {
                value: 1.0,
                lo: -0.1,
                sd: Some(0.5),
            },
        ),
    );
    assert_eq!(fix.setting, Some(Setting::ParticlesPerSource));
    assert!(!fix.applies());
}

#[test]
fn monte_carlo_noise_takes_the_named_count_when_the_radius_is_capped() {
    let facts = RunFacts {
        min_clearance_m: None,
        ..KNOWN_Y
    };
    let c = ctx(&facts, settings());
    let named = |count| {
        refused(
            Quantity::T30,
            NotEvaluable::MonteCarloNoise {
                value: 2.0,
                sd: Some(0.2),
                limit: 0.025,
                resamples: 200,
                refused_resamples: 0,
                particle_count: count,
            },
        )
    };
    let (code, fix) = one(
        &c,
        &named(ParticleCount::Named {
            factor: 4.0,
            margin: 1.5,
            particles: Some(600_000),
        }),
    );
    assert_eq!(code, code::MONTE_CARLO_NOISE);
    assert_eq!(num(fix.to), 600_000.0);
    // Says no: the core names none.
    let (_, fix) = one(&c, &named(ParticleCount::ScalingNotConfirmed));
    assert!(!fix.applies());
    assert_eq!(fix.setting, Some(Setting::ParticlesPerSource));
}

#[test]
fn wide_and_straddle_name_the_radius_and_the_time_step() {
    let c = ctx(
        &KNOWN_Y,
        RunSettings {
            time_step_s: 0.005,
            ..settings()
        },
    );
    let (code, fix) = one(&c, &shown(RangeStatus::Wide));
    assert_eq!(code, code::WIDE);
    assert_eq!(fix.setting, Some(Setting::ReceiverRadius));
    let mut s = shown(RangeStatus::Wide);
    if let Evaluated::Value { straddle, .. } = &mut s {
        *straddle = Some([1.0, 2.0]);
    }
    let (code, fix) = one(&c, &s);
    assert_eq!(code, code::STRADDLE);
    assert_eq!((num(fix.from), num(fix.to)), (0.005, 0.001));
}

#[test]
fn lost_particles_name_y_only_when_the_run_was_meshed_without_it_and_never_more_particles() {
    let lost = refused(
        Quantity::T30,
        NotEvaluable::LostParticles {
            share: 0.015,
            limit: 0.01,
        },
    );
    // Meshed without -Y: -Y, applied.
    let no_y = RunFacts {
        meshing: Meshing::Known {
            preserve_boundary: false,
            refinement: false,
        },
        ..KNOWN_Y
    };
    let (code, fix) = one(&ctx(&no_y, settings()), &lost);
    assert_eq!(code, code::LOST_PARTICLES);
    assert_eq!(fix.to, Some(SettingValue::Bool(true)));
    // Says no: with -Y already, no setting; never particles.
    let (_, fix) = one(&ctx(&KNOWN_Y, settings()), &lost);
    assert_eq!(fix.setting, None);
    // Unknown meshing: named, no Apply.
    let unknown = RunFacts {
        meshing: Meshing::Unknown,
        ..KNOWN_Y
    };
    let (_, fix) = one(&ctx(&unknown, settings()), &lost);
    assert!(!fix.applies());
    // Decision 56's warning, 0.3 % to 1 %, on a shown value.
    let mut warned = shown(RangeStatus::Ok);
    if let Evaluated::Value {
        lost_share_warning, ..
    } = &mut warned
    {
        *lost_share_warning = Some(0.005);
    }
    let (code, fix) = one(&ctx(&no_y, settings()), &warned);
    assert_eq!(code, code::LOST_SHARE_WARNING);
    assert_eq!(fix.setting, Some(Setting::PreserveBoundary));
    assert_ne!(fix.setting, Some(Setting::ParticlesPerSource));
}

#[test]
fn every_refusal_kind_has_its_advice() {
    let c = ctx(
        &KNOWN_Y,
        RunSettings {
            duration_s: 1.5,
            time_step_s: 0.005,
            trans_epsilon: 5.0,
            echogram_per_source: false,
            ..settings()
        },
    );
    let t30 = |why| refused(Quantity::T30, why);
    let cases: Vec<(NotEvaluable, &str, Option<Setting>, bool)> = vec![
        (
            NotEvaluable::RangeNotReached {
                needed_db: -35.0,
                reached_db: -20.0,
            },
            code::RUN_TOO_SHORT,
            Some(Setting::Duration),
            true,
        ),
        (
            NotEvaluable::Truncated {
                value: 1.0,
                with_tail: None,
                limit: 0.025,
            },
            code::RUN_TOO_SHORT,
            Some(Setting::Duration),
            true,
        ),
        (
            NotEvaluable::Unresolved {
                value: 1.0,
                low: 0.9,
                high: 1.1,
                limit: 0.025,
            },
            code::ONSET_TOO_COARSE,
            Some(Setting::TimeStep),
            true,
        ),
        (
            NotEvaluable::EarlyUnresolved {
                value: 1.0,
                continued: 1.0,
                low: None,
                high: None,
                limit: 0.025,
            },
            code::ONSET_TOO_COARSE,
            Some(Setting::TimeStep),
            true,
        ),
        (
            NotEvaluable::RangeTooShort {
                span_s: 0.1,
                needed_s: 0.2,
            },
            code::DECAY_NOT_FITTED,
            None,
            false,
        ),
        (
            NotEvaluable::NotDecaying,
            code::DECAY_NOT_FITTED,
            None,
            false,
        ),
        (
            NotEvaluable::EmptyWindow { from_s: 0.05 },
            code::DECAY_NOT_FITTED,
            None,
            false,
        ),
        (
            NotEvaluable::MissingMoves {
                floor_db: Some(-50.0),
                lost_share: None,
                value: 1.0,
                with_missing: None,
                limit: 0.005,
            },
            code::SOLVER_FLOOR,
            Some(Setting::ExtinctionExponent),
            true,
        ),
        (
            NotEvaluable::MissingNotCleared {
                floor_db: None,
                lost_share: Some(0.001),
                needed_db: -35.0,
                reached_db: -30.0,
            },
            code::PARTICLES_LEFT_ALIVE,
            Some(Setting::Duration),
            true,
        ),
        (
            NotEvaluable::NoiseUnknown {
                value: 1.0,
                detail: "x".into(),
            },
            code::NOISE_UNKNOWN,
            None,
            false,
        ),
        (
            NotEvaluable::SeveralSources {
                sources: vec!["S1".into(), "S2".into()],
            },
            code::SEVERAL_SOURCES,
            Some(Setting::EchogramPerSource),
            true,
        ),
        (
            NotEvaluable::NoTimeSeries {
                detail: "TCR".into(),
            },
            code::NO_TIME_SERIES,
            None,
            false,
        ),
        (
            NotEvaluable::NoAWeight { freq_hz: 63 },
            code::BANDS_NOT_COVERED,
            None,
            false,
        ),
        (
            NotEvaluable::BandMissing {
                freq_hz: 125,
                needed_hz: vec![125],
            },
            code::BANDS_NOT_COVERED,
            None,
            false,
        ),
        (
            NotEvaluable::NotOctaveBands {
                bands_hz: vec![100],
            },
            code::BANDS_NOT_COVERED,
            None,
            false,
        ),
        (
            NotEvaluable::BandRefused {
                freq_hz: 125,
                detail: "x".into(),
            },
            code::BAND_REFUSED,
            None,
            false,
        ),
    ];
    for (why, code, setting, applies) in cases {
        let label = format!("{why:?}");
        let (got, fix) = one(&c, &t30(why));
        assert_eq!(got, code, "{label}");
        assert_eq!(fix.setting, setting, "{label}");
        assert_eq!(fix.applies(), applies, "{label}");
    }
    // A1: a refusal with no setting fix carries its cause and no Apply.
    let (_, f) = one(&c, &t30(NotEvaluable::NotDecaying));
    assert!(f.why_no_apply.is_some() && f.to.is_none());
    // Several sources with the echogram per source on: choose a source, no setting.
    let on = ctx(&KNOWN_Y, settings());
    let (_, f) = one(&on, &t30(NotEvaluable::SeveralSources { sources: vec![] }));
    assert_eq!(f.setting, None);
    // The floor at the new-project extinction: named, no Apply.
    let (_, f) = one(
        &on,
        &t30(NotEvaluable::MissingMoves {
            floor_db: Some(-70.0),
            lost_share: None,
            value: 1.0,
            with_missing: None,
            limit: 0.005,
        }),
    );
    assert!(!f.applies());
    // A time step already at 1 ms: named, no Apply.
    let (_, f) = one(
        &on,
        &t30(NotEvaluable::Unresolved {
            value: 1.0,
            low: 0.9,
            high: 1.1,
            limit: 0.025,
        }),
    );
    assert!(!f.applies());
}

#[test]
fn noise_uncalibrated_names_its_count_and_its_radius() {
    let c = ctx(&KNOWN_Y, settings());
    let v = advise_value(
        &c,
        &refused(
            Quantity::T30,
            NotEvaluable::NoiseUncalibrated {
                value: 1.0,
                particles: 100_000,
                crossings_per_particle: 2.0,
                min_particles: 150_000,
                max_crossings_per_particle: 1.0,
                particles_at_least: Some(150_000),
                receiver_radius_scale_at_most: Some(0.5),
            },
        ),
    );
    assert_eq!(v.len(), 2);
    assert_eq!(num(v[0].2.to), 150_000.0);
    assert_eq!(v[1].2.setting, Some(Setting::ReceiverRadius));
    assert_eq!(num(v[1].2.to), 0.15);
}

#[test]
fn every_edt_refusal_reason_has_its_own_advice() {
    let c = ctx(
        &KNOWN_Y,
        RunSettings {
            radius_m: 1.2,
            ..settings()
        },
    );
    let want = |r: &str| match r {
        "run_too_short" | "not_decaying_at_run_end" => code::RUN_TOO_SHORT,
        "step_too_coarse" => code::ONSET_TOO_COARSE,
        "too_few_particles" => code::EDT_TOO_FEW_PARTICLES,
        "receiver_too_large" => code::EDT_RECEIVER_TOO_LARGE,
        "no_energy" | "no_energy_after_arrival" | "direct_only" | "not_decaying" => {
            code::EDT_OUTSIDE_METHOD
        }
        other => panic!("EDT reason {other} has no advice: add it to after::edt_refused"),
    };
    for r in edt::REFUSAL_REASONS {
        let (code, _) = one(
            &c,
            &refused(Quantity::Edt, NotEvaluable::EdtRefused { reason: r.into() }),
        );
        assert_eq!(code, want(r), "{r}");
    }
    let (_, f) = one(
        &c,
        &refused(
            Quantity::Edt,
            NotEvaluable::EdtRefused {
                reason: "receiver_too_large".into(),
            },
        ),
    );
    // At most a metre, and below the walls: the clearance's 0.6.
    assert_eq!((num(f.from), num(f.to)), (1.2, 0.6));
}

#[test]
fn every_after_text_holds_no_digit() {
    let facts = [
        KNOWN_Y,
        RunFacts {
            meshing: Meshing::Unknown,
            min_clearance_m: None,
        },
        RunFacts {
            meshing: Meshing::Known {
                preserve_boundary: false,
                refinement: true,
            },
            ..KNOWN_Y
        },
    ];
    for f in &facts {
        let c = ctx(f, settings());
        for e in [
            refused(
                Quantity::T30,
                NotEvaluable::LostParticles {
                    share: 0.02,
                    limit: 0.01,
                },
            ),
            refused(
                Quantity::T30,
                NotEvaluable::RangeBelowZero {
                    value: 1.0,
                    lo: -1.0,
                    sd: None,
                },
            ),
            refused(Quantity::T30, NotEvaluable::NotDecaying),
            shown(RangeStatus::Wide),
        ] {
            for (code, cause, fix) in advise_value(&c, &e) {
                no_digits(&Advice {
                    code: code.into(),
                    cause,
                    fix,
                    values: vec![],
                });
            }
        }
    }
}

// ---- reports of real runs -----------------------------------------------------------------------

/// A2 on the committed runs: every advice path names a value of the report that is not `ok`, and
/// every value that is refused, `wide` or warned has an item.
#[test]
fn a2_every_advice_path_is_a_report_value_and_every_flagged_value_has_advice() {
    for rel in [
        "results/seats_spps",
        "results/sources2_spps",
        "results/energetic_spps",
        "results/seats_tcr",
    ] {
        let r = crate::results::load(&fixture(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"));
        let rep = crate::results::report::report(&r);
        let json = serde_json::to_value(&rep).unwrap();
        let at = |path: &str| {
            let ptr = format!("/{}", path.replace('.', "/"));
            json.pointer(&ptr).cloned()
        };
        let mut explained = std::collections::HashSet::new();
        for (i, a) in rep.advice.iter().enumerate() {
            assert!(CODES.contains(&a.code.as_str()), "{rel}: {}", a.code);
            no_digits(a);
            assert!(!a.values.is_empty(), "{rel}: advice {i} explains nothing");
            for p in &a.values {
                let v = at(p).unwrap_or_else(|| panic!("{rel}: {p} is not in the report"));
                assert!(
                    v.get("not_evaluable").is_some()
                        || v.get("status").and_then(|s| s.as_str()) == Some("wide")
                        || v.get("lost_share_warning").is_some(),
                    "{rel}: {p} is ok but carries advice {}",
                    a.code
                );
                explained.insert(p.clone());
            }
            if a.fix.applies() {
                assert!(json.pointer(&format!("/advice/{i}/fix/to")).is_some());
            }
        }
        for (p, e) in super::after::values(&rep) {
            let flagged = e.refusal().is_some()
                || e.status() == Some(RangeStatus::Wide)
                || matches!(
                    e,
                    Evaluated::Value {
                        lost_share_warning: Some(_),
                        ..
                    }
                );
            assert_eq!(flagged, explained.contains(&p), "{rel}: {p}");
        }
    }
}

#[test]
fn several_sources_summed_without_an_echogram_per_source_offers_it() {
    let r = crate::results::load(&fixture("results/sources2_spps")).unwrap();
    let rep = crate::results::report::report(&r);
    let s = rep.spps.as_ref().unwrap();
    if s.echogram_per_source {
        return;
    }
    let a = find(&rep.advice, code::SEVERAL_SOURCES);
    assert_eq!(a.fix.setting, Some(Setting::EchogramPerSource));
    assert_eq!(a.fix.to, Some(SettingValue::Bool(true)));
}

/// T3 on arm B's stored run (`B:\data\m12\b78-mesh\B_q5_Y`, backlog 78: Elmia S01, `-Y`,
/// 150,000 particles, 0.31 m), when the data is on this machine: its T30s refused
/// `range_below_zero` name the receiver radius, 0.31 to 0.6. Skipped elsewhere
/// (`SIMPA_ARM_B_RUN` names another copy).
#[test]
fn arm_b_range_below_zero_t30_names_a_radius_of_0_6() {
    let dir = std::env::var_os("SIMPA_ARM_B_RUN")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(r"B:\data\m12\b78-mesh\B_q5_Y\runs\20261004-081146-274-spps")
        });
    if !dir.join("run.json").exists() {
        eprintln!("skipped: no arm B run at {}", dir.display());
        return;
    }
    let r = crate::results::load(&dir).unwrap();
    let rep = crate::results::report::report(&r);
    let a = find(&rep.advice, code::RANGE_BELOW_ZERO);
    let t30 = a.values.iter().filter(|p| p.ends_with(".t30_s")).count();
    assert!(t30 >= 1, "{:?}", a.values);
    assert_eq!(a.fix.setting, Some(Setting::ReceiverRadius));
    // The run reports the radius as the solver read it, in f32.
    assert_eq!((num(a.fix.from) as f32, num(a.fix.to)), (0.31, 0.6));
    assert_eq!(a.fix.bound, Some(RadiusBound::Clearance));
    // Apply on the arm-B project: its 0.31 is the run's 0.31 in f32.
    let mut p = load("rooms/elmia_corrected.simpa");
    p.solvers.spps.receiver_radius_m = F64::new(0.31);
    assert!(
        apply_op(
            &p,
            Setting::ReceiverRadius,
            a.fix.from.unwrap(),
            a.fix.to.unwrap()
        )
        .is_ok()
    );
    eprintln!(
        "arm B: {} values range_below_zero ({t30} T30); advice codes {:?}",
        a.values.len(),
        codes_of(&rep.advice)
    );
}
