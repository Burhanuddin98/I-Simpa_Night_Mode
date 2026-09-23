//! Numbers as the solvers read them: `atoi`, and `atof` into a 32-bit `float`.

/// The `f64` an imported `f32` value becomes: the value of the `f32`'s shortest decimal, so
/// `0.31f32` (printed by upstream's GUI as `0.310000002384186`) becomes `0.31`, not
/// `0.3100000023841858`. The result always narrows back to the same `f32`
/// (`widen_f32(x) as f32` has the bits of `x`); in the rare case where the shortest decimal
/// would not, the exact widening `x as f64` is returned instead. Non-finite values widen exactly.
pub fn widen_f32(x: f32) -> f64 {
    if !x.is_finite() {
        return f64::from(x);
    }
    // Rust prints an f32 as the shortest decimal that reads back to the same f32.
    match format!("{x}").parse::<f64>() {
        Ok(short) if (short as f32).to_bits() == x.to_bits() => short,
        _ => f64::from(x),
    }
}

/// The text of a finite real: the shortest decimal that reads back to the same `f64` bits, with
/// `.`, no exponent and no grouping. `None` for NaN and the infinities.
pub(crate) fn real_text(v: f64) -> Option<String> {
    v.is_finite().then(|| format!("{v}"))
}

/// A real attribute as the solver reads it (`CoreString::ToFloat`, `coreString.cpp:89-105`): the
/// first `,` becomes `.`, then `atof`, then the `float` it is stored in. Stricter than `atof`: the
/// whole text must be a number, and a non-finite one is refused.
pub(crate) fn solver_real(text: &str) -> Option<f32> {
    let text = text.trim();
    let fixed;
    let text = match text.find(',') {
        Some(i) => {
            fixed = format!("{}.{}", &text[..i], &text[i + 1..]);
            fixed.as_str()
        }
        None => text,
    };
    if text.is_empty()
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || b"+-.eE".contains(&b))
    {
        return None;
    }
    let v = text.parse::<f64>().ok()?;
    let f = v as f32;
    f.is_finite().then_some(f)
}

/// An integer attribute as the solver reads it (`atoi`), but strict: the whole text must be a
/// decimal integer within a C `int`.
pub(crate) fn solver_int(text: &str) -> Option<i32> {
    let text = text.trim();
    let digits = text.strip_prefix(['+', '-']).unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    text.parse::<i32>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widening_recovers_the_typed_decimal() {
        assert_eq!(widen_f32(0.31), 0.31);
        assert_eq!(widen_f32(0.02), 0.02);
        assert_eq!(widen_f32(0.01), 0.01);
        assert_eq!(widen_f32(-0.0).to_bits(), (-0.0f64).to_bits());
        assert_eq!(widen_f32(solver_real("0.310000002384186").unwrap()), 0.31);
        assert_eq!(
            widen_f32(solver_real("73.1404190063477").unwrap()),
            73.14042
        );
    }

    #[test]
    fn widening_always_narrows_back() {
        // Every 4093rd f32 bit pattern, both signs, all exponents.
        let mut n = 0u32;
        let mut bits = 0u32;
        while let Some(next) = bits.checked_add(4093) {
            let x = f32::from_bits(bits);
            if x.is_finite() {
                assert_eq!((widen_f32(x) as f32).to_bits(), bits, "{x:e}");
                n += 1;
            }
            bits = next;
        }
        assert!(n > 1_000_000);
    }

    #[test]
    fn real_text_is_shortest_and_exact() {
        for v in [
            0.1,
            1e-7,
            1e21,
            -0.0,
            0.30000000000000004,
            123456.789,
            5e-324,
            f64::MAX,
        ] {
            let t = real_text(v).unwrap();
            assert!(!t.contains(['e', 'E', ',']), "{t}");
            assert_eq!(t.parse::<f64>().unwrap().to_bits(), v.to_bits(), "{t}");
        }
        assert_eq!(real_text(0.1).unwrap(), "0.1");
        assert_eq!(real_text(-0.0).unwrap(), "-0");
        assert_eq!(real_text(f64::NAN), None);
        assert_eq!(real_text(f64::INFINITY), None);
    }

    #[test]
    fn solver_parsing() {
        assert_eq!(solver_real("0,5"), Some(0.5));
        assert_eq!(solver_real("1e3"), Some(1000.0));
        assert_eq!(solver_real("abc"), None);
        assert_eq!(solver_real("inf"), None);
        assert_eq!(solver_real("1e39"), None);
        assert_eq!(solver_int("21"), Some(21));
        assert_eq!(solver_int("-1"), Some(-1));
        assert_eq!(solver_int("1.9"), None);
        assert_eq!(solver_int("true"), None);
        assert_eq!(solver_int("2147483648"), None);
    }
}
