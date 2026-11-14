//! A square of anchors and the pieces of a closed tour inside it, as
//! `Window`, `Piece` and `cut_pieces` in `solver/window.py`.

use mow_core::Point;

/// Anchors with `x0 <= x < x1` and `y0 <= y < y1`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Window {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

impl Window {
    pub const fn new(x0: i32, y0: i32, x1: i32, y1: i32) -> Self {
        Self { x0, y0, x1, y1 }
    }

    pub const fn width(self) -> i32 {
        self.x1 - self.x0
    }

    pub const fn height(self) -> i32 {
        self.y1 - self.y0
    }

    /// Anchors in the window: `width * height`, or 0 if it is empty.
    pub const fn nodes(self) -> usize {
        if self.width() <= 0 || self.height() <= 0 {
            return 0;
        }

        self.width() as usize * self.height() as usize
    }

    pub const fn contains(self, p: Point) -> bool {
        p.x >= self.x0 && p.x < self.x1 && p.y >= self.y0 && p.y < self.y1
    }
}

/// Positions `first ..= last` of a closed tour (wrapping around), inside the
/// window, with the tour outside it just before `first` and just after `last`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Piece {
    pub first: usize,
    pub last: usize,
}

impl Piece {
    /// Unit steps from the entry to the exit, inside the window.
    pub const fn steps(self, count: usize) -> usize {
        (self.last + count - self.first) % count
    }
}

/// The pieces of the closed tour `positions` inside `window`, in tour order,
/// starting with the first piece after position 0. `None` if the tour never
/// leaves the window, since then there is nothing outside to keep; empty if
/// it never enters it.
pub fn cut_pieces(positions: &[Point], window: Window) -> Option<Vec<Piece>> {
    let count = positions.len();

    // start outside, then runs of inside positions never wrap
    let start = positions.iter().position(|&p| !window.contains(p))?;

    let mut pieces = Vec::new();
    let mut first: Option<usize> = None;

    for step in 0..=count {
        let index = (start + step) % count;
        let inside = step < count && window.contains(positions[index]);

        match (inside, first) {
            (true, None) => first = Some(index),
            (false, Some(entry)) => {
                let last = (index + count - 1) % count;
                pieces.push(Piece { first: entry, last });
                first = None;
            }
            _ => {}
        }
    }

    Some(pieces)
}

/// For every position, whether it lies in one of `pieces`.
pub(crate) fn in_pieces(count: usize, pieces: &[Piece]) -> Vec<bool> {
    let mut inside = vec![false; count];

    for piece in pieces {
        for offset in 0..=piece.steps(count) {
            inside[(piece.first + offset) % count] = true;
        }
    }

    inside
}

/// The unit steps inside the window, summed over the pieces.
pub(crate) fn old_steps(count: usize, pieces: &[Piece]) -> u64 {
    pieces.iter().map(|piece| piece.steps(count) as u64).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loop_around(corners: &[(i32, i32)]) -> Vec<Point> {
        let corners: Vec<Point> = corners.iter().map(|&c| c.into()).collect();
        mow_core::Route::new(corners).unwrap().positions().collect()
    }

    const STRAIGHT: [(i32, i32); 4] = [(-1, 0), (5, 0), (5, 2), (-1, 2)];

    #[test]
    fn both_passes_through_the_window_are_pieces() {
        let tour = loop_around(&STRAIGHT);
        let pieces = cut_pieces(&tour, Window::new(0, 0, 5, 3)).unwrap();

        // out along y = 0 from x = 0 to 4, back along y = 2 from x = 4 to 0
        assert_eq!(
            pieces,
            vec![Piece { first: 1, last: 5 }, Piece { first: 9, last: 13 }]
        );
        assert_eq!(old_steps(tour.len(), &pieces), 8);
    }

    #[test]
    fn a_tour_inside_has_nothing_to_keep_and_one_outside_no_pieces() {
        let tour = loop_around(&STRAIGHT);

        assert_eq!(cut_pieces(&tour, Window::new(-5, -5, 10, 10)), None);
        assert_eq!(cut_pieces(&tour, Window::new(20, 20, 30, 30)), Some(Vec::new()));
    }

    #[test]
    fn a_piece_may_wrap_past_position_zero() {
        // the tour starts inside the window and ends inside it
        let tour = loop_around(&[(0, 0), (5, 0), (5, 2), (0, 2)]);
        let pieces = cut_pieces(&tour, Window::new(0, 0, 2, 3)).unwrap();

        assert_eq!(pieces, vec![Piece { first: 11, last: 1 }]);
        assert_eq!(pieces[0].steps(tour.len()), 4);

        let inside = in_pieces(tour.len(), &pieces);
        assert_eq!(inside.iter().filter(|&&i| i).count(), 5);
        assert!(inside[0] && inside[1] && inside[11] && inside[13]);
    }
}
