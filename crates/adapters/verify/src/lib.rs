//! The official Python verifier behind the [`mow_core::Verifier`] port.
//!
//! [`PythonVerifier`] starts `tools/verify/verify.py` once and keeps it
//! running: each call sends one solution inline as a JSON line and reads one
//! answer line back. The script caches every instance it has loaded, so only
//! the first solution of an instance pays for rasterizing it.
//!
//! Failures to reach the verifier (no python, no script, no instance, a
//! process that keeps dying) are [`mow_core::PortError`]s; an infeasible
//! solution is a [`Report`] with errors.

mod process;
mod verifier;

#[cfg(test)]
mod tests;

pub use verifier::{python_in, PythonVerifier, Report, PYTHON_ENV, SCRIPT};
