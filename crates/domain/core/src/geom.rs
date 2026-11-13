//! Points, axes and boxes on the integer grid.

/// A lattice point, or the cell whose lower left corner it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Taxicab distance: the length of any shortest axis parallel path.
    pub fn l1(self, other: Self) -> u64 {
        let dx = (i64::from(self.x) - i64::from(other.x)).unsigned_abs();
        let dy = (i64::from(self.y) - i64::from(other.y)).unsigned_abs();

        dx + dy
    }

    pub const fn shifted(self, dx: i32, dy: i32) -> Self {
        Self::new(self.x + dx, self.y + dy)
    }

    /// Mirrored across the diagonal `x = y`.
    pub const fn transposed(self) -> Self {
        Self::new(self.y, self.x)
    }

    /// True if the two points share a row or a column.
    pub fn is_axis_parallel_to(self, other: Self) -> bool {
        self.x == other.x || self.y == other.y
    }
}

impl From<(i32, i32)> for Point {
    fn from((x, y): (i32, i32)) -> Self {
        Self::new(x, y)
    }
}

/// The direction a lane runs in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Axis {
    Horizontal,
    Vertical,
}

impl Axis {
    pub const fn flipped(self) -> Self {
        match self {
            Self::Horizontal => Self::Vertical,
            Self::Vertical => Self::Horizontal,
        }
    }
}

/// An inclusive box: `x_max` and `y_max` are the last cells inside, not one past.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Bounds {
    pub x_min: i32,
    pub y_min: i32,
    pub x_max: i32,
    pub y_max: i32,
}

impl Bounds {
    pub const fn width(self) -> u32 {
        (self.x_max - self.x_min + 1) as u32
    }

    pub const fn height(self) -> u32 {
        (self.y_max - self.y_min + 1) as u32
    }

    pub const fn contains(self, p: Point) -> bool {
        p.x >= self.x_min && p.x <= self.x_max && p.y >= self.y_min && p.y <= self.y_max
    }

    pub const fn transposed(self) -> Self {
        Self {
            x_min: self.y_min,
            y_min: self.x_min,
            x_max: self.y_max,
            y_max: self.x_max,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn l1_is_symmetric_and_handles_extremes() {
        let a = Point::new(i32::MIN, 0);
        let b = Point::new(i32::MAX, 0);

        assert_eq!(a.l1(b), u64::from(u32::MAX));
        assert_eq!(b.l1(a), a.l1(b));
        assert_eq!(Point::new(1, 2).l1(Point::new(4, -2)), 7);
    }

    #[test]
    fn bounds_measure_inclusively() {
        let b = Bounds {
            x_min: -1,
            y_min: 2,
            x_max: 3,
            y_max: 2,
        };

        assert_eq!(b.width(), 5);
        assert_eq!(b.height(), 1);
        assert!(b.contains(Point::new(3, 2)));
        assert!(!b.contains(Point::new(4, 2)));
        assert_eq!(b.transposed().width(), 1);
    }
}
