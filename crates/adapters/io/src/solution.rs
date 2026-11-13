//! Solution files: read into a [`Solution`], written in the official format.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use mow_core::{Point, Route, Solution};

use crate::error::{IoError, TourError};
use crate::schema::{empty_meta, PointSequence, SolutionFile, SOLUTION_CONTENT_TYPE};

/// Reads and checks one `*.solution.json`. `meta` is dropped.
pub fn load_solution(path: &Path) -> Result<Solution, IoError> {
    load_solution_file(path)?.to_solution()
}

/// Writes `solution` with an empty `meta`.
pub fn save_solution(path: &Path, solution: &Solution) -> Result<(), IoError> {
    save_solution_file(path, &SolutionFile::from_solution(solution))
}

/// Reads a solution file as it is, `meta` included.
pub fn load_solution_file(path: &Path) -> Result<SolutionFile, IoError> {
    let json = crate::read_text(path)?;
    SolutionFile::parse(&json).map_err(|e| e.in_file(path))
}

/// Writes through a temporary file next to `path` and a rename, so a crash
/// never leaves half a file behind.
pub fn save_solution_file(path: &Path, file: &SolutionFile) -> Result<(), IoError> {
    let json = file.to_json()?;
    let temp = temp_path(path);

    let written = write_all(&temp, json.as_bytes()).and_then(|()| fs::rename(&temp, path));
    written.map_err(|source| {
        // the temporary file may not exist; the write error is what matters
        let _ = fs::remove_file(&temp);
        IoError::Write {
            path: path.to_path_buf(),
            source,
        }
    })
}

impl SolutionFile {
    pub fn from_solution(solution: &Solution) -> Self {
        Self {
            content_type: SOLUTION_CONTENT_TYPE.to_owned(),
            instance_uid: solution.uid().to_owned(),
            tours: solution.routes().iter().map(tour).collect(),
            meta: empty_meta(),
        }
    }

    /// Checks every tour as the Python schema does.
    pub fn to_solution(&self) -> Result<Solution, IoError> {
        let routes = self
            .tours
            .iter()
            .enumerate()
            .map(|(index, t)| route(t).map_err(|reason| IoError::Tour { index, reason }))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Solution::new(&self.instance_uid, routes))
    }
}

fn tour(route: &Route) -> PointSequence {
    PointSequence {
        x: route.corners().iter().map(|p| p.x).collect(),
        y: route.corners().iter().map(|p| p.y).collect(),
    }
}

/// Equal lengths, at least one point, no point repeated right after itself
/// (the closing edge included), every edge axis parallel.
fn route(tour: &PointSequence) -> Result<Route, TourError> {
    if tour.x.len() != tour.y.len() {
        return Err(TourError::LengthMismatch {
            x: tour.x.len(),
            y: tour.y.len(),
        });
    }

    let corners: Vec<Point> = tour
        .x
        .iter()
        .zip(&tour.y)
        .map(|(&x, &y)| Point::new(x, y))
        .collect();

    if corners.len() > 1 {
        let next = corners.iter().cycle().skip(1);
        if let Some(index) = corners.iter().zip(next).position(|(a, b)| a == b) {
            return Err(TourError::RepeatedPoint(corners[index], index));
        }
    }

    Ok(Route::new(corners)?)
}

fn temp_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}.tmp", std::process::id()));
    path.with_file_name(name)
}

fn write_all(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = fs::File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mow_core::ModelError;

    fn parse(json: &str) -> Result<Solution, IoError> {
        SolutionFile::parse(json)?.to_solution()
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mow-io-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn round_trip_through_a_file() {
        let square = Route::new(vec![
            Point::new(0, 0),
            Point::new(3, 0),
            Point::new(3, 2),
            Point::new(0, 2),
        ]);
        let still = Route::new(vec![Point::new(-5, 7)]);
        let solution = Solution::new("rt", vec![square.unwrap(), still.unwrap()]);

        let path = scratch("rt.solution.json");
        save_solution(&path, &solution).unwrap();
        let back = load_solution(&path).unwrap();

        assert_eq!(back, solution);
        assert_eq!(back.objective(), 10);
        assert!(fs::read_to_string(&path)
            .unwrap()
            .starts_with(r#"{"content_type":"CGSHOP2027_Solution""#));
    }

    #[test]
    fn meta_survives_byte_for_byte() {
        let json = r#"{"content_type":"CGSHOP2027_Solution","instance_uid":"m","tours":[{"x":[0,2],"y":[1,1]}],"meta":{"z":1,"a":[1.0,{"b":null}]}}"#;

        let path = scratch("m.solution.json");
        save_solution_file(&path, &SolutionFile::parse(json).unwrap()).unwrap();

        assert_eq!(fs::read_to_string(&path).unwrap(), json);
    }

    #[test]
    fn bad_tours_are_errors() {
        let repeat = parse(r#"{"instance_uid":"a","tours":[{"x":[0,0],"y":[1,1]}]}"#);
        assert!(matches!(
            repeat,
            Err(IoError::Tour {
                index: 0,
                reason: TourError::RepeatedPoint(_, 0)
            })
        ));

        let diagonal = parse(r#"{"instance_uid":"a","tours":[{"x":[0],"y":[0]},{"x":[0,1],"y":[0,1]}]}"#);
        assert!(matches!(
            diagonal,
            Err(IoError::Tour {
                index: 1,
                reason: TourError::Model(ModelError::NotAxisParallel(..))
            })
        ));

        let empty = parse(r#"{"instance_uid":"a","tours":[{"x":[],"y":[]}]}"#);
        assert!(matches!(
            empty,
            Err(IoError::Tour {
                reason: TourError::Model(ModelError::EmptyRoute),
                ..
            })
        ));

        let lengths = parse(r#"{"instance_uid":"a","tours":[{"x":[0,1],"y":[0]}]}"#);
        assert!(matches!(
            lengths,
            Err(IoError::Tour {
                reason: TourError::LengthMismatch { x: 2, y: 1 },
                ..
            })
        ));
    }

    #[test]
    fn writing_into_a_missing_directory_is_an_error() {
        let path = Path::new("no/such/dir/x.solution.json");
        let solution = Solution::new("x", vec![Route::new(vec![Point::new(0, 0)]).unwrap()]);

        assert!(matches!(
            save_solution(path, &solution),
            Err(IoError::Write { .. })
        ));
    }
}
