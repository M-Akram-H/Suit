//! Python literals.

/// A float literal that Python reads back as the same binary64.
pub fn float(value: f64) -> String {
    match value {
        v if v.is_nan() => "float(\"nan\")".to_owned(),
        v if v.is_infinite() && v > 0.0 => "float(\"inf\")".to_owned(),
        v if v.is_infinite() => "float(\"-inf\")".to_owned(),
        // Rust's shortest round-trip form (`1.0`, `0.1`, `1e-7`) is valid Python.
        v => format!("{v:?}"),
    }
}

/// A double-quoted string literal.
pub fn string(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for c in value.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Lower-case ASCII identifier stem for a display name: `My Title!` becomes `my_title`.
pub fn identifier(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        match c {
            c if c.is_ascii_alphanumeric() => out.push(c.to_ascii_lowercase()),
            _ if !out.ends_with('_') && !out.is_empty() => out.push('_'),
            _ => {}
        }
    }
    let out = out.trim_end_matches('_');
    match out.chars().next() {
        Some(first) if first.is_ascii_alphabetic() => out.to_owned(),
        Some(_) => format!("v{out}"),
        None => "v".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literals() {
        assert_eq!(float(1.0), "1.0");
        assert_eq!(float(0.1), "0.1");
        assert_eq!(float(23.0 / 60.0), "0.38333333333333336");
        assert_eq!(float(1e-7), "1e-7");
        assert_eq!(string(r#"\frac{a}{b} "x""#), r#""\\frac{a}{b} \"x\"""#);
        assert_eq!(string("two\nlines"), "\"two\\nlines\"");
    }

    #[test]
    fn identifiers() {
        assert_eq!(identifier("circle"), "circle");
        assert_eq!(identifier("My Title!"), "my_title");
        assert_eq!(identifier("2nd  dot"), "v2nd_dot");
        assert_eq!(identifier("π"), "v");
        assert_eq!(identifier(""), "v");
    }
}
