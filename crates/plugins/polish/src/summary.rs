//! One line of numbers per tiling: `summarize` and the report of `main` in
//! `solver/window_map.py`, as a library for the `mow` CLI.
//!
//! Sums go through [`numpy_sum`], numpy's pairwise summation, so the
//! shares come out to the last bit as Python's.

use mow_core::{Problem, Route};

use crate::error::PolishError;
use crate::window_map::{cutter_span, window_map, WindowMap};

/// Window sides in cutter spans.
pub const DEFAULT_SIDES: [u32; 3] = [2, 4, 8];

/// The worst windows, as shares of all windows the tours touch.
pub const WORST_SHARES: [f64; 2] = [0.10, 0.25];

#[derive(Debug, Clone, PartialEq)]
pub struct WindowSummary {
    pub instance: String,
    pub family: &'static str,
    pub side_spans: u32,
    pub side: i32,
    pub windows: usize,
    pub room_share: f64,
    pub worst_10_share: f64,
    pub worst_25_share: f64,
    pub worst_10_pieces: f64,
    pub worst_10_room: f64,
}

/// The summary of one map. A map no tour touches gives zeros, where
/// Python would fail on the median of nothing.
pub fn summarize(problem: &Problem, map: &WindowMap, side_spans: u32) -> WindowSummary {
    let all_room = map.room();
    let touched: Vec<usize> = (0..map.length.len()).filter(|&w| map.length[w] > 0).collect();
    let room: Vec<f64> = touched.iter().map(|&w| all_room[w]).collect();
    let pieces: Vec<f64> = touched.iter().map(|&w| map.pieces[w] as f64).collect();

    // windows ordered by room, most first; ties keep their order
    let mut order: Vec<usize> = (0..room.len()).collect();
    order.sort_by(|&a, &b| room[b].total_cmp(&room[a]));

    let clipped = |indices: &[usize]| numpy_sum(indices.iter().map(|&i| room[i].max(0.0)));
    // summed in window order, as numpy sums `np.clip(room, 0, None)`
    let positive_room = numpy_sum(room.iter().map(|r| r.max(0.0)));

    let shares = WORST_SHARES.map(|share| {
        let worst = clipped(&order[..worst_count(share, room.len())]);
        if positive_room != 0.0 {
            worst / positive_room
        } else {
            0.0
        }
    });

    let worst = &order[..worst_count(WORST_SHARES[0], room.len())];

    WindowSummary {
        instance: problem.uid().to_string(),
        family: family(problem.uid()),
        side_spans,
        side: map.side,
        windows: touched.len(),
        room_share: numpy_sum(room.iter().copied()) / map.total_length as f64,
        worst_10_share: shares[0],
        worst_10_pieces: median(worst.iter().map(|&i| pieces[i]).collect()),
        worst_25_share: shares[1],
        worst_10_room: median(worst.iter().map(|&i| room[i]).collect()),
    }
}

/// `max(1, round(share * count))`, rounding half to even as Python does.
/// At most `count`, so an empty map takes no window.
fn worst_count(share: f64, count: usize) -> usize {
    let rounded = (share * count as f64).round_ties_even() as usize;
    rounded.max(1).min(count)
}

/// `statistics.median`: the middle value, or the mean of the two middle ones.
fn median(mut values: Vec<f64>) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;

    if values.len() % 2 == 1 {
        values[middle]
    } else {
        (values[middle - 1] + values[middle]) / 2.0
    }
}

/// The values numpy's reduction hands its inner loop at a time.
const NUMPY_BUFFER: usize = 8192;

/// `np.sum` of float64: the values go in buffers of 8192, each summed
/// pairwise (blocks of eight, halves above 128 values, a plain loop below
/// 8) and added to the total in turn.
pub fn numpy_sum(values: impl IntoIterator<Item = f64>) -> f64 {
    let values: Vec<f64> = values.into_iter().collect();

    values
        .chunks(NUMPY_BUFFER)
        .fold(0.0, |total, chunk| total + pairwise(chunk))
}

fn pairwise(values: &[f64]) -> f64 {
    let n = values.len();

    if n < 8 {
        return values.iter().fold(0.0, |sum, &v| sum + v);
    }

    if n <= 128 {
        let mut lanes = [0.0; 8];
        lanes.copy_from_slice(&values[..8]);

        let blocks = n - n % 8;
        for block in values[8..blocks].chunks_exact(8) {
            for (lane, &v) in lanes.iter_mut().zip(block) {
                *lane += v;
            }
        }

        let r = lanes;
        let head = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]));
        return values[blocks..].iter().fold(head, |sum, &v| sum + v);
    }

    let half = n / 2;
    let half = half - half % 8;
    pairwise(&values[..half]) + pairwise(&values[half..])
}

/// The cutter family of an instance, read from the end of its name, as
/// `family` in `solver/report.py` (`_(\d)([a-z])(\d+)[a-z]*$`).
pub fn family(uid: &str) -> &'static str {
    let bytes = uid.as_bytes();
    let mut end = bytes.len();

    while end > 0 && bytes[end - 1].is_ascii_lowercase() {
        end -= 1;
    }

    let digits_end = end;
    while end > 0 && bytes[end - 1].is_ascii_digit() {
        end -= 1;
    }

    let shape_ok = end < digits_end && end >= 3;
    let matched = shape_ok
        && bytes[end - 1].is_ascii_lowercase()
        && bytes[end - 2].is_ascii_digit()
        && bytes[end - 3] == b'_';
    if !matched {
        return "unknown";
    }

    match bytes[end - 1] {
        b's' => "square",
        b'l' => "diamond",
        b'd' => "clipped square",
        b'p' => "plus",
        b'x' => "X",
        _ => "unknown",
    }
}

/// The summaries of the tours of `routes` for each side in `sides_spans`,
/// in cutter spans: what `python -m solver.window_map` prints for one
/// instance.
pub fn window_report(
    problem: &Problem,
    routes: &[Route],
    sides_spans: &[u32],
) -> Result<Vec<WindowSummary>, PolishError> {
    let span = cutter_span(problem);

    sides_spans
        .iter()
        .map(|&spans| {
            let side = i32::try_from(spans).unwrap_or(i32::MAX).saturating_mul(span);
            let map = window_map(problem, routes, side)?;

            Ok(summarize(problem, &map, spans))
        })
        .collect()
}

/// The header of the report, as Python prints it.
pub fn report_header() -> String {
    format!(
        "{:30} {:>4} {:>7} {:>6} {:>6} {:>6} {:>6} {:>7}",
        "instance", "side", "windows", "room", "top10", "top25", "pieces", "room/w"
    )
}

/// One line of the report, as Python prints it.
pub fn report_line(summary: &WindowSummary) -> String {
    let percent = |share: f64| format!("{:.1}%", share * 100.0);

    format!(
        "{:30} {:>4} {:>7} {:>6} {:>6} {:>6} {:>6.0} {:>7.0}",
        summary.instance,
        summary.side_spans,
        summary.windows,
        percent(summary.room_share),
        percent(summary.worst_10_share),
        percent(summary.worst_25_share),
        summary.worst_10_pieces,
        summary.worst_10_room,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn family_reads_the_cutter_letter() {
        assert_eq!(family("euronight0070000_8749k_3s12"), "square");
        assert_eq!(family("map_fl4_20250813_1725_79k_2p6"), "plus");
        assert_eq!(family("ics39603_9468_1l5"), "diamond");
        assert_eq!(family("dca1389_1773k_3x12b"), "X");
        assert_eq!(family("nothing_here"), "unknown");
        assert_eq!(family("_1q5"), "unknown");
    }

    #[test]
    fn median_of_even_and_odd_counts() {
        assert_eq!(median(vec![3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median(vec![4.0, 1.0, 2.0, 3.0]), 2.5);
        assert_eq!(median(Vec::new()), 0.0);
    }

    #[test]
    fn worst_count_rounds_half_to_even() {
        assert_eq!(worst_count(0.10, 25), 2);
        assert_eq!(worst_count(0.10, 35), 4);
        assert_eq!(worst_count(0.25, 2), 1);
        assert_eq!(worst_count(0.10, 3), 1);
    }

    #[test]
    fn pairwise_sum_differs_from_a_plain_loop_as_numpy_does() {
        let values: Vec<f64> = (0..300).map(|i| 0.1 * f64::from(i) + 1e-3).collect();
        let plain = values.iter().fold(0.0, |s, &v| s + v);

        assert!((numpy_sum(values.iter().copied()) - plain).abs() < 1e-9);
        assert_eq!(numpy_sum([1.0, 2.0, 3.0]), 6.0);
    }

    #[test]
    fn report_line_formats_like_python() {
        let summary = WindowSummary {
            instance: "x_1s2".into(),
            family: "square",
            side_spans: 2,
            side: 4,
            windows: 12,
            room_share: 0.1234,
            worst_10_share: 0.5,
            worst_25_share: 1.0,
            worst_10_pieces: 3.0,
            worst_10_room: 17.4,
        };

        let expected = format!(
            "{:30} {:>4} {:>7} {:>6} {:>6} {:>6} {:>6} {:>7}",
            "x_1s2", 2, 12, "12.3%", "50.0%", "100.0%", 3, 17
        );
        assert_eq!(report_line(&summary), expected);
    }
}
