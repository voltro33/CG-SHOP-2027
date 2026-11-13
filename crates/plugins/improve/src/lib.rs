//! Repair and polish of walks and routes, on top of `mow-coverage`.
//!
//! - [`Detours`] (`detours`): out-and-back trips to the cells a walk or a set
//!   of routes misses, as `solver/repair.py`.
//! - [`CornerCut`] (`corner-cut`): cut corners while every cell stays swept,
//!   as `prune_routes` in `solver/improve.py`.
//!
//! Both make the same choices as Python in the same order, so on the same
//! input they give the same corners; they just never build the dense
//! position arrays Python works on.

mod cut;
mod normalize;
mod prune;
mod repair;
mod stretches;

pub use prune::CornerCut;
pub use repair::Detours;
