//! SPPS particle statistics, typed from the two committed statistics tables, and refused when a
//! table is not laid out as `SaveThreadsStats` writes it.

#[path = "run_support.rs"]
mod support;

use simpa_core::formats::gabe::{self, Column, ColumnData, Gabe};
use simpa_core::run::StatsError;
use simpa_core::run::stats::{self, ROW_LABELS};
use support::*;

#[test]
fn tutorial1_has_27_bands_of_10000() {
    let st = stats::read_file(&fixture("solver-outputs/tutorial1/spps_stats.gabe")).unwrap();
    assert_eq!(st.bands.len(), 27);
    let freqs = st.freqs();
    assert_eq!(freqs.first(), Some(&50));
    assert_eq!(freqs.last(), Some(&20000));
    assert!(freqs.windows(2).all(|w| w[0] < w[1]), "{freqs:?}");
    assert!(st.bands.iter().all(|b| b.total == 10_000));
    for b in &st.bands {
        let sum = b.absorbed_by_atmosphere
            + b.absorbed_by_materials
            + b.absorbed_by_fittings
            + b.lost_by_infinite_loops
            + b.lost_by_meshing_problems
            + b.remaining;
        assert_eq!(sum, b.total, "{b:?}");
    }
}

#[test]
fn nightmode_losses_are_in_upstreams_row_order() {
    let st = stats::read_file(&fixture(
        "solver-outputs/nightmode-2026-09-08/spps_stats.gabe",
    ))
    .unwrap();
    assert_eq!(st.freqs(), [125, 250, 500, 1000, 2000, 4000]);
    let meshing: Vec<u32> = st
        .bands
        .iter()
        .map(|b| b.lost_by_meshing_problems)
        .collect();
    assert_eq!(meshing, [38, 54, 30, 42, 50, 48]);
    assert!(st.bands.iter().all(|b| b.total == 100_000));
    // Row 3 (loops) and row 4 (meshing) both count as lost; the worst band is 0.056 %.
    let worst = st
        .bands
        .iter()
        .filter_map(|b| b.loss_ratio())
        .fold(0.0, f64::max);
    assert!(worst > 0.0005 && worst < 0.0006, "{worst}");
    println!(
        "nightmode 2026-09-08: lost per band {:?} of 100,000",
        st.bands.iter().map(|b| b.lost()).collect::<Vec<_>>()
    );
}

fn tutorial1_table() -> Gabe {
    gabe::read_file(&fixture("solver-outputs/tutorial1/spps_stats.gabe")).unwrap()
}

fn layout_error(g: &Gabe) -> String {
    match stats::from_gabe(g) {
        Err(StatsError::Layout(m)) => m,
        other => panic!("expected a layout error, got {other:?}"),
    }
}

#[test]
fn a_table_not_laid_out_as_upstreams_is_refused() {
    // Row labels swapped: loops and meshing would be read the wrong way round.
    let mut g = tutorial1_table();
    if let ColumnData::ShortString(rows) = &mut g.columns[0].data {
        rows.swap(3, 4);
    }
    assert!(layout_error(&g).contains("row labels"));

    // A band column that is float, one with 6 rows, one mislabelled, one negative count.
    let mut g = tutorial1_table();
    g.columns[1].data = ColumnData::Float {
        num_of_digits: 0,
        values: vec![0.0; 7],
    };
    assert!(layout_error(&g).contains("not an integer column"));

    let mut g = tutorial1_table();
    g.columns[2].data = ColumnData::Int(vec![0; 6]);
    assert!(layout_error(&g).contains("6 rows"));

    let mut g = tutorial1_table();
    g.columns[3].label = b"125Hz".to_vec();
    assert!(layout_error(&g).contains("not '<f> Hz'"));

    let mut g = tutorial1_table();
    if let ColumnData::Int(v) = &mut g.columns[4].data {
        v[6] = -1;
    }
    assert!(layout_error(&g).contains("negative count"));

    // No label column at all.
    let g = Gabe {
        version: 2,
        read_only: true,
        columns: vec![Column {
            label: b"125 Hz".to_vec(),
            data: ColumnData::Int(vec![0; 7]),
        }],
    };
    assert!(layout_error(&g).contains("first column"));
}

#[test]
fn a_label_column_alone_is_a_table_with_no_bands() {
    let g = Gabe {
        version: 2,
        read_only: true,
        columns: vec![Column {
            label: vec![],
            data: ColumnData::ShortString(
                ROW_LABELS.iter().map(|l| l.as_bytes().to_vec()).collect(),
            ),
        }],
    };
    assert!(stats::from_gabe(&g).unwrap().bands.is_empty());
}

#[test]
fn a_missing_or_truncated_file_is_a_format_error() {
    let dir = fresh_dir("stats");
    let missing = stats::read_file(&dir.join("nope.gabe"));
    assert!(
        matches!(missing, Err(StatsError::Format(ref e)) if e.kind() == "notfound"),
        "{missing:?}"
    );
    let bytes = std::fs::read(fixture("solver-outputs/tutorial1/spps_stats.gabe")).unwrap();
    let cut = dir.join("cut.gabe");
    std::fs::write(&cut, &bytes[..bytes.len() / 2]).unwrap();
    let truncated = stats::read_file(&cut);
    assert!(
        matches!(truncated, Err(StatsError::Format(ref e)) if e.kind() == "truncated"),
        "{truncated:?}"
    );
}
