//! Small numbers every report shares: the cutter family of an instance, the
//! pessimistic score, percent changes and medians (`report.py`).

use crate::pyfmt::round;

/// The cutter family letters of instance names, and what they mean.
const FAMILY_NAMES: [(u8, &str); 5] = [
    (b's', "square"),
    (b'l', "diamond"),
    (b'd', "clipped square"),
    (b'p', "plus"),
    (b'x', "X"),
];

/// The cutter family of an instance, from its name: names end in `_`, k,
/// the cutter letter and the cutter size, sometimes with a variant letter
/// (`_(\d)([a-z])(\d+)[a-z]*$` in Python).
pub fn family(uid: &str) -> &'static str {
    family_letter(uid.as_bytes())
        .and_then(|letter| FAMILY_NAMES.iter().find(|(l, _)| *l == letter))
        .map_or("unknown", |(_, name)| name)
}

fn family_letter(name: &[u8]) -> Option<u8> {
    let rest = trim_end(name, |b| b.is_ascii_lowercase());

    let before_size = trim_end(rest, |b| b.is_ascii_digit());
    if before_size.len() == rest.len() {
        return None;
    }

    match before_size {
        [.., b'_', k, letter] if k.is_ascii_digit() && letter.is_ascii_lowercase() => Some(*letter),
        _ => None,
    }
}

fn trim_end(bytes: &[u8], drop: impl Fn(u8) -> bool) -> &[u8] {
    let keep = bytes.iter().rposition(|&b| !drop(b)).map_or(0, |i| i + 1);

    &bytes[..keep]
}

/// `(bound / objective) ** 2`, the score if a team reached the lower bound.
pub fn pessimistic_score(bound: u64, objective: u64) -> Option<f64> {
    if bound == 0 || objective == 0 {
        return None;
    }

    Some((bound as f64 / objective as f64).powi(2))
}

/// `round(100 (value - reference) / reference, 1)`.
pub fn pct(value: f64, reference: f64) -> f64 {
    round(100.0 * (value - reference) / reference, 1)
}

/// `statistics.median`: the middle value, or the mean of the two middle ones.
pub fn median(values: &[f64]) -> Option<f64> {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);

    let n = sorted.len();
    match n {
        0 => None,
        _ if n % 2 == 1 => Some(sorted[n / 2]),
        _ => Some((sorted[n / 2 - 1] + sorted[n / 2]) / 2.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_reads_the_cutter_letter() {
        assert_eq!(family("ara238025_353k_1l5"), "diamond");
        assert_eq!(family("bby34656_1372k_3x3"), "X");
        assert_eq!(family("fht47608_34k_2p4"), "plus");
        assert_eq!(family("srpg2839_25k_1d7b"), "clipped square");
        assert_eq!(family("spg-c-poly6002_28k_1s4"), "square");
        assert_eq!(family("no_suffix_here"), "unknown");
        assert_eq!(family("a_1q2"), "unknown");
    }

    #[test]
    fn pessimistic_score_is_squared_bound_ratio() {
        assert_eq!(pessimistic_score(50, 100), Some(0.25));
        assert_eq!(pessimistic_score(100, 100), Some(1.0));
        assert_eq!(pessimistic_score(0, 100), None);
    }

    #[test]
    fn medians_like_statistics_median() {
        assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(&[4.0, 1.0, 2.0, 3.0]), Some(2.5));
        assert_eq!(median(&[]), None);
        assert_eq!(pct(110.0, 100.0), 10.0);
    }
}
