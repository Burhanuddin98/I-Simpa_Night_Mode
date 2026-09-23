//! Source names and receiver labels as Windows file names.

/// Why `name` cannot be a Windows file or folder name, if it cannot.
///
/// Refused: the empty name; control characters (Unicode category Cc, which covers the C0 range
/// Windows forbids, DEL and the C1 range); the characters `< > : " / \ | ? *`; a trailing space or
/// dot (Windows strips them, so `a.` and `a` collide); and the reserved device names CON, PRN,
/// AUX, NUL, COM0-9, LPT0-9 and COM/LPT with a superscript 1, 2 or 3, alone or before an
/// extension, in any case. The contract lists COM1-9 and LPT1-9; the 0 and superscript forms are
/// reserved by current Windows as well, so they are refused too.
pub(super) fn filename_problem(name: &str) -> Option<String> {
    if name.is_empty() {
        return Some("is empty".to_string());
    }
    if let Some(c) = name.chars().find(|c| c.is_control()) {
        return Some(format!(
            "contains the control character U+{:04X}",
            u32::from(c)
        ));
    }
    if let Some(c) = name.chars().find(|c| r#"<>:"/\|?*"#.contains(*c)) {
        return Some(format!(
            "contains '{c}', which Windows forbids in file names"
        ));
    }
    if name.ends_with(' ') || name.ends_with('.') {
        return Some("ends with a space or a dot, which Windows strips".to_string());
    }
    let stem = name.split('.').next().unwrap_or(name).trim_end_matches(' ');
    if is_reserved_device(stem) {
        return Some(format!("'{stem}' is a reserved Windows device name"));
    }
    None
}

fn is_reserved_device(stem: &str) -> bool {
    let upper = stem.to_uppercase();
    if matches!(upper.as_str(), "CON" | "PRN" | "AUX" | "NUL") {
        return true;
    }
    let tail = upper
        .strip_prefix("COM")
        .or_else(|| upper.strip_prefix("LPT"));
    match tail {
        Some(t) => {
            let mut chars = t.chars();
            matches!(
                (chars.next(), chars.next()),
                (Some('0'..='9' | '\u{b9}' | '\u{b2}' | '\u{b3}'), None)
            )
        }
        None => false,
    }
}

/// The key two names collide under: NTFS compares names case-insensitively. Lowercasing is the
/// approximation used here; NTFS's own upcase table agrees for every script in common use.
pub(super) fn collision_key(name: &str) -> String {
    name.to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names() {
        for ok in [
            "Receiver 1",
            "Récepteur Ł",
            "a.b",
            "CONSOLE",
            "COM10",
            "LPT",
            "com",
            "x%y",
        ] {
            assert_eq!(filename_problem(ok), None, "{ok}");
        }
        for bad in [
            "",
            "a:b",
            "a/b",
            "a\\b",
            "a|b",
            "a?",
            "a*",
            "<a>",
            "\"a\"",
            "a\tb",
            "a\u{7f}",
            "a ",
            "a.",
            "CON",
            "con",
            "Con.txt",
            "NUL .x",
            "COM1",
            "lpt9",
            "COM0",
            "COM\u{b9}",
            "aux",
        ] {
            assert!(filename_problem(bad).is_some(), "{bad:?}");
        }
    }
}
