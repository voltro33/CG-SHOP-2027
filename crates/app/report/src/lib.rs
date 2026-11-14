//! Reports over the solver's data, ported from Python's reporting tools.
//! Each works on Python's data folder (`data/`) as well as Rust's
//! (`data/rust/`), since both keep the same formats.
//!
//! - [`catalog`]: one row of features per instance (`catalog.py`).
//! - [`leaderboard`] and [`summary`]: the leaderboard and its headline
//!   numbers (`report.py`).
//! - [`mining`]: what the job log says about which configs make bests
//!   (`mine_report.py`).
//! - [`diagnose`]: where a tour's length goes (`diagnose.py`).
//! - [`bench`]: picking and scoring the fixed benchmark set (`bench.py`).
//!
//! Numbers are printed and written as Python does ([`pyfmt`]), so the
//! outputs compare as text with Python's.

pub mod bench;
pub mod bests;
pub mod catalog;
pub mod csv;
pub mod diagnose;
pub mod family;
pub mod leaderboard;
pub mod mining;
pub mod pyfmt;
pub mod summary;
