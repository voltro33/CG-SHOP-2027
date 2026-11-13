//! Solution files with the `meta` that says how they were made.

use mow_core::Solution;
use mow_io::SolutionFile;
use serde_json::value::RawValue;
use serde_json::{json, Value};

/// How one solution was made, kept in its file so it can be reproduced.
#[derive(Debug, Clone, PartialEq)]
pub struct Made {
    pub job: String,
    pub config: String,
    pub round: Option<u32>,
    pub kick_scale: f64,
    pub time_limit: f64,
    pub seconds: f64,
}

/// The solution file with `meta` `{"job", "solver": "rust", "config", ...}`.
pub fn solution_file(solution: &Solution, made: &Made) -> SolutionFile {
    let meta = json!({
        "job": made.job,
        "solver": "rust",
        "config": made.config,
        "round": made.round,
        "kick_scale": made.kick_scale,
        "time_limit": made.time_limit,
        "seconds": made.seconds,
    });

    let raw = RawValue::from_string(meta.to_string()).expect("serde_json writes valid JSON");
    SolutionFile {
        meta: raw,
        ..SolutionFile::from_solution(solution)
    }
}

/// A string field of a file's `meta`, if there is one.
pub fn meta_field(file: &SolutionFile, key: &str) -> Option<String> {
    let meta: Value = serde_json::from_str(file.meta.get()).ok()?;

    meta.get(key)?.as_str().map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use mow_core::{Point, Route};

    use super::*;

    #[test]
    fn meta_names_the_rust_solver_and_config() {
        let route = Route::new(vec![Point::new(0, 0), Point::new(2, 0)]).unwrap();
        let solution = Solution::new("u", vec![route]);
        let made = Made {
            job: "u|lanes-h|s0|vr1".into(),
            config: "lanes-h".into(),
            round: Some(1),
            kick_scale: 1.0,
            time_limit: 120.0,
            seconds: 0.5,
        };

        let file = solution_file(&solution, &made);
        assert_eq!(meta_field(&file, "solver").as_deref(), Some("rust"));
        assert_eq!(meta_field(&file, "config").as_deref(), Some("lanes-h"));
        assert_eq!(meta_field(&file, "job").as_deref(), Some("u|lanes-h|s0|vr1"));

        let back = SolutionFile::parse(&file.to_json().unwrap()).unwrap();
        assert_eq!(back.to_solution().unwrap(), solution);
    }
}
