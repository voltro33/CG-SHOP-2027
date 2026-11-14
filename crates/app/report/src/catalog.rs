//! The instance catalog (`catalog.py`): one row of plain features per
//! instance, to see at a glance what shapes, cutters and cutter counts the
//! solver must handle.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use mow_core::{hull_bound, lower_bound, witness_bound, Problem};
use mow_io::{problem_from_file, InstanceFile};

use crate::csv::{line, Row, Table};
use crate::pyfmt::{repr, round};

pub const HEADER: [&str; 17] = [
    "source",
    "instance",
    "cells",
    "width",
    "height",
    "aspect",
    "fill",
    "holes",
    "k",
    "cutter_cells",
    "cutter_width",
    "cutter_height",
    "cutter_kind",
    "step_gain",
    "lower_bound",
    "hull_bound",
    "witness_bound",
];

/// The catalog's rows end in `\r\n`, as `csv.DictWriter` writes by default.
pub const LINE_END: &str = "\r\n";

/// One catalog row.
#[derive(Debug, Clone, PartialEq)]
pub struct Features {
    pub source: String,
    pub instance: String,

    // the region
    pub cells: usize,
    pub width: u32,
    pub height: u32,
    pub aspect: f64,
    /// Cells over bounding box area; low means sparse or ragged.
    pub fill: f64,
    pub holes: usize,

    // the cutters
    pub k: usize,
    pub cutter_cells: usize,
    pub cutter_width: u32,
    pub cutter_height: u32,
    pub cutter_kind: String,
    pub step_gain: u32,

    pub lower_bound: u64,
    /// The area bound for the cutter's orthogonal hull (`mow_core::hull`).
    pub hull_bound: u64,
    /// The witness bound (`mow_core::witness`).
    pub witness_bound: u64,
}

/// `square`, `rectangle` (a full box that is not square) or `other`.
pub fn cutter_kind(problem: &Problem) -> &'static str {
    let cutter = problem.cutter();
    let bounds = cutter.bounds();
    let (width, height) = (bounds.width() as usize, bounds.height() as usize);

    // a cutter that fills its bounding box exactly is a rectangle
    match (cutter.area() == width * height, width == height) {
        (false, _) => "other",
        (true, true) => "square",
        (true, false) => "rectangle",
    }
}

/// Measures one instance file; `source` is the name of its folder.
pub fn features(file: &InstanceFile, source: &str) -> Result<Features, String> {
    let problem = problem_from_file(file).map_err(|e| format!("{}: {e}", file.instance_uid))?;

    let region = problem.region_bounds();
    let (width, height) = (region.width(), region.height());
    let cutter = problem.cutter().bounds();
    let cells = problem.num_cells();

    Ok(Features {
        source: source.to_owned(),
        instance: file.instance_uid.clone(),
        cells,
        width,
        height,
        aspect: round(f64::from(width.max(height)) / f64::from(width.min(height)), 2),
        fill: round(cells as f64 / (f64::from(width) * f64::from(height)), 3),
        holes: file.region_to_cover.inner_boundaries.len(),
        k: problem.k(),
        cutter_cells: problem.cutter().area(),
        cutter_width: cutter.width(),
        cutter_height: cutter.height(),
        cutter_kind: cutter_kind(&problem).to_owned(),
        step_gain: problem.cutter().step_gain(),
        lower_bound: lower_bound(&problem),
        hull_bound: hull_bound(&problem),
        witness_bound: witness_bound(&problem),
    })
}

/// Reads and measures one `*.instance.json`.
pub fn measure(path: &Path) -> Result<Features, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let file = InstanceFile::parse(&text).map_err(|e| format!("{}: {e}", path.display()))?;

    let source = path
        .parent()
        .and_then(Path::file_name)
        .map_or(String::new(), |name| name.to_string_lossy().into_owned());

    features(&file, &source)
}

/// Every file measured on `threads` threads, in the order given.
pub fn catalog(files: &[PathBuf], threads: usize) -> Result<Vec<Features>, String> {
    let next = AtomicUsize::new(0);
    let results: Vec<Mutex<Option<Result<Features, String>>>> =
        files.iter().map(|_| Mutex::new(None)).collect();

    std::thread::scope(|scope| {
        for _ in 0..threads.max(1) {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(path) = files.get(index) else { break };

                let measured = measure(path);
                *results[index].lock().unwrap_or_else(|p| p.into_inner()) = Some(measured);
            });
        }
    });

    results
        .into_iter()
        .map(|slot| {
            let measured = slot.into_inner().unwrap_or_else(|p| p.into_inner());
            measured.unwrap_or_else(|| Err("not measured".to_owned()))
        })
        .collect()
}

impl Features {
    /// The row's cells as `csv.DictWriter` writes them.
    pub fn fields(&self) -> Vec<String> {
        vec![
            self.source.clone(),
            self.instance.clone(),
            self.cells.to_string(),
            self.width.to_string(),
            self.height.to_string(),
            repr(self.aspect),
            repr(self.fill),
            self.holes.to_string(),
            self.k.to_string(),
            self.cutter_cells.to_string(),
            self.cutter_width.to_string(),
            self.cutter_height.to_string(),
            self.cutter_kind.clone(),
            self.step_gain.to_string(),
            self.lower_bound.to_string(),
            self.hull_bound.to_string(),
            self.witness_bound.to_string(),
        ]
    }

    fn parse(row: &Row) -> Result<Self, String> {
        Ok(Self {
            source: row.get("source").to_owned(),
            instance: row.get("instance").to_owned(),
            cells: row.parse("cells")?,
            width: row.parse("width")?,
            height: row.parse("height")?,
            aspect: row.parse("aspect")?,
            fill: row.parse("fill")?,
            holes: row.parse("holes")?,
            k: row.parse("k")?,
            cutter_cells: row.parse("cutter_cells")?,
            cutter_width: row.parse("cutter_width")?,
            cutter_height: row.parse("cutter_height")?,
            cutter_kind: row.get("cutter_kind").to_owned(),
            step_gain: row.parse("step_gain")?,
            lower_bound: row.parse("lower_bound")?,
            hull_bound: parse_or_zero(row, "hull_bound")?,
            witness_bound: parse_or_zero(row, "witness_bound")?,
        })
    }

    /// True for the challenge instances, which every report counts.
    pub fn is_challenge(&self) -> bool {
        self.source == "instances"
    }
}

/// A column catalogs written before it existed lack: 0 there.
fn parse_or_zero(row: &Row, column: &str) -> Result<u64, String> {
    match row.get(column) {
        "" => Ok(0),
        _ => row.parse(column),
    }
}

/// The catalog as `catalog.py` writes it, plus the bound columns.
pub fn to_csv(rows: &[Features]) -> String {
    let header: Vec<String> = HEADER.iter().map(|&name| name.to_owned()).collect();
    let mut text = line(&header, LINE_END);

    for row in rows {
        text += &line(&row.fields(), LINE_END);
    }

    text
}

/// Every row of a catalog file.
pub fn read(path: &Path) -> Result<Vec<Features>, String> {
    let table = Table::read(path)?;

    table
        .rows()
        .map(|row| Features::parse(&row).map_err(|e| format!("{}: {e}", path.display())))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ROW: &str = "instances,fjs3649_17k_1x7,17305,73,694,9.51,0.342,26,1,33,7,7,other,11,1572,2468,1500";

    #[test]
    fn rows_without_bound_columns_read_as_zero() {
        let old = &HEADER[..15];
        let table = Table::parse(&format!("{}\n{}\n", old.join(","), &ROW[..ROW.len() - 10]));
        let row = Features::parse(&table.rows().next().unwrap()).unwrap();

        assert_eq!(row.lower_bound, 1572);
        assert_eq!((row.hull_bound, row.witness_bound), (0, 0));
    }

    #[test]
    fn rows_read_and_write_back() {
        let table = Table::parse(&format!("{}\n{ROW}\n", HEADER.join(",")));
        let row = Features::parse(&table.rows().next().unwrap()).unwrap();

        assert!(row.is_challenge());
        assert_eq!(row.aspect, 9.51);
        assert_eq!(row.fields().join(","), ROW);
        assert!(to_csv(&[row]).ends_with(&format!("{ROW}\r\n")));
    }
}
