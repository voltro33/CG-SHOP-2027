//! `mow compare A B` on 4 x 4 lawns: two files, or two folders matched by
//! name. Every solution is checked before it is compared; with folders an
//! invalid one is left out and the rest still compared.

mod common;

use std::path::{Path, PathBuf};

use mow_cli::compare::folders::compare_folders;
use mow_cli::compare::{check_pair, parse_args, summary_page};
use mow_cli::official::Official;
use mow_core::{Point, PortError, Route, Solution};
use mow_io::{save_solution, SolutionFile};
use mow_verify::Report;

/// Stands in for the official verifier: feasible, with the Rust objective.
struct Agrees;

impl Official for Agrees {
    fn check(&self, file: &SolutionFile) -> Result<Report, PortError> {
        let objective = file.to_solution().map(|s| s.objective()).ok();
        Ok(Report {
            errors: Vec::new(),
            objective,
        })
    }
}

fn tour(uid: &str, corners: &[(i32, i32)]) -> Solution {
    let points = corners.iter().map(|&(x, y)| Point::new(x, y)).collect();
    Solution::new(uid, vec![Route::new(points).unwrap()])
}

/// Row by row: length 18.
fn snake(uid: &str) -> Solution {
    tour(
        uid,
        &[(0, 0), (3, 0), (3, 1), (0, 1), (0, 2), (3, 2), (3, 3), (0, 3)],
    )
}

/// The same rows driven one step past the lawn's edge: length 22.
fn wide(uid: &str) -> Solution {
    tour(
        uid,
        &[(0, 0), (4, 0), (4, 1), (0, 1), (0, 2), (4, 2), (4, 3), (0, 3)],
    )
}

/// Only the first two rows: length 8, half the lawn missed.
fn half(uid: &str) -> Solution {
    tour(uid, &[(0, 0), (3, 0), (3, 1), (0, 1)])
}

/// A temporary folder with 4 x 4 lawns `a` to `d` (square cutters, k = 1).
struct Setup {
    dir: PathBuf,
    instances: PathBuf,
}

fn setup(name: &str) -> Setup {
    let dir = common::temp_dir(name);
    let instances = dir.join("instances");
    std::fs::create_dir_all(&instances).unwrap();
    for uid in ["a", "b", "c", "d"] {
        common::write_instance(&instances, uid, &[(0, 0), (4, 0), (4, 4), (0, 4)], 1, 1);
    }

    Setup { dir, instances }
}

impl Setup {
    fn save(&self, folder: &str, solution: &Solution) -> PathBuf {
        let path = self
            .dir
            .join(folder)
            .join(format!("{}.solution.json", solution.uid()));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        save_solution(&path, solution).unwrap();
        path
    }

    fn pair(&self, a: &Path, b: &Path) -> Result<mow_cli::compare::Pair, String> {
        let labels = mow_cli::compare::pair::labels(a, b);
        check_pair(a, b, labels, &self.instances, &Agrees)
    }
}

#[test]
fn two_valid_files_are_compared() {
    let s = setup("pair-valid");
    let a = s.save("wide/best", &wide("a"));
    let b = s.save("snake/best", &snake("a"));

    let pair = s.pair(&a, &b).unwrap();

    assert_eq!((pair.a.label.as_str(), pair.b.label.as_str()), ("wide", "snake"));
    assert_eq!((pair.a.objective(), pair.b.objective()), (22, 18));
    assert!(pair.gain() > 0.0);
    assert!(
        pair.verdict().starts_with("snake is 18.18% shorter"),
        "{}",
        pair.verdict()
    );

    let html = pair.page();
    assert_eq!(html.matches("<figure>").count(), 2);
    assert!(html.contains("<td>-4</td>"));
    assert!(html.contains("A: wide") && html.contains("B: snake"));
}

#[test]
fn an_invalid_file_stops_a_pair_and_is_named() {
    let s = setup("pair-invalid");
    let a = s.save("good", &snake("a"));
    let b = s.save("bad", &half("a"));

    let error = s.pair(&a, &b).unwrap_err();

    assert!(error.contains("not a valid solution"), "{error}");
    assert!(
        error.contains("bad") && error.contains("8 lawn cells not mowed"),
        "{error}"
    );
    assert!(!error.contains("good"), "{error}");
}

#[test]
fn files_of_two_instances_are_refused() {
    let s = setup("pair-two");
    let a = s.save("x", &snake("a"));
    let b = s.save("y", &snake("b"));

    let error = s.pair(&a, &b).unwrap_err();

    assert!(error.contains("not the same instance"), "{error}");
}

#[test]
fn two_folders_are_matched_by_name_and_invalid_ones_left_out() {
    let s = setup("folders");
    // a: B shorter; b: tied; c: B invalid; d: only in A
    s.save("one/best", &wide("a"));
    s.save("two/best", &snake("a"));
    s.save("one/best", &snake("b"));
    s.save("two/best", &snake("b"));
    s.save("one/best", &snake("c"));
    s.save("two/best", &half("c"));
    s.save("one/best", &snake("d"));

    let (one, two) = (s.dir.join("one/best"), s.dir.join("two/best"));
    let comparison = compare_folders(&one, &two, &s.instances, &Agrees, 2).unwrap();

    assert_eq!(
        (comparison.label_a.as_str(), comparison.label_b.as_str()),
        ("one", "two")
    );
    let uids: Vec<&str> = comparison.pairs.iter().map(|p| p.uid()).collect();
    assert_eq!(uids, ["a", "b"], "B's gains first");

    assert_eq!(comparison.invalid.len(), 1);
    assert_eq!(comparison.invalid[0].name, "c.solution.json");
    assert_eq!(comparison.invalid[0].reason, "B (two): 8 lawn cells not mowed");
    assert_eq!(comparison.only_a, ["d.solution.json"]);
    assert!(comparison.only_b.is_empty());

    let all = &comparison.groups()[0];
    assert_eq!(
        (all.instances, all.b_shorter, all.a_shorter, all.tied),
        (2, 1, 0, 1)
    );
    assert!(all.gain() > 0.0);
    assert!(comparison.verdict().starts_with("2 instances: two shorter on 1"));

    // then by family (these names carry none) and by k, each holding both
    let groups = comparison.groups();
    let names: Vec<(&str, usize)> = groups.iter().map(|g| (g.name.as_str(), g.instances)).collect();
    assert_eq!(names, [("all", 2), ("unknown", 2), ("k = 1", 2)]);

    // one table: every compared instance linked, the left out ones listed
    let html = summary_page(&comparison, ("one/best", "two/best"), "pages");
    assert_eq!(html.matches("<table").count(), 1);
    assert!(html.contains("href=\"pages/a.html\"") && html.contains("href=\"pages/b.html\""));
    assert!(html.contains("invalid: B (two): 8 lawn cells not mowed"));
    assert!(html.contains("only in A: one"));
    assert!(html.contains("data-key=\"k\" data-value=\"1\""), "a card per k");
    assert_eq!(
        html.matches("data-status=\"out\"").count(),
        3,
        "chip and two rows"
    );
}

#[test]
fn folders_with_no_names_in_common_are_refused() {
    let s = setup("folders-apart");
    s.save("one", &snake("a"));
    s.save("two", &snake("b"));

    let error =
        compare_folders(&s.dir.join("one"), &s.dir.join("two"), &s.instances, &Agrees, 1).unwrap_err();

    assert!(error.contains("no solution file names in common"), "{error}");
}

#[test]
fn the_official_verifier_agrees_on_both() {
    let s = setup("pair-official");
    let Some(official) = common::verifier(&s.instances) else {
        return;
    };
    let a = s.save("wide", &wide("a"));
    let b = s.save("snake", &snake("a"));

    let labels = mow_cli::compare::pair::labels(&a, &b);
    let pair = check_pair(&a, &b, labels, &s.instances, &official).unwrap();

    assert_eq!(pair.b.objective(), 18);
}

#[test]
fn compare_takes_exactly_two_paths() {
    let words = |line: &str| line.split_whitespace().map(str::to_owned).collect::<Vec<_>>();

    let options = parse_args(words("x y --no-open --verifiers 2").into_iter()).unwrap();
    assert_eq!(
        (options.a.as_path(), options.b.as_path()),
        (Path::new("x"), Path::new("y"))
    );
    assert_eq!((options.verifiers, options.open), (2, false));
    assert!(parse_args(words("x y --pages 3").into_iter()).is_err());

    assert!(parse_args(words("x").into_iter()).is_err());
    assert!(parse_args(words("").into_iter()).is_err());
    assert!(parse_args(words("x y --no-verify").into_iter()).is_err());
}

#[test]
fn a_file_and_a_folder_are_refused() {
    let s = setup("mixed");
    let file = s.save("one", &snake("a"));
    let options = parse_args(
        [
            file.to_string_lossy().into_owned(),
            s.dir.join("one").to_string_lossy().into_owned(),
        ]
        .into_iter(),
    )
    .unwrap();

    let error = mow_cli::compare::run(&options).unwrap_err();

    assert!(error.contains("not one of each"), "{error}");
}
