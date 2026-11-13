//! SVG pictures of instances, steps and solutions; plugs in as an Observer.
//!
//! A [`Scene`] borrows what it shows (the problem, and optionally segments,
//! an order, routes and uncovered cells) and streams SVG text into any
//! [`std::io::Write`]. [`SvgObserver`] turns every pipeline event into one
//! numbered picture, so a run can be replayed step by step.
//!
//! Pictures stay small on large lawns: the region is drawn as rectangles of
//! merged row runs, never cell by cell, and routes from their corners only.
//! World `y` grows upward, as in the Python plots.

mod legend;
mod observer;
pub mod palette;
mod region;
mod scene;
mod svg;
mod view;

pub use observer::SvgObserver;
pub use region::{merged_rects, CellRect};
pub use scene::Scene;
