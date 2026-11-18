//! Two folders of solutions, matched by file name.
//!
//! A solution's file name is its instance (`<uid>.solution.json`), so the
//! same name in both folders is the same instance. Each matched pair is
//! checked and compared as two files are. A pair with an invalid solution
//! is reported and left out of the totals; names in only one folder are
//! listed and not compared.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use mow_report::family::family;
use mow_tui::viewer::{self, Table};

use super::page::summary_page;
use super::pair::{check_pair, labels, print_table, Pair};
use super::CompareOptions;
use crate::layout::repo_root;
use crate::official::{Official, VerifierPool};

const SUFFIX: &str = ".solution.json";

/// How many instances the terminal lists.
const SHOWN: usize = 10;

/// A matched name that was not compared, and why.
#[derive(Debug, Clone, PartialEq)]
pub struct Skipped {
    pub name: String,
    pub reason: String,
}

/// Totals over some of the compared instances.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    pub name: String,
    pub instances: usize,
    pub points_a: f64,
    pub points_b: f64,
    pub b_shorter: usize,
    pub a_shorter: usize,
    pub tied: usize,
}

impl Group {
    fn of<'a>(name: &str, pairs: impl Iterator<Item = &'a Pair>) -> Self {
        let mut group = Self {
            name: name.to_owned(),
            instances: 0,
            points_a: 0.0,
            points_b: 0.0,
            b_shorter: 0,
            a_shorter: 0,
            tied: 0,
        };

        for pair in pairs {
            group.instances += 1;
            group.points_a += pair.points(&pair.a);
            group.points_b += pair.points(&pair.b);

            match pair.b.objective().cmp(&pair.a.objective()) {
                std::cmp::Ordering::Less => group.b_shorter += 1,
                std::cmp::Ordering::Greater => group.a_shorter += 1,
                std::cmp::Ordering::Equal => group.tied += 1,
            }
        }

        group
    }

    pub fn gain(&self) -> f64 {
        self.points_b - self.points_a
    }
}

/// What two folders hold against each other.
#[derive(Debug, Clone)]
pub struct FolderComparison {
    pub label_a: String,
    pub label_b: String,
    /// The valid pairs, B's largest gains first.
    pub pairs: Vec<Pair>,
    pub invalid: Vec<Skipped>,
    pub only_a: Vec<String>,
    pub only_b: Vec<String>,
}

impl FolderComparison {
    /// All instances, then by cutter family, then by k.
    pub fn groups(&self) -> Vec<Group> {
        let mut groups = vec![Group::of("all", self.pairs.iter())];

        let families: BTreeSet<&str> = self.pairs.iter().map(|p| family(p.uid())).collect();
        for name in families {
            groups.push(Group::of(
                name,
                self.pairs.iter().filter(|p| family(p.uid()) == name),
            ));
        }

        let ks: BTreeSet<usize> = self.pairs.iter().map(|p| p.problem.k()).collect();
        for k in ks {
            groups.push(Group::of(
                &format!("k = {k}"),
                self.pairs.iter().filter(|p| p.problem.k() == k),
            ));
        }

        groups
    }

    /// One line on the whole comparison.
    pub fn verdict(&self) -> String {
        let all = Group::of("all", self.pairs.iter());

        format!(
            "{} instances: {} shorter on {}, {} shorter on {}, tied on {}; {} - {} = {:+.4} points",
            all.instances,
            self.label_b,
            all.b_shorter,
            self.label_a,
            all.a_shorter,
            all.tied,
            self.label_b,
            self.label_a,
            all.gain()
        )
    }

    fn groups_table(&self) -> Table {
        let rows = self
            .groups()
            .iter()
            .map(|g| {
                vec![
                    g.name.clone(),
                    g.instances.to_string(),
                    format!("{:.3}", g.points_a),
                    format!("{:.3}", g.points_b),
                    format!("{:+.4}", g.gain()),
                    g.b_shorter.to_string(),
                    g.a_shorter.to_string(),
                    g.tied.to_string(),
                ]
            })
            .collect();

        Table {
            caption: "Totals".to_owned(),
            header: [
                "",
                "instances",
                &format!("points A: {}", self.label_a),
                &format!("points B: {}", self.label_b),
                "B - A",
                "B shorter",
                "A shorter",
                "tied",
            ]
            .map(str::to_owned)
            .to_vec(),
            rows,
        }
    }

    /// `pairs` as a table for the terminal.
    fn instances_table(&self, pairs: &[&Pair]) -> Table {
        let rows = pairs
            .iter()
            .map(|p| {
                let (a, b) = (p.a.objective(), p.b.objective());
                let change = 100.0 * (b as f64 - a as f64) / a.max(1) as f64;

                vec![
                    p.uid().to_owned(),
                    a.to_string(),
                    b.to_string(),
                    format!("{:+}", i128::from(b) - i128::from(a)),
                    format!("{change:+.2}%"),
                    format!("{:+.4}", p.gain()),
                ]
            })
            .collect();

        Table {
            caption: format!("The {} largest differences in points", pairs.len()),
            header: [
                "instance",
                "A longest",
                "B longest",
                "B - A",
                "change",
                "points B - A",
            ]
            .map(str::to_owned)
            .to_vec(),
            rows,
        }
    }
}

/// Checks and compares every name the two folders share, on `threads`
/// threads.
pub fn compare_folders(
    a: &Path,
    b: &Path,
    instances: &Path,
    official: &dyn Official,
    threads: usize,
) -> Result<FolderComparison, String> {
    let (names_a, names_b) = (solution_names(a)?, solution_names(b)?);
    let shared: Vec<&String> = names_a.intersection(&names_b).collect();
    if shared.is_empty() {
        return Err(format!(
            "no solution file names in common between {} and {}",
            a.display(),
            b.display()
        ));
    }

    let (label_a, label_b) = labels(a, b);
    let results = check_all(&shared, a, b, (&label_a, &label_b), instances, official, threads);

    let mut pairs = Vec::new();
    let mut invalid = Vec::new();
    for (name, result) in shared.iter().zip(results) {
        match result {
            Ok(pair) => pairs.push(pair),
            Err(reason) => invalid.push(Skipped {
                name: (*name).clone(),
                reason,
            }),
        }
    }
    pairs.sort_by(|x, y| y.gain().total_cmp(&x.gain()).then_with(|| x.uid().cmp(y.uid())));

    Ok(FolderComparison {
        label_a,
        label_b,
        pairs,
        invalid,
        only_a: names_a.difference(&names_b).cloned().collect(),
        only_b: names_b.difference(&names_a).cloned().collect(),
    })
}

/// Every `*.solution.json` in `dir`, by file name.
fn solution_names(dir: &Path) -> Result<BTreeSet<String>, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("cannot list {}: {e}", dir.display()))?;

    Ok(entries
        .filter_map(Result::ok)
        .filter(|e| e.path().is_file())
        .filter_map(|e| e.file_name().to_str().map(str::to_owned))
        .filter(|name| name.ends_with(SUFFIX))
        .collect())
}

/// `check_pair` on every shared name; invalid ones are printed loudly as
/// they are found.
fn check_all(
    names: &[&String],
    a: &Path,
    b: &Path,
    (label_a, label_b): (&str, &str),
    instances: &Path,
    official: &dyn Official,
    threads: usize,
) -> Vec<Result<Pair, String>> {
    let next = AtomicUsize::new(0);
    let slots: Vec<Mutex<Option<Result<Pair, String>>>> = names.iter().map(|_| Mutex::new(None)).collect();

    std::thread::scope(|scope| {
        for _ in 0..threads.max(1) {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(name) = names.get(index) else { break };

                let (in_a, in_b) = (a.join(name), b.join(name));
                let labels = (label_a.to_owned(), label_b.to_owned());
                let result = check_pair(&in_a, &in_b, labels, instances, official)
                    .map_err(|reason| short_reason(&reason, (&in_a, &in_b), (label_a, label_b)));
                if let Err(reason) = &result {
                    eprintln!("!!! INVALID {name}, left out: {reason}");
                }

                *slots[index].lock().unwrap_or_else(|p| p.into_inner()) = Some(result);
            });
        }
    });

    slots
        .into_iter()
        .map(|slot| {
            slot.into_inner()
                .unwrap_or_else(|p| p.into_inner())
                .unwrap_or_else(|| Err("not checked".to_owned()))
        })
        .collect()
}

/// `check_pair`'s error on one line, each file named by its side and label
/// instead of its path: `B (two): 8 lawn cells not mowed`.
fn short_reason(reason: &str, (a, b): (&Path, &Path), (label_a, label_b): (&str, &str)) -> String {
    reason
        .trim_start_matches("not a valid solution:")
        .trim()
        .replace(&a.display().to_string(), &format!("A ({label_a})"))
        .replace(&b.display().to_string(), &format!("B ({label_b})"))
        .replace("\n  ", "; ")
}

/// Compares the folders, prints the totals, writes a side by side page for
/// every compared instance and the summary page linking them, and opens
/// the summary. Fails when any solution was invalid, after writing
/// everything.
pub fn run(options: &CompareOptions) -> Result<(), String> {
    let verifiers = options.verifiers.max(1);
    let official =
        VerifierPool::start(&repo_root(), &options.instances, verifiers).map_err(|e| e.to_string())?;
    let comparison = compare_folders(
        &options.a,
        &options.b,
        &options.instances,
        &official,
        verifiers * 2,
    )?;

    print_summary(&comparison);

    let path = write_pages(options, &comparison, verifiers * 2)?;
    println!("written {}", path.display());

    if options.open {
        viewer::open(&path)?;
    }

    match comparison.invalid.len() {
        0 => Ok(()),
        n => Err(format!("!!! {n} invalid solutions left out of the comparison")),
    }
}

fn print_summary(comparison: &FolderComparison) {
    println!("{}", comparison.verdict());
    print_table(&comparison.groups_table());

    let mut biggest: Vec<&Pair> = comparison.pairs.iter().filter(|p| p.gain() != 0.0).collect();
    biggest.sort_by(|x, y| y.gain().abs().total_cmp(&x.gain().abs()));
    biggest.truncate(SHOWN);
    if !biggest.is_empty() {
        print_table(&comparison.instances_table(&biggest));
    }

    for skipped in &comparison.invalid {
        println!("  invalid, left out: {}: {}", skipped.name, skipped.reason);
    }
    for (label, names) in [
        (&comparison.label_a, &comparison.only_a),
        (&comparison.label_b, &comparison.only_b),
    ] {
        if !names.is_empty() {
            println!("  only in {label}: {} files, not compared", names.len());
        }
    }
}

/// A side by side page for every compared instance, on `threads` threads,
/// in a folder next to the summary page; then the summary. Returns the
/// summary's path.
fn write_pages(
    options: &CompareOptions,
    comparison: &FolderComparison,
    threads: usize,
) -> Result<PathBuf, String> {
    let view = options.out.join("view");
    let name = mow_store::safe_name(&format!(
        "compare-{}-vs-{}",
        comparison.label_a, comparison.label_b
    ));
    let folder = view.join(&name);

    let next = AtomicUsize::new(0);
    let failed: Mutex<Vec<String>> = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..threads.max(1) {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(pair) = comparison.pairs.get(index) else {
                    break;
                };

                if let Err(error) = viewer::write(&folder, pair.uid(), &pair.page()) {
                    failed.lock().unwrap_or_else(|p| p.into_inner()).push(error);
                }
            });
        }
    });

    let failed = failed.into_inner().unwrap_or_else(|p| p.into_inner());
    if let Some(first) = failed.first() {
        return Err(format!("{} pages not written, the first: {first}", failed.len()));
    }

    let paths = (options.a.display().to_string(), options.b.display().to_string());
    let html = summary_page(comparison, (&paths.0, &paths.1), &name);
    viewer::write(&view, &name, &html)
}
