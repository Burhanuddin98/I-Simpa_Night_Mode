//! The bed's decay files: `decays/<cell>.csv`, one per cell, long form, never one file per row
//! (`docs/investigations/2026-09-29-m8a/SPEC.md`, section 6). Columns `source,seed,receiver,
//! freq_hz,u_s,level_db`, `u_s` from each curve's own arrival:
//! - `spps`: SPPS's decay curve (`decay_curve.points`, thinned within 0.01 dB) per seed, receiver
//!   and band;
//! - `spps_fit`: each T30's fitted line at the ends of its range, drawn through the curve's
//!   −5 dB point with the slope `−60/T30` (two rows);
//! - `transport`, `transport_fit`: the same of the transport's receiver Schroeder curves (the
//!   energy of all its replicas), seed 0, with the receiver's T30 over the replicas.

use std::io::Write;
use std::path::Path;

use super::check::{Reads, bed_air};
use super::file::{BedFile, Cell};
use super::read::{Curve, SppsRead};
use super::transport::Transports;

/// The `u` at which `points` first reach `level` dB, by straight lines between them.
fn crossing(points: &[[f64; 2]], level: f64) -> Option<f64> {
    points.windows(2).find_map(|w| {
        let ([u0, l0], [u1, l1]) = (w[0], w[1]);
        (l0 >= level && l1 <= level && l0 != l1).then(|| u0 + (u1 - u0) * (l0 - level) / (l0 - l1))
    })
}

fn rows_of(
    out: &mut impl Write,
    source: &str,
    seed: u32,
    receiver: usize,
    freq_hz: i32,
    curve: &Curve,
    t30: Option<f64>,
) -> std::io::Result<()> {
    for [u, l] in &curve.points {
        writeln!(out, "{source},{seed},{receiver},{freq_hz},{u},{l}")?;
    }
    if let (Some(t), Some(u5)) = (t30, crossing(&curve.points, -5.0)) {
        writeln!(out, "{source}_fit,{seed},{receiver},{freq_hz},{u5},-5")?;
        writeln!(
            out,
            "{source}_fit,{seed},{receiver},{freq_hz},{},-35",
            u5 + t / 2.0
        )?;
    }
    Ok(())
}

fn spps_rows(out: &mut impl Write, seed: u32, r: &SppsRead) -> std::io::Result<()> {
    for (ri, rec) in r.curves.iter().enumerate() {
        for (bi, c) in rec.iter().enumerate() {
            if let Some(c) = c {
                let t = r.t30.get(ri).and_then(|x| x.get(bi)).and_then(|t| t.t);
                rows_of(out, "spps", seed, ri, r.bands_hz[bi], c, t)?;
            }
        }
    }
    Ok(())
}

const HEADER: &str = "source,seed,receiver,freq_hz,u_s,level_db";

/// Writes `dir/<cell>.csv` for `cell`: its seeds' curves (and its extension's), and its
/// transports'. Returns the file's path.
pub fn write_cell(
    dir: &Path,
    bed: &BedFile,
    cell: &Cell,
    reads: &Reads,
    transports: &Transports,
) -> std::io::Result<std::path::PathBuf> {
    let path = dir.join(format!("{}.csv", cell.id));
    let mut out = std::io::BufWriter::new(std::fs::File::create(&path)?);
    writeln!(out, "{HEADER}")?;
    if let Some(seeds) = reads.spps.get(&cell.id) {
        for (seed, r) in seeds {
            if let Ok(r) = r {
                spps_rows(&mut out, *seed, r)?;
            }
        }
    }
    for &f in bed.bands_hz(cell.air) {
        let air = bed_air(bed, cell.air, f64::from(f));
        if let Some(Ok(t)) = transports.get(&cell.room, cell.alpha, air) {
            for (ri, (from_s, points)) in t.curves.iter().enumerate() {
                let c = Curve {
                    from_s: *from_s,
                    points: points.clone(),
                };
                let t30 = t.receivers.get(ri).map(|x| x[0]);
                rows_of(&mut out, "transport", 0, ri, f as i32, &c, t30)?;
            }
        }
    }
    out.flush()?;
    Ok(path)
}

/// Writes `dir/atmospheric-validation.csv`: upstream's validation runs' curves.
pub fn write_atmospheric(dir: &Path, reads: &Reads) -> std::io::Result<Option<std::path::PathBuf>> {
    if reads.atmospheric.spps.is_empty() {
        return Ok(None);
    }
    let path = dir.join("atmospheric-validation.csv");
    let mut out = std::io::BufWriter::new(std::fs::File::create(&path)?);
    writeln!(out, "{HEADER}")?;
    for (seed, r) in &reads.atmospheric.spps {
        if let Ok(r) = r {
            spps_rows(&mut out, *seed, r)?;
        }
    }
    out.flush()?;
    Ok(Some(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_curve_is_crossed_by_straight_lines() {
        let p = [[0.0, 0.0], [0.0, -1.0], [1.0, -11.0], [2.0, -21.0]];
        assert_eq!(crossing(&p, -5.0), Some(0.4));
        assert_eq!(crossing(&p, -21.0), Some(2.0));
        assert_eq!(crossing(&p, -30.0), None);
        let mut v = Vec::new();
        rows_of(
            &mut v,
            "spps",
            3,
            1,
            500,
            &Curve {
                from_s: 0.01,
                points: p.to_vec(),
            },
            Some(1.2),
        )
        .unwrap();
        let text = String::from_utf8(v).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 6);
        assert_eq!(lines[4], "spps_fit,3,1,500,0.4,-5");
        assert_eq!(lines[5], "spps_fit,3,1,500,1,-35");
    }
}
