//! OR-Tools CP-SAT behind the [`mow_core::CpSolver`] port.
//!
//! [`CpSatPool`] starts a few `tools/cpsat/cpsat.py` processes once and keeps
//! them running: each solve takes a free one, sends the model inline as one
//! JSON line and reads one answer line back. The model travels as OR-Tools'
//! own `CpModelProto` in protobuf JSON ([`to_proto_json`]), so the script
//! only parses it and solves it.
//!
//! Failures to reach the solver (no python, no script, a process that keeps
//! dying, a request Python cannot read) are [`mow_core::PortError`]s; an
//! infeasible or invalid model is a [`mow_core::CpResponse`] with that status.

mod pool;
mod process;
mod proto;

#[cfg(test)]
mod tests;

pub use pool::{python_in, CpSatPool, PYTHON_ENV, SCRIPT};
pub use proto::{from_proto_json, to_proto_json};
