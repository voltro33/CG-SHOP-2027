//! Just enough of the official file formats for the fixture tests: the
//! instance polygons scan converted by crossing parity (as `grid._rasterize`),
//! and the tours of a solution.

use std::path::{Path, PathBuf};

use mow_core::{BitGrid, Cutter, Point, Problem, Route};
use serde_json::Value;

pub type Ring = Vec<(i32, i32)>;

/// The repository root, three levels above this crate.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

pub fn instance_path(uid: &str) -> PathBuf {
    repo_root().join(format!(
        "data/instances/challenge_v0/instances/{uid}.instance.json"
    ))
}

pub fn best_path(uid: &str) -> PathBuf {
    repo_root().join(format!("data/solutions/best/{uid}.solution.json"))
}

fn read_json(path: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn ring(value: &Value) -> Ring {
    let coords = |key: &str| -> Vec<i32> {
        value[key]
            .as_array()
            .expect("coordinate list")
            .iter()
            .map(|v| v.as_i64().expect("integer coordinate") as i32)
            .collect()
    };

    coords("x").into_iter().zip(coords("y")).collect()
}

/// The cells enclosed by `rings`, holes by parity.
pub fn rasterize(rings: &[Ring]) -> Vec<Point> {
    let y_min = rings.iter().flatten().map(|p| p.1).min().expect("a ring");
    let y_max = rings.iter().flatten().map(|p| p.1).max().expect("a ring");
    let mut rows: Vec<Vec<i32>> = vec![Vec::new(); (y_max - y_min) as usize];

    for ring in rings {
        for (i, &(x, y1)) in ring.iter().enumerate() {
            let y2 = ring[(i + 1) % ring.len()].1;
            for y in y1.min(y2)..y1.max(y2) {
                rows[(y - y_min) as usize].push(x);
            }
        }
    }

    let mut cells = Vec::new();
    for (r, xs) in rows.iter_mut().enumerate() {
        xs.sort_unstable();
        for pair in xs.chunks_exact(2) {
            cells.extend((pair[0]..pair[1]).map(|x| Point::new(x, y_min + r as i32)));
        }
    }

    cells
}

fn grid(cells: &[Point]) -> BitGrid {
    BitGrid::from_cells(cells).unwrap().expect("some cells")
}

/// `None` when the instance file is not there (the data is not checked in).
pub fn problem(uid: &str) -> Option<Problem> {
    let json = read_json(&instance_path(uid))?;

    let region = &json["region_to_cover"];
    let mut rings = vec![ring(&region["outer_boundary"])];
    for hole in region["inner_boundaries"].as_array().into_iter().flatten() {
        rings.push(ring(hole));
    }

    let center = &json["cutter_center"];
    let (cx, cy) = (center[0].as_i64()? as i32, center[1].as_i64()? as i32);
    let offsets: Vec<Point> = rasterize(&[ring(&json["cutter"])])
        .into_iter()
        .map(|p| p.shifted(-cx, -cy))
        .collect();

    let k = json["number_of_cutters"].as_u64()? as usize;
    let cutter = Cutter::new(grid(&offsets)).unwrap();

    Some(Problem::new(uid, grid(&rasterize(&rings)), cutter, k).unwrap())
}

/// The tours of a solution file as corner lists.
pub fn tours(path: &Path) -> Option<Vec<Vec<Point>>> {
    let json = read_json(path)?;
    let tours = json["tours"].as_array()?;

    Some(
        tours
            .iter()
            .map(|t| ring(t).into_iter().map(Point::from).collect())
            .collect(),
    )
}

pub fn routes(tours: &[Vec<Point>]) -> Vec<Route> {
    tours.iter().map(|t| Route::new(t.clone()).unwrap()).collect()
}

/// One fixture line: `uid expected tour index count dx dy`.
#[derive(Debug, Clone)]
pub struct Recipe {
    pub uid: String,
    pub expected: usize,
    pub tour: usize,
    pub index: usize,
    pub count: usize,
    pub dx: i32,
    pub dy: i32,
}

impl Recipe {
    pub fn parse(line: &str) -> Self {
        let f: Vec<&str> = line.split_whitespace().collect();
        let n = |i: usize| f[i].parse::<i64>().expect("number");

        Self {
            uid: f[0].to_string(),
            expected: n(1) as usize,
            tour: n(2) as usize,
            index: n(3) as usize,
            count: n(4) as usize,
            dx: n(5) as i32,
            dy: n(6) as i32,
        }
    }

    /// `count` corners from `index`, wrapping, moved by `(dx, dy)`.
    pub fn apply(&self, tours: &[Vec<Point>]) -> Vec<Vec<Point>> {
        let mut result = tours.to_vec();
        let corners = &mut result[self.tour];
        let n = corners.len();

        for step in 0..self.count {
            let p = &mut corners[(self.index + step) % n];
            *p = p.shifted(self.dx, self.dy);
        }

        result
    }
}

pub fn recipes() -> Vec<Recipe> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mutations.txt");
    let text = std::fs::read_to_string(path).expect("fixture file");

    text.lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(Recipe::parse)
        .collect()
}
