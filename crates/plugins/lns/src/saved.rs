//! What a search leaves for the next run of the same instance, one JSON
//! file per instance (`<out>/state/<uid>.json`): the operator weights and
//! the solution the walk stood on. An instance seen first starts from the
//! mean weights of the instances with the same cutter shape.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use mow_core::{Point, Route};

use crate::error::LnsError;
use crate::weights::Wheel;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Saved {
    pub uid: String,
    pub runs: u32,
    pub seconds: f64,
    /// The stored best when the run ended: a later, shorter best from
    /// elsewhere makes the saved walk stale.
    pub stored_best: u64,
    pub current_objective: u64,
    /// Corners of each route of the current solution.
    pub current: Vec<Vec<[i32; 2]>>,
    pub destroy: Wheel,
    pub recreate: Wheel,
}

impl Saved {
    pub fn path(dir: &Path, uid: &str) -> PathBuf {
        dir.join(format!("{uid}.json"))
    }

    /// The saved state of `uid`, if there is one.
    pub fn load(dir: &Path, uid: &str) -> Result<Option<Self>, LnsError> {
        let path = Self::path(dir, uid);
        if !path.is_file() {
            return Ok(None);
        }

        let text = std::fs::read_to_string(&path).map_err(|e| state_error(&path, e))?;
        serde_json::from_str(&text)
            .map(Some)
            .map_err(|e| state_error(&path, e))
    }

    /// Writes the state through a temporary file, so a crash never leaves
    /// half a file.
    pub fn save(&self, dir: &Path) -> Result<(), LnsError> {
        let path = Self::path(dir, &self.uid);
        std::fs::create_dir_all(dir).map_err(|e| state_error(&path, e))?;

        let text = serde_json::to_string(self).map_err(|e| state_error(&path, e))?;
        let partial = path.with_extension("json.partial");
        std::fs::write(&partial, text).map_err(|e| state_error(&path, e))?;
        std::fs::rename(&partial, &path).map_err(|e| state_error(&path, e))
    }

    pub fn routes(&self) -> Result<Vec<Route>, LnsError> {
        self.current
            .iter()
            .map(|corners| {
                Ok(Route::new(
                    corners.iter().map(|&[x, y]| Point::new(x, y)).collect(),
                )?)
            })
            .collect()
    }

    pub fn corners_of(routes: &[Route]) -> Vec<Vec<[i32; 2]>> {
        routes
            .iter()
            .map(|r| r.corners().iter().map(|p| [p.x, p.y]).collect())
            .collect()
    }
}

/// The cutter shape letter of an instance name: `x` of `fjs3649_17k_1x7`.
pub fn shape_of(uid: &str) -> Option<char> {
    let last = uid.rsplit('_').next()?;
    last.chars().find(|c| c.is_ascii_alphabetic())
}

/// The saved wheels of every other instance with the same cutter shape.
pub fn same_shape(dir: &Path, uid: &str) -> Vec<(Wheel, Wheel)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let shape = shape_of(uid);

    entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_str()?.strip_suffix(".json")?.to_owned();
            (name != uid && shape_of(&name) == shape).then_some(name)
        })
        .filter_map(|name| Saved::load(dir, &name).ok().flatten())
        .map(|saved| (saved.destroy, saved.recreate))
        .collect()
}

fn state_error(path: &Path, error: impl std::fmt::Display) -> LnsError {
    LnsError::State {
        path: path.display().to_string(),
        reason: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes_come_from_the_last_part_of_the_name() {
        assert_eq!(shape_of("fjs3649_17k_1x7"), Some('x'));
        assert_eq!(shape_of("isoa22945_987k_3x10"), Some('x'));
        assert_eq!(shape_of("srpg5461_22k_1l3"), Some('l'));
        assert_eq!(shape_of("plain"), Some('p'));
    }

    #[test]
    fn saves_and_loads_and_pools_by_shape() {
        let dir = std::env::temp_dir().join(format!("mow-lns-saved-{}", std::process::id()));
        let route = Route::new(vec![Point::new(0, 0), Point::new(4, 0)]).unwrap();

        let mut wheel = Wheel::new(["a"]);
        wheel.operators[0].weight = 3.0;
        let saved = Saved {
            uid: "aa_1k_1x3".to_owned(),
            runs: 1,
            seconds: 2.0,
            stored_best: 8,
            current_objective: 8,
            current: Saved::corners_of(std::slice::from_ref(&route)),
            destroy: wheel.clone(),
            recreate: wheel,
        };
        saved.save(&dir).unwrap();

        let back = Saved::load(&dir, "aa_1k_1x3").unwrap().unwrap();
        assert_eq!(back.routes().unwrap(), vec![route]);
        assert_eq!(back.destroy.operators[0].weight, 3.0);
        assert!(Saved::load(&dir, "missing").unwrap().is_none());

        assert_eq!(same_shape(&dir, "bb_2k_2x5").len(), 1);
        assert!(same_shape(&dir, "bb_2k_2s5").is_empty());
        assert!(same_shape(&dir, "aa_1k_1x3").is_empty());

        std::fs::remove_dir_all(dir).unwrap();
    }
}
