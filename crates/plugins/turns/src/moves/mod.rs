//! The moves of the search. Each one looks at one corner of a tour, tries
//! its candidates in a fixed order and applies the first that saves steps
//! and keeps every cell swept, to the corners and to the counts alike.

mod or_opt;
mod shortcut;
mod slide;
mod transfer;
mod two_opt;

#[cfg(test)]
mod tests;

use mow_core::Point;
use mow_coverage::{CoverageError, VisitCounts};

use crate::edit::Edit;
use crate::TurnsOptions;

pub use or_opt::or_opt;
pub use shortcut::shortcut;
pub use slide::slide;
pub use transfer::transfer;
pub use two_opt::two_opt;

/// What every move shares: the visit counts of all tours, the edit buffers
/// and a count of coverage checks.
pub struct Ctx<'a> {
    pub counts: &'a mut VisitCounts,
    pub options: TurnsOptions,
    pub edit: Edit,
    pub buf: Vec<Point>,
    pub tries: u64,
}

impl<'a> Ctx<'a> {
    pub fn new(counts: &'a mut VisitCounts, options: TurnsOptions) -> Self {
        Self {
            counts,
            options,
            edit: Edit::default(),
            buf: Vec::new(),
            tries: 0,
        }
    }
}

/// The moves, cheapest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Move {
    Slide,
    Shortcut,
    TwoOpt,
    OrOpt,
}

impl Move {
    pub const ALL: [Self; 4] = [Self::Slide, Self::Shortcut, Self::TwoOpt, Self::OrOpt];

    pub const fn name(self) -> &'static str {
        crate::MOVES[self as usize]
    }

    /// Tries the move at corner `i`. Returns the steps saved, 0 if nothing changed.
    pub fn try_at(self, corners: &mut Vec<Point>, i: usize, ctx: &mut Ctx) -> Result<u64, CoverageError> {
        match self {
            Self::Slide => slide(corners, i, ctx),
            Self::Shortcut => shortcut(corners, i, ctx),
            Self::TwoOpt => two_opt(corners, i, ctx),
            Self::OrOpt => or_opt(corners, i, ctx),
        }
    }
}
