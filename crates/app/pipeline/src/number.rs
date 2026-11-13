//! Python's `format(value, "g")`, so labels match the job log byte for byte.

/// Significant digits of Python's `g` format without a precision.
const PRECISION: i32 = 6;

/// `f"{value:g}"`: six significant digits, trailing zeros dropped, and
/// scientific notation when the exponent is below -4 or at least 6.
pub fn format_g(value: f64) -> String {
    if value.is_nan() {
        return "nan".to_string();
    }
    if value.is_infinite() {
        return if value > 0.0 { "inf" } else { "-inf" }.to_string();
    }
    if value == 0.0 {
        return if value.is_sign_negative() { "-0" } else { "0" }.to_string();
    }

    // rounding to six digits first decides the exponent, as Python does
    let scientific = format!("{:.*e}", (PRECISION - 1) as usize, value);
    let (mantissa, exponent) = scientific.split_once('e').expect("Rust writes an exponent");
    let exponent: i32 = exponent.parse().expect("Rust writes an integer exponent");

    if (-4..PRECISION).contains(&exponent) {
        let decimals = (PRECISION - 1 - exponent) as usize;
        return trimmed(&format!("{value:.decimals$}")).to_string();
    }

    let sign = if exponent < 0 { '-' } else { '+' };
    format!("{}e{sign}{:02}", trimmed(mantissa), exponent.abs())
}

/// Drops trailing zeros after a decimal point, and the point if bare.
fn trimmed(text: &str) -> &str {
    if !text.contains('.') {
        return text;
    }

    text.trim_end_matches('0').trim_end_matches('.')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_python_on_the_usual_knobs() {
        let cases = [
            (0.0, "0"),
            (1.0, "1"),
            (30.0, "30"),
            (0.25, "0.25"),
            (0.003, "0.003"),
            (2.5, "2.5"),
            (-1.5, "-1.5"),
            (16.0, "16"),
        ];

        for (value, expected) in cases {
            assert_eq!(format_g(value), expected, "{value}");
        }
    }

    #[test]
    fn matches_python_at_the_edges() {
        let cases = [
            (1e-5, "1e-05"),
            (0.0001, "0.0001"),
            (123456.0, "123456"),
            (1234567.0, "1.23457e+06"),
            (999999.5, "1e+06"),
            (1e100, "1e+100"),
            (0.1 + 0.2, "0.3"),
            (1.0 / 3.0, "0.333333"),
            (f64::INFINITY, "inf"),
            (f64::NEG_INFINITY, "-inf"),
            (f64::NAN, "nan"),
            (-0.0, "-0"),
        ];

        for (value, expected) in cases {
            assert_eq!(format_g(value), expected, "{value}");
        }
    }
}
