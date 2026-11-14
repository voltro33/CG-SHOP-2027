//! Numbers and JSON the way Python writes them, so reports compare as text.
//!
//! - [`round`] is Python's `round(x, digits)`: Rust's fixed formatting rounds
//!   the exact binary value half to even, as CPython does.
//! - [`repr`] is `repr(float)`: shortest round trip digits, always a `.0` on
//!   whole numbers, exponents below `1e-4` and from `1e16` on.
//! - [`Json`] dumps like `json.dumps(value, indent=2)`, keys in insertion order.

use std::fmt::Write as _;

/// Python's `round(value, digits)`.
pub fn round(value: f64, digits: usize) -> f64 {
    if !value.is_finite() {
        return value;
    }

    format!("{value:.digits$}").parse().unwrap_or(value)
}

/// Python's `repr(value)` of a float.
pub fn repr(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_owned();
    }
    if value.is_infinite() {
        return if value > 0.0 { "Infinity" } else { "-Infinity" }.to_owned();
    }

    let magnitude = value.abs();
    if magnitude != 0.0 && !(1e-4..1e16).contains(&magnitude) {
        return exponent(value);
    }

    let text = format!("{value}");
    match text.contains('.') {
        true => text,
        false => text + ".0",
    }
}

/// `1e-05`, `1.5e+16`: Rust's shortest digits with Python's exponent style.
fn exponent(value: f64) -> String {
    let text = format!("{value:e}");
    let (mantissa, power) = text.split_once('e').unwrap_or((&text, "0"));

    let (sign, digits) = match power.strip_prefix('-') {
        Some(digits) => ('-', digits),
        None => ('+', power),
    };

    format!("{mantissa}e{sign}{digits:0>2}")
}

/// Python's `format(value, f"{width}.{digits}%")`.
pub fn percent(value: f64, width: usize, digits: usize) -> String {
    let text = format!("{:.digits$}%", value * 100.0);

    format!("{text:>width$}")
}

/// A JSON value with ordered keys, as Python builds it.
#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    pub fn str(text: impl Into<String>) -> Self {
        Self::Str(text.into())
    }

    pub fn int(value: impl TryInto<i64>) -> Self {
        value.try_into().map_or(Self::Null, Self::Int)
    }

    pub fn float(value: Option<f64>) -> Self {
        value.map_or(Self::Null, Self::Float)
    }

    pub fn object(pairs: Vec<(&str, Json)>) -> Self {
        Self::Object(pairs.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
    }

    /// `json.dumps(self, indent=2)`.
    pub fn dump(&self) -> String {
        let mut text = String::new();
        self.write(&mut text, 0);

        text
    }

    fn write(&self, out: &mut String, depth: usize) {
        match self {
            Self::Null => out.push_str("null"),
            Self::Int(value) => {
                let _ = write!(out, "{value}");
            }
            Self::Float(value) => out.push_str(&repr(*value)),
            Self::Str(text) => quote(out, text),
            Self::List(items) if items.is_empty() => out.push_str("[]"),
            Self::Object(pairs) if pairs.is_empty() => out.push_str("{}"),
            Self::List(items) => {
                out.push('[');
                for (index, item) in items.iter().enumerate() {
                    separate(out, index, depth + 1);
                    item.write(out, depth + 1);
                }
                close(out, depth, ']');
            }
            Self::Object(pairs) => {
                out.push('{');
                for (index, (key, value)) in pairs.iter().enumerate() {
                    separate(out, index, depth + 1);
                    quote(out, key);
                    out.push_str(": ");
                    value.write(out, depth + 1);
                }
                close(out, depth, '}');
            }
        }
    }
}

fn separate(out: &mut String, index: usize, depth: usize) {
    if index > 0 {
        out.push(',');
    }
    out.push('\n');
    out.push_str(&"  ".repeat(depth));
}

fn close(out: &mut String, depth: usize, bracket: char) {
    out.push('\n');
    out.push_str(&"  ".repeat(depth));
    out.push(bracket);
}

/// A JSON string with Python's default `ensure_ascii` escapes.
fn quote(out: &mut String, text: &str) {
    out.push('"');

    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || (c as u32) > 0x7e => {
                let mut units = [0u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    let _ = write!(out, "\\u{unit:04x}");
                }
            }
            c => out.push(c),
        }
    }

    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_and_prints_like_python() {
        assert_eq!(round(2.675, 2), 2.67);
        assert_eq!(round(0.125, 2), 0.12);
        assert_eq!(repr(round(1.0, 2)), "1.0");
        assert_eq!(repr(0.409), "0.409");
        assert_eq!(repr(1e-5), "1e-05");
        assert_eq!(repr(1.5e16), "1.5e+16");
        assert_eq!(repr(-0.0), "-0.0");
        assert_eq!(percent(0.5, 6, 1), " 50.0%");
        assert_eq!(percent(1.697, 6, 1), "169.7%");
    }

    #[test]
    fn dumps_with_two_space_indent() {
        let value = Json::object(vec![
            ("a", Json::Int(1)),
            ("b", Json::List(vec![Json::str("x"), Json::Float(2.0)])),
            ("c", Json::Object(vec![])),
            ("d", Json::Null),
        ]);

        let expected =
            "{\n  \"a\": 1,\n  \"b\": [\n    \"x\",\n    2.0\n  ],\n  \"c\": {},\n  \"d\": null\n}";
        assert_eq!(value.dump(), expected);
    }
}
