//! Vertical lanes are horizontal lanes of the mirrored problem.

use mow_core::{Axis, Problem, Segment, SolveError};

/// Runs `horizontal` on `problem` for horizontal lanes, or on the mirrored
/// problem for vertical ones, mirroring its segments back.
pub(crate) fn along<F>(axis: Axis, problem: &Problem, horizontal: F) -> Result<Vec<Segment>, SolveError>
where
    F: Fn(&Problem) -> Result<Vec<Segment>, SolveError>,
{
    match axis {
        Axis::Horizontal => horizontal(problem),
        Axis::Vertical => {
            let segments = horizontal(&problem.transposed())?;
            Ok(segments.into_iter().map(Segment::transposed).collect())
        }
    }
}

/// `-h` or `-v`, the suffix the Python method names use.
pub(crate) const fn suffix(axis: Axis) -> &'static str {
    match axis {
        Axis::Horizontal => "h",
        Axis::Vertical => "v",
    }
}

/// A horizontal segment from `i64` coordinates, refused if they leave `i32`.
pub(crate) fn segment(step: &'static str, y: i64, a: i64, b: i64) -> Result<Segment, SolveError> {
    let narrow = |v: i64| {
        i32::try_from(v).map_err(|_| SolveError::Unsupported {
            step,
            reason: format!("lane coordinate {v} is outside the i32 range"),
        })
    };

    Ok(Segment::horizontal(narrow(y)?, narrow(a)?, narrow(b)?)?)
}
