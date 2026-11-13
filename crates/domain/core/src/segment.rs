//! A straight stretch of anchors on one lane.

use crate::error::ModelError;
use crate::geom::{Axis, Point};

/// Anchors `from..=to` along a lane. A horizontal segment runs in `x` at
/// height `lane`, a vertical one in `y` at `x = lane`. Driving it costs
/// `to - from` whichever way it is driven.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Segment {
    axis: Axis,
    lane: i32,
    from: i32,
    to: i32,
}

impl Segment {
    pub fn new(axis: Axis, lane: i32, from: i32, to: i32) -> Result<Self, ModelError> {
        if from > to {
            return Err(ModelError::BackwardSegment { from, to });
        }

        Ok(Self { axis, lane, from, to })
    }

    pub fn horizontal(y: i32, from: i32, to: i32) -> Result<Self, ModelError> {
        Self::new(Axis::Horizontal, y, from, to)
    }

    pub fn vertical(x: i32, from: i32, to: i32) -> Result<Self, ModelError> {
        Self::new(Axis::Vertical, x, from, to)
    }

    pub const fn axis(self) -> Axis {
        self.axis
    }

    pub const fn lane(self) -> i32 {
        self.lane
    }

    pub const fn from(self) -> i32 {
        self.from
    }

    pub const fn to(self) -> i32 {
        self.to
    }

    pub const fn length(self) -> u64 {
        (self.to as i64 - self.from as i64) as u64
    }

    /// The low end: smallest `x` (horizontal) or `y` (vertical).
    pub const fn start(self) -> Point {
        self.at(self.from)
    }

    pub const fn end(self) -> Point {
        self.at(self.to)
    }

    /// `(entry, exit)` when driven forwards, or backwards if `reversed`.
    pub const fn ends(self, reversed: bool) -> (Point, Point) {
        if reversed {
            (self.end(), self.start())
        } else {
            (self.start(), self.end())
        }
    }

    /// Mirrored across `x = y`: the same numbers on the other axis.
    pub const fn transposed(self) -> Self {
        Self {
            axis: self.axis.flipped(),
            ..self
        }
    }

    const fn at(self, t: i32) -> Point {
        match self.axis {
            Axis::Horizontal => Point::new(t, self.lane),
            Axis::Vertical => Point::new(self.lane, t),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ends_follow_the_axis() {
        let h = Segment::horizontal(4, -2, 5).unwrap();
        let v = h.transposed();

        assert_eq!(h.ends(false), (Point::new(-2, 4), Point::new(5, 4)));
        assert_eq!(v.ends(true), (Point::new(4, 5), Point::new(4, -2)));
        assert_eq!(v.length(), 7);
        assert_eq!(v.axis(), Axis::Vertical);
    }

    #[test]
    fn single_anchor_segments_are_allowed_backward_ones_are_not() {
        assert_eq!(Segment::vertical(0, 3, 3).unwrap().length(), 0);
        assert!(Segment::horizontal(0, 3, 2).is_err());
    }
}
