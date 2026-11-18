//! Two solution files of the same instance.
//!
//! Each file must pass our own coverage check and then the official
//! verifier before anything is compared. Only then do the lengths and the
//! points of both go into a table, with both tours drawn side by side on
//! one page, panned and zoomed together.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use mow_core::{lower_bound, Problem, Solution};
use mow_coverage::uncovered_count;
use mow_io::{load_problem, load_solution_file, SolutionFile};
use mow_tui::viewer::{self, Panel, Table};

use super::CompareOptions;
use crate::layout::{instance_path, repo_root};
use crate::official::{confirm, Official, VerifierPool};

/// One file that passed both checks.
#[derive(Debug, Clone)]
pub struct Side {
    pub label: String,
    pub path: PathBuf,
    pub solution: Solution,
}

impl Side {
    pub fn objective(&self) -> u64 {
        self.solution.objective()
    }

    pub fn lengths(&self) -> Vec<u64> {
        self.solution.routes().iter().map(|r| r.length()).collect()
    }
}

/// Two checked solutions of one instance.
#[derive(Debug, Clone)]
pub struct Pair {
    pub problem: Problem,
    pub lower_bound: u64,
    pub a: Side,
    pub b: Side,
}

impl Pair {
    pub fn uid(&self) -> &str {
        self.problem.uid()
    }

    /// What one side scores against the lower bound: `(bound / longest)^2`.
    pub fn points(&self, side: &Side) -> f64 {
        match side.objective() {
            0 => 0.0,
            objective => (self.lower_bound as f64 / objective as f64).powi(2),
        }
    }

    /// The points B has over A.
    pub fn gain(&self) -> f64 {
        self.points(&self.b) - self.points(&self.a)
    }

    /// One line on which is shorter and by how much.
    pub fn verdict(&self) -> String {
        let (a, b) = (self.a.objective(), self.b.objective());

        match a.cmp(&b) {
            Ordering::Equal => "the same longest tour".to_owned(),
            Ordering::Greater => format!(
                "{} is {:.2}% shorter, {:+.4} points",
                self.b.label,
                percent(a - b, a),
                self.gain()
            ),
            Ordering::Less => format!(
                "{} is {:.2}% shorter, {:+.4} points",
                self.a.label,
                percent(b - a, b),
                -self.gain()
            ),
        }
    }

    /// The comparison as rows: what, A, B, and B against A.
    pub fn table(&self) -> Table {
        let (a, b) = (&self.a, &self.b);
        let tours = |s: &Side| join(&s.lengths());
        let gap = |s: &Side| match self.lower_bound {
            0 => "n/a".to_owned(),
            bound => format!("{:.2}%", percent(s.objective().saturating_sub(bound), bound)),
        };
        let signed = |x: u64, y: u64| format!("{:+}", i128::from(y) - i128::from(x));

        let rows = vec![
            vec![
                "longest tour".to_owned(),
                a.objective().to_string(),
                b.objective().to_string(),
                signed(a.objective(), b.objective()),
            ],
            vec!["tours".to_owned(), tours(a), tours(b), String::new()],
            vec![
                "all tours together".to_owned(),
                a.solution.total_length().to_string(),
                b.solution.total_length().to_string(),
                signed(a.solution.total_length(), b.solution.total_length()),
            ],
            vec!["gap to the lower bound".to_owned(), gap(a), gap(b), String::new()],
            vec![
                "points".to_owned(),
                format!("{:.4}", self.points(a)),
                format!("{:.4}", self.points(b)),
                format!("{:+.4}", self.gain()),
            ],
            vec![
                "official verifier".to_owned(),
                "feasible".to_owned(),
                "feasible".to_owned(),
                String::new(),
            ],
        ];

        Table {
            caption: format!(
                "{}, k = {}, lower bound {}: {}",
                self.uid(),
                self.problem.k(),
                self.lower_bound,
                self.verdict()
            ),
            header: vec![
                String::new(),
                format!("A: {}", a.label),
                format!("B: {}", b.label),
                "B - A".to_owned(),
            ],
            rows,
        }
    }

    /// The whole page: the table, then both tours.
    pub fn page(&self) -> String {
        let panels = [
            Panel::of(&format!("A: {}", self.a.label), &self.problem, &self.a.solution),
            Panel::of(&format!("B: {}", self.b.label), &self.problem, &self.b.solution),
        ];
        let title = format!("{}: {} against {}", self.uid(), self.a.label, self.b.label);

        viewer::page_with(&title, self.problem.k(), &[self.table()], &panels)
    }
}

/// Starts the official verifier, checks both files, writes the page and
/// opens it.
pub fn run(options: &CompareOptions) -> Result<(), String> {
    let official = VerifierPool::start(&repo_root(), &options.instances, 1).map_err(|e| e.to_string())?;
    let pair = check_pair(
        &options.a,
        &options.b,
        labels(&options.a, &options.b),
        &options.instances,
        &official,
    )?;

    print_table(&pair.table());

    let path = viewer::write(
        &options.out.join("view"),
        &format!("compare-{}", pair.uid()),
        &pair.page(),
    )?;
    println!("written {}", path.display());

    if options.open {
        viewer::open(&path)?;
    }
    Ok(())
}

/// Reads both files, checks that they are of the same instance and that
/// each is a valid solution, ours and the official way. Fails, naming
/// every file that is not valid, before anything is compared.
pub fn check_pair(
    a: &Path,
    b: &Path,
    (label_a, label_b): (String, String),
    instances: &Path,
    official: &dyn Official,
) -> Result<Pair, String> {
    let files = [read(a)?, read(b)?];

    let uid = files[0].instance_uid.clone();
    if files[1].instance_uid != uid {
        return Err(format!(
            "not the same instance: {} is {uid}, {} is {}",
            a.display(),
            b.display(),
            files[1].instance_uid
        ));
    }

    let instance = instance_path(instances, &uid);
    let problem = load_problem(&instance).map_err(|e| format!("cannot read {}: {e}", instance.display()))?;

    let checked: Vec<Result<Solution, String>> = files.iter().map(|f| check(&problem, f, official)).collect();
    let invalid: Vec<String> = [a, b]
        .iter()
        .zip(&checked)
        .filter_map(|(path, result)| result.as_ref().err().map(|e| format!("{}: {e}", path.display())))
        .collect();
    if !invalid.is_empty() {
        return Err(format!("not a valid solution:\n  {}", invalid.join("\n  ")));
    }

    let mut solutions = checked.into_iter().flatten();
    let (Some(solution_a), Some(solution_b)) = (solutions.next(), solutions.next()) else {
        unreachable!("both checked above");
    };

    Ok(Pair {
        lower_bound: lower_bound(&problem),
        a: Side {
            label: label_a,
            path: a.to_path_buf(),
            solution: solution_a,
        },
        b: Side {
            label: label_b,
            path: b.to_path_buf(),
            solution: solution_b,
        },
        problem,
    })
}

fn read(path: &Path) -> Result<SolutionFile, String> {
    load_solution_file(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
}

/// One file: its tours, every lawn cell mowed by our count, and the
/// official verifier's verdict with the same longest tour.
fn check(problem: &Problem, file: &SolutionFile, official: &dyn Official) -> Result<Solution, String> {
    let solution = file.to_solution().map_err(|e| e.to_string())?;

    let missed = uncovered_count(problem, solution.routes()).map_err(|e| e.to_string())?;
    if missed > 0 {
        return Err(format!("{missed} lawn cells not mowed"));
    }

    confirm(official, file, solution.objective())?;
    Ok(solution)
}

/// Short names that tell two paths apart: the last part where they differ,
/// the file name when the names differ, else a nearer folder.
pub fn labels(a: &Path, b: &Path) -> (String, String) {
    let names = |p: &Path| -> Vec<String> {
        p.components()
            .rev()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect()
    };
    let (na, nb) = (names(a), names(b));

    let differ = (0..na.len().max(nb.len())).find(|&i| na.get(i) != nb.get(i));
    match differ {
        None => ("A".to_owned(), "B".to_owned()),
        Some(i) => {
            let at = |n: &[String]| n.get(i).cloned().unwrap_or_else(|| ".".to_owned());
            (at(&na), at(&nb))
        }
    }
}

/// A table in the terminal: its caption, then columns padded to fit.
pub(crate) fn print_table(table: &Table) {
    println!("{}", table.caption);

    let mut rows = vec![table.header.clone()];
    rows.extend(table.rows.iter().cloned());

    let widths: Vec<usize> = (0..table.header.len())
        .map(|c| {
            rows.iter()
                .map(|r| r.get(c).map_or(0, String::len))
                .max()
                .unwrap_or(0)
        })
        .collect();

    for row in &rows {
        let cells: Vec<String> = row
            .iter()
            .zip(&widths)
            .enumerate()
            .map(|(c, (cell, &w))| match c {
                0 => format!("{cell:<w$}"),
                _ => format!("{cell:>w$}"),
            })
            .collect();
        println!("  {}", cells.join("   "));
    }
}

fn join(lengths: &[u64]) -> String {
    lengths.iter().map(u64::to_string).collect::<Vec<_>>().join(" / ")
}

fn percent(part: u64, whole: u64) -> f64 {
    100.0 * part as f64 / whole as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_the_file_names_when_they_differ() {
        let (a, b) = labels(Path::new("x/one.json"), Path::new("x/two.json"));
        assert_eq!((a.as_str(), b.as_str()), ("one.json", "two.json"));
    }

    #[test]
    fn labels_are_the_nearest_folder_that_differs() {
        let (a, b) = labels(
            Path::new("data/rust-slide/slide/best/u.solution.json"),
            Path::new("data/rust/best/u.solution.json"),
        );
        assert_eq!((a.as_str(), b.as_str()), ("slide", "rust"));
    }

    #[test]
    fn the_same_path_twice_is_a_and_b() {
        let (a, b) = labels(Path::new("u.json"), Path::new("u.json"));
        assert_eq!((a.as_str(), b.as_str()), ("A", "B"));
    }
}
