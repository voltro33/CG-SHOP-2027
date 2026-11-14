//! A dense closed tour with the pieces inside an area taken out: what is
//! left falls into chains, each a run of positions from one piece's exit
//! to the next piece's entry.

use mow_core::Point;
use mow_polish::{cut_pieces, Piece, Window};

/// Positions `first, first + 1, ..` (wrapping), `len` of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chain {
    pub first: usize,
    pub len: usize,
}

impl Chain {
    pub fn entry(self, positions: &[Point]) -> Point {
        positions[self.first]
    }

    pub fn exit(self, positions: &[Point]) -> Point {
        positions[(self.first + self.len - 1) % positions.len()]
    }

    /// Unit steps inside the chain.
    pub fn steps(self) -> u64 {
        self.len as u64 - 1
    }

    /// The chain's positions, in tour order or backwards.
    pub fn positions<'a>(self, positions: &'a [Point], reversed: bool) -> impl Iterator<Item = Point> + 'a {
        let n = positions.len();
        (0..self.len).map(move |t| {
            let t = if reversed { self.len - 1 - t } else { t };
            positions[(self.first + t) % n]
        })
    }
}

/// The tour cut by an area: the positions taken out, and the chains left.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cut {
    pub removed: Vec<Point>,
    pub chains: Vec<Chain>,
}

/// Cuts the closed dense tour `positions` by `window`. `None` if the tour
/// never enters the window or never leaves it.
pub fn cut(positions: &[Point], window: Window) -> Option<Cut> {
    let pieces = cut_pieces(positions, window)?;
    if pieces.is_empty() {
        return None;
    }

    Some(Cut {
        removed: removed(positions, &pieces),
        chains: chains(positions.len(), &pieces),
    })
}

fn removed(positions: &[Point], pieces: &[Piece]) -> Vec<Point> {
    let n = positions.len();

    pieces
        .iter()
        .flat_map(|piece| (0..=piece.steps(n)).map(move |t| positions[(piece.first + t) % n]))
        .collect()
}

/// The runs between consecutive pieces, never empty since pieces are maximal.
fn chains(n: usize, pieces: &[Piece]) -> Vec<Chain> {
    let m = pieces.len();

    (0..m)
        .map(|i| {
            let (piece, next) = (pieces[i], pieces[(i + 1) % m]);
            Chain {
                first: (piece.last + 1) % n,
                len: (next.first + 2 * n - piece.last - 1) % n,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 10 x 4 rectangle tour, corners (0,0) (9,0) (9,3) (0,3).
    fn ring() -> Vec<Point> {
        let bottom = (0..9).map(|x| Point::new(x, 0));
        let right = (0..3).map(|y| Point::new(9, y));
        let top = (1..=9).rev().map(|x| Point::new(x, 3));
        let left = (1..=3).rev().map(|y| Point::new(0, y));

        bottom.chain(right).chain(top).chain(left).collect()
    }

    #[test]
    fn a_window_across_the_ring_leaves_two_chains() {
        let tour = ring();
        let cut = cut(&tour, Window::new(4, -1, 6, 5)).unwrap();

        // x = 4 and 5 on the bottom and the top
        assert_eq!(cut.removed.len(), 4);
        assert_eq!(cut.chains.len(), 2);

        let total: usize = cut.chains.iter().map(|c| c.len).sum();
        assert_eq!(total + cut.removed.len(), tour.len());

        let first = cut.chains[0];
        assert_eq!(first.entry(&tour), Point::new(6, 0));
        assert_eq!(first.exit(&tour), Point::new(6, 3));
        let back: Vec<Point> = first.positions(&tour, true).collect();
        assert_eq!(back[0], Point::new(6, 3));
    }

    #[test]
    fn windows_missing_or_holding_the_tour_cut_nothing() {
        let tour = ring();

        assert!(cut(&tour, Window::new(20, 20, 30, 30)).is_none());
        assert!(cut(&tour, Window::new(-5, -5, 20, 20)).is_none());
    }

    #[test]
    fn a_chain_may_wrap_past_position_zero() {
        let tour = ring();
        let cut = cut(&tour, Window::new(3, 2, 6, 5)).unwrap();

        assert_eq!(cut.chains.len(), 1);
        let chain = cut.chains[0];
        assert_eq!(chain.entry(&tour), Point::new(2, 3));
        assert_eq!(chain.exit(&tour), Point::new(6, 3));
        assert_eq!(chain.len, tour.len() - 3);
    }
}
