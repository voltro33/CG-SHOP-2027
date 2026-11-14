//! What can go wrong while polishing: bad input, the solver, or an answer
//! the solver should not have given.

use thiserror::Error;

use mow_core::{ModelError, PortError};

#[derive(Debug, Error)]
pub enum PolishError {
    #[error(transparent)]
    Model(#[from] ModelError),

    #[error(transparent)]
    Solver(#[from] PortError),

    #[error("there are no tours to polish")]
    NoTours,

    #[error("the window side must be at least 1, not {0}")]
    BadSide(i32),

    #[error("the solver answered with {got} values for {expected} variables")]
    ShortAnswer { got: usize, expected: usize },

    #[error("the steps the solver chose are not one closed walk")]
    NotOneWalk,
}
