//! The 31 bench instances against the official Python verifier: mutated
//! stored bests (see `tools/golden/coverage/make_fixtures.py`) must leave
//! exactly as many cells uncovered, and incremental answers must equal a
//! full recheck.
//!
//! The instance and solution files are not checked in; without them the
//! tests say so and pass.

mod common;

use std::collections::BTreeMap;

use common::load::{best_path, problem, recipes, routes, tours, Recipe};
use common::Rng;
use mow_core::{Point, Problem, Route};
use mow_coverage::{uncovered_count, Scratch, VisitCounts};

struct Instance {
    problem: Problem,
    tours: Vec<Vec<Point>>,
}

fn by_instance() -> Option<BTreeMap<String, (Instance, Vec<Recipe>)>> {
    let mut result: BTreeMap<String, (Instance, Vec<Recipe>)> = BTreeMap::new();

    for recipe in recipes() {
        if !result.contains_key(&recipe.uid) {
            let Some(problem) = problem(&recipe.uid) else {
                eprintln!("skipped: no instance file for {}", recipe.uid);
                return None;
            };
            let tours = tours(&best_path(&recipe.uid))?;
            result.insert(recipe.uid.clone(), (Instance { problem, tours }, Vec::new()));
        }

        result.get_mut(&recipe.uid)?.1.push(recipe);
    }

    Some(result)
}

#[test]
fn mutated_bests_match_the_official_verifier() {
    let Some(instances) = by_instance() else {
        return;
    };

    let mut checked = 0;
    for (instance, recipes) in instances.values() {
        for recipe in recipes {
            let routes = routes(&recipe.apply(&instance.tours));
            let got = uncovered_count(&instance.problem, &routes).unwrap();

            assert_eq!(got, recipe.expected, "{recipe:?}");
            checked += 1;
        }
    }

    assert_eq!(instances.len(), 31);
    eprintln!("{checked} mutations match");
}

/// Uncovered count by a full sweep of every position but the stretch
/// `skip..skip + len` of route `r`.
fn recheck_without(problem: &Problem, routes: &[Route], r: usize, skip: usize, len: usize) -> usize {
    let mut cut: Vec<Route> = Vec::new();

    for (i, route) in routes.iter().enumerate() {
        let positions: Vec<Point> = route.positions().collect();
        if i != r {
            cut.push(route.clone());
            continue;
        }

        let n = positions.len();
        let kept = (skip + len..skip + n).map(|j| positions[j % n]);
        cut.extend(kept.map(|p| Route::new(vec![p]).unwrap()));
    }

    uncovered_count(problem, &cut).unwrap()
}

#[test]
fn incremental_answers_equal_a_full_recheck_on_bench_bests() {
    let Some(instances) = by_instance() else {
        return;
    };

    let mut rng = Rng::new(2027);
    let mut scratch = Scratch::default();

    // the smaller half keeps this test fast in debug builds
    for (uid, (instance, _)) in instances
        .iter()
        .filter(|(_, (i, _))| i.problem.num_cells() < 70_000)
    {
        let routes = routes(&instance.tours);
        let counts = VisitCounts::from_routes(&instance.problem, &routes).unwrap();
        assert!(counts.is_covered(), "{uid} best is covered");

        for _ in 0..4 {
            let r = rng.below(routes.len());
            let positions: Vec<Point> = routes[r].positions().collect();
            let skip = rng.below(positions.len());
            let len = (1 + rng.below(40)).min(positions.len());

            let removed = (skip..skip + len).map(|j| positions[j % positions.len()]);
            let lost = counts.cells_lost_if_removed(&mut scratch, removed);

            let expected = recheck_without(&instance.problem, &routes, r, skip, len);
            assert_eq!(lost, expected, "{uid} route {r} skip {skip} len {len}");
        }
    }
}
