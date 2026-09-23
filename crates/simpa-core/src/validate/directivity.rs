//! Directivity balloon files, read as `xhn_DirectivityParser::parse` reads them
//! (`lib_interface/input_output/directivity/directivityParser.cpp:38-112`), and checked so that
//! nothing in them is skipped silently or aborts a solver.
//!
//! The solver opens the file in text mode, so `\r\n` ends a line. It skips empty lines and lines
//! starting with `;`, and splits the rest on `,`:
//! - `"Frequency",<f>,<unit>` (exactly 3 fields) sets the current frequency to `stod(<f>)`;
//! - a row of exactly 38 fields, while the current frequency is above 0, is data: field 1 with
//!   every non-digit removed is φ, fields 2 to 38 are the values at θ = 0, 5, ..., 180 degrees;
//! - anything else is ignored.
//!
//! [`parse`] refuses, with the reason, every file in which the solver would skip a row, read a
//! frequency with rows missing, or throw: a value that `stod` cannot read aborts both solvers
//! (`0xC0000409`, VERIFIED P2 `dir_nan`), and a row with the wrong field count is skipped
//! (VERIFIED P2 `dir_badrow`). Values must be plain finite decimal numbers, which is stricter
//! than `stod` (it would read `1.5dB` as 1.5).

/// The φ angles every frequency block must have: 0 to 355 in 5-degree steps.
const PHI_ROWS: u32 = 72;

/// A parsed file: the frequencies that carry complete data.
#[derive(Debug)]
pub(super) struct Balloon {
    frequencies: Vec<f64>,
}

impl Balloon {
    /// True if the file has complete data for `hz`. The solver compares the band's integer
    /// frequency with the file's frequencies as doubles, exactly (`directivityBalloon.cpp:49-52`).
    pub(super) fn covers(&self, hz: u32) -> bool {
        self.frequencies.contains(&f64::from(hz))
    }
}

/// C `isspace` in the "C" locale.
fn is_c_space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

fn trim_c_space(mut s: &[u8]) -> &[u8] {
    while let [first, rest @ ..] = s
        && is_c_space(*first)
    {
        s = rest;
    }
    while let [rest @ .., last] = s
        && is_c_space(*last)
    {
        s = rest;
    }
    s
}

/// True if `std::stod` reads something from `field` (a number prefix after leading spaces),
/// false if it throws `std::invalid_argument`.
fn stod_reads(field: &[u8]) -> bool {
    let mut s = field;
    while let [first, rest @ ..] = s
        && is_c_space(*first)
    {
        s = rest;
    }
    if let [b'+' | b'-', rest @ ..] = s {
        s = rest;
    }
    let starts_ci =
        |word: &[u8]| s.len() >= word.len() && s[..word.len()].eq_ignore_ascii_case(word);
    match s {
        [d, ..] if d.is_ascii_digit() => true,
        [b'.', d, ..] if d.is_ascii_digit() => true,
        _ => starts_ci(b"inf") || starts_ci(b"nan"),
    }
}

/// A plain finite number, the whole field apart from surrounding spaces.
fn plain_number(field: &[u8]) -> Option<f64> {
    let text = std::str::from_utf8(trim_c_space(field)).ok()?;
    let ok = !text.is_empty()
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'+' | b'-' | b'.' | b'e' | b'E'));
    let value: f64 = if ok { text.parse().ok()? } else { return None };
    value.is_finite().then_some(value)
}

fn show(field: &[u8]) -> String {
    String::from_utf8_lossy(field).into_owned()
}

/// Why a value field is refused.
fn value_problem(field: &[u8]) -> Option<String> {
    if plain_number(field).is_some() {
        None
    } else if stod_reads(field) {
        Some(format!(
            "{:?} is not a plain finite number: the solver would read only a prefix of it, or \
             an infinite value",
            show(field)
        ))
    } else {
        Some(format!(
            "{:?} is not a number: std::stod throws and both solvers abort (0xC0000409)",
            show(field)
        ))
    }
}

/// Parses a directivity file. `Err` holds the first reason the file is refused.
pub(super) fn parse(bytes: &[u8]) -> Result<Balloon, String> {
    let mut current = 0.0f64;
    // (frequency, bit set of the φ rows seen, as φ / 5 for φ in 0..=355).
    let mut blocks: Vec<(f64, u128)> = Vec::new();
    let mut data_rows = 0usize;

    for (index, raw) in bytes.split(|&b| b == b'\n').enumerate() {
        let n = index + 1;
        let line = raw.strip_suffix(b"\r").unwrap_or(raw);
        if line.is_empty() || line[0] == b';' {
            continue;
        }
        let tokens: Vec<&[u8]> = line.split(|&b| b == b',').collect();
        if tokens.len() == 3 && tokens[0] == b"\"Frequency\"" {
            if let Some(problem) = value_problem(tokens[1]) {
                return Err(format!("line {n}: the frequency {problem}"));
            }
            current = plain_number(tokens[1]).unwrap_or(0.0);
            if current <= 0.0 {
                return Err(format!(
                    "line {n}: frequency {current} is not above 0, so the solver skips every row \
                     of its block"
                ));
            }
            continue;
        }
        let phi_like = tokens[0].iter().any(u8::is_ascii_digit);
        if tokens.len() != 38 {
            if phi_like && tokens.len() >= 2 {
                return Err(format!(
                    "line {n} looks like a data row but has {} comma-separated fields; a data row \
                     needs exactly 38 (φ and 37 values), so the solver skips it silently",
                    tokens.len()
                ));
            }
            continue;
        }
        if current <= 0.0 {
            if phi_like {
                return Err(format!(
                    "line {n}: a data row before any \"Frequency\" line; the solver skips it"
                ));
            }
            continue;
        }
        let digits: String = tokens[0]
            .iter()
            .filter(|b| b.is_ascii_digit())
            .map(|&b| char::from(b))
            .collect();
        if digits.is_empty() {
            return Err(format!(
                "line {n}: the angle field {:?} holds no digit: std::stod throws and both solvers \
                 abort (0xC0000409)",
                show(tokens[0])
            ));
        }
        let phi: f64 = digits.parse().unwrap_or(f64::INFINITY);
        if !(phi <= 360.0 && phi % 5.0 == 0.0) {
            return Err(format!(
                "line {n}: φ = {digits} is not a multiple of 5 from 0 to 360, so the solver skips \
                 the row"
            ));
        }
        for (k, field) in tokens[1..].iter().enumerate() {
            if let Some(problem) = value_problem(field) {
                return Err(format!(
                    "line {n}, value {} (θ = {}°): {problem}",
                    k + 1,
                    5 * k
                ));
            }
        }
        data_rows += 1;
        let bit = if phi < 360.0 {
            1u128 << ((phi / 5.0) as u32)
        } else {
            0
        };
        match blocks.iter_mut().find(|(f, _)| *f == current) {
            Some((_, rows)) => *rows |= bit,
            None => blocks.push((current, bit)),
        }
    }

    if data_rows == 0 {
        return Err("the file holds no data rows".to_string());
    }
    let full = (1u128 << PHI_ROWS) - 1;
    for &(f, rows) in &blocks {
        if rows & full != full {
            let missing: Vec<u32> = (0..PHI_ROWS)
                .filter(|k| rows & (1u128 << k) == 0)
                .map(|k| 5 * k)
                .collect();
            return Err(format!(
                "frequency {f} Hz has {} of the 72 φ rows from 0 to 355°; φ = {}° is the first \
                 missing, and the interpolation would read an empty row",
                PHI_ROWS as usize - missing.len(),
                missing[0]
            ));
        }
    }
    Ok(Balloon {
        frequencies: blocks.iter().map(|&(f, _)| f).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(freq: &str, trailing: &str) -> String {
        let mut s = format!("\"Frequency\",{freq},\"Hz\" \r\n");
        for k in 0..72 {
            s.push_str(&format!("\"{:>3}°\"", 5 * k));
            for t in 0..37 {
                s.push_str(&format!(", {:.2}", -(t as f64) / 4.0));
            }
            s.push_str(trailing);
            s.push_str("\r\n");
        }
        s
    }

    #[test]
    fn a_complete_file_parses() {
        let text = format!(
            ";  comment\r\n\"Format\",4.0\r\n{}{}",
            block("125", ""),
            block("250", "")
        );
        let b = parse(text.as_bytes()).unwrap();
        assert!(b.covers(125) && b.covers(250) && !b.covers(500));
    }

    #[test]
    fn upstreams_sample_file_parses() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/solvers/src-929a5c8/src/spps/tests/speaker-test3.txt"
        );
        // The upstream source tree is a Grace-local build input, not a committed fixture.
        let Ok(bytes) = std::fs::read(path) else {
            return;
        };
        let b = parse(&bytes).unwrap();
        for f in [40, 125, 1000, 4000, 8000] {
            assert!(b.covers(f), "{f}");
        }
        assert!(!b.covers(10000));
    }

    #[test]
    fn refused_files() {
        let cases = [
            (block("125", ","), "needs exactly 38"),
            (block("12x", ""), "not a plain finite number"),
            (block("Hz", ""), "std::stod throws"),
            (block("0", ""), "not above 0"),
            (
                block("125", "").replace(", -0.25,", ", abc,"),
                "std::stod throws",
            ),
            (
                block("125", "").replace("\" 10°\"", "\" 12°\""),
                "not a multiple of 5",
            ),
            (
                block("125", "").replace("\"  5°\"", "\"  0°\""),
                "φ = 5° is the first missing",
            ),
            (";only\r\n".to_string(), "no data rows"),
            (
                format!("\"  0°\"{}\r\n", ", 1.0".repeat(37)),
                "before any \"Frequency\"",
            ),
        ];
        for (text, reason) in cases {
            let err = parse(text.as_bytes()).unwrap_err();
            assert!(err.contains(reason), "{err}");
        }
    }
}
