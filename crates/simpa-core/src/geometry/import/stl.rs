//! STL, ASCII and binary.
//!
//! **Binary** (80-byte header, `u32` triangle count, 50-byte records: normal, three vertices,
//! attribute count) is read little-endian, with the file opened as bytes and the three vertices
//! of each record read in order: upstream's reader opens the file in text mode and indexes the
//! triangles `(i, 3i+1, 3i+2)` (`3dengine/Core/stl.cpp:316-319`), which is wrong for every
//! triangle but the first. Normals and attribute bytes are ignored. One group, `model`.
//!
//! **ASCII** is read by its structure (`solid`, `facet normal`, `outer loop`, three `vertex`
//! lines, `endloop`, `endfacet`, `endsolid`); each `solid` becomes a surface group named after
//! it (`solid <n>` when it has no name).
//!
//! **Which one:** a file whose length is exactly `84 + 50 n`, where `n` is the count at byte 80,
//! is binary, unless it starts with `solid` and also parses as ASCII; otherwise a file that
//! starts with `solid` is ASCII. Anything else is refused. (Binary headers often start with
//! `solid` too, so that word alone decides nothing.)
//!
//! STL has no shared vertices: every facet carries its own three, and welding (exact, see the
//! module docs of `geometry::import`) rebuilds the connectivity.

use super::{ImportError, ImportOptions, ImportedModel, MeshFormat, RawMesh, Result};
use crate::config_xml::widen_f32;

const FMT: &str = "stl";

pub(super) fn read(bytes: &[u8], options: &ImportOptions) -> Result<ImportedModel> {
    let starts_solid = {
        let t = bytes.iter().position(|b| !b.is_ascii_whitespace());
        t.is_some_and(|i| bytes[i..].starts_with(b"solid"))
    };
    let binary_size = bytes.len() >= 84 && {
        let n = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as u64;
        84 + 50 * n == bytes.len() as u64
    };
    if binary_size {
        if starts_solid && let Ok(m) = ascii(bytes, options) {
            return Ok(m);
        }
        return binary(bytes, options);
    }
    if starts_solid {
        return ascii(bytes, options);
    }
    if bytes.len() >= 84 {
        let n = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as u64;
        return Err(ImportError::invalid(
            FMT,
            format!(
                "not ASCII (no leading `solid`), and not binary: {n} triangles need {} bytes, \
                 the file has {}",
                84 + 50 * n,
                bytes.len()
            ),
        ));
    }
    Err(ImportError::truncated(FMT, "the 84-byte binary header"))
}

fn binary(bytes: &[u8], options: &ImportOptions) -> Result<ImportedModel> {
    let n = u32::from_le_bytes([bytes[80], bytes[81], bytes[82], bytes[83]]) as usize;
    let mut vertices = Vec::with_capacity(3 * n);
    let mut polygons = Vec::with_capacity(n);
    for t in 0..n {
        let rec = &bytes[84 + 50 * t..84 + 50 * (t + 1)];
        let base = u32::try_from(vertices.len())
            .map_err(|_| ImportError::unsupported(FMT, "more than 4,294,967,295 vertices"))?;
        for v in 0..3 {
            let o = 12 + 12 * v;
            let f = |k: usize| {
                let b = &rec[o + 4 * k..o + 4 * k + 4];
                widen_f32(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            };
            vertices.push([f(0), f(1), f(2)]);
        }
        polygons.push((vec![base, base + 1, base + 2], 0));
    }
    RawMesh {
        vertices,
        polygons,
        groups: vec!["model".to_string()],
        notes: vec!["binary STL".to_string()],
    }
    .finish(MeshFormat::Stl, options, n)
}

fn ascii(bytes: &[u8], options: &ImportOptions) -> Result<ImportedModel> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| ImportError::syntax(FMT, "file", "an ASCII STL must be text"))?;
    let mut lines = text
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .filter(|(_, l)| !l.is_empty());
    let mut vertices: Vec<[f64; 3]> = Vec::new();
    let mut polygons = Vec::new();
    let mut groups: Vec<String> = Vec::new();
    let expect = |got: Option<(usize, &str)>, what: &str| -> Result<(usize, String)> {
        match got {
            Some((n, l)) if l.split_ascii_whitespace().next() == what.split(' ').next() => {
                let words: Vec<&str> = l.split_ascii_whitespace().collect();
                let want: Vec<&str> = what.split(' ').collect();
                if words.len() >= want.len() && words[..want.len()] == want[..] {
                    Ok((n, l.to_string()))
                } else {
                    Err(ImportError::syntax(
                        FMT,
                        format!("line {n}"),
                        format!("expected `{what}`"),
                    ))
                }
            }
            Some((n, _)) => Err(ImportError::syntax(
                FMT,
                format!("line {n}"),
                format!("expected `{what}`"),
            )),
            None => Err(ImportError::truncated(FMT, format!("`{what}`"))),
        }
    };
    while let Some((n, line)) = lines.next() {
        let (_, solid) = expect(Some((n, line)), "solid")?;
        let name = solid["solid".len()..].trim();
        let g = groups.len() as u32;
        groups.push(if name.is_empty() {
            format!("solid {}", groups.len() + 1)
        } else {
            name.to_string()
        });
        loop {
            let Some((n, line)) = lines.next() else {
                return Err(ImportError::truncated(FMT, "`endsolid`"));
            };
            if line.split_ascii_whitespace().next() == Some("endsolid") {
                break;
            }
            expect(Some((n, line)), "facet normal")?;
            expect(lines.next(), "outer loop")?;
            let base = u32::try_from(vertices.len())
                .map_err(|_| ImportError::unsupported(FMT, "more than 4,294,967,295 vertices"))?;
            for _ in 0..3 {
                let (n, l) = expect(lines.next(), "vertex")?;
                let mut p = [0.0f64; 3];
                let mut tok = l.split_ascii_whitespace().skip(1);
                for c in &mut p {
                    let t = tok.next().ok_or_else(|| {
                        ImportError::syntax(FMT, format!("line {n}"), "a vertex needs 3 values")
                    })?;
                    *c = t.parse().map_err(|_| {
                        ImportError::syntax(
                            FMT,
                            format!("line {n}"),
                            format!("`{t}` is not a number"),
                        )
                    })?;
                }
                vertices.push(p);
            }
            expect(lines.next(), "endloop")?;
            expect(lines.next(), "endfacet")?;
            polygons.push((vec![base, base + 1, base + 2], g));
        }
    }
    let source_faces = polygons.len();
    RawMesh {
        vertices,
        polygons,
        groups,
        notes: vec!["ASCII STL".to_string()],
    }
    .finish(MeshFormat::Stl, options, source_faces)
}
