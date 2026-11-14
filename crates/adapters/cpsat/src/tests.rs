//! Solves of small models through Python. The ones that need python (and
//! OR-Tools in it) skip without it.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use mow_core::{CpModel, CpParams, CpSolver, CpStatus, Linear, Lit, PortError, Var};

use crate::{python_in, CpSatPool, SCRIPT};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn pool(size: usize) -> Option<CpSatPool> {
    let root = root();
    if !python_in(&root).is_file() {
        println!("skipped: no python in {}", root.display());
        return None;
    }

    Some(CpSatPool::from_repo(&root, size).unwrap())
}

/// One second and one thread: every model here is solved at once.
fn params() -> CpParams {
    CpParams {
        time_limit: Duration::from_secs(1),
        workers: 1,
    }
}

/// `x + y` minimized subject to `x + 2y >= 7`, both in `[0, 10]`.
fn knapsack() -> (CpModel, Var, Var) {
    let mut model = CpModel::new();
    let x = model.new_int(0, 10);
    let y = model.new_int(0, 10);

    model.add_linear(Linear::new(vec![(x, 1), (y, 2)]), 7, i64::MAX);
    model.minimize(Linear::sum([x, y]));

    (model, x, y)
}

#[test]
fn missing_script_and_python_are_errors() {
    let python = python_in(&root());

    let script = CpSatPool::new(&python, "no/such/cpsat.py", 1);
    assert!(matches!(script, Err(PortError::NotFound(m)) if m.contains("script")));

    let result = CpSatPool::new("no/such/python.exe", root().join(SCRIPT), 1);
    assert!(matches!(result, Err(PortError::External { message, .. }) if message.contains("python")));
}

#[test]
fn an_optimum() {
    let Some(pool) = pool(1) else { return };
    let (model, x, y) = knapsack();

    let response = pool.solve(&model, &params()).unwrap();

    assert_eq!(response.status, CpStatus::Optimal);
    assert_eq!(response.objective, 4.0);
    assert_eq!(response.values.len(), 2);
    assert_eq!(response.value(x).unwrap() + 2 * response.value(y).unwrap(), 7);
}

#[test]
fn enforcement_literals() {
    let Some(pool) = pool(1) else { return };

    // x >= 5 unless b, x <= 1 if b; b costs 10, x costs 1: b off is cheaper
    let mut model = CpModel::new();
    let x = model.new_int(0, 9);
    let b = model.new_bool();

    model
        .add_linear(Linear::sum([x]), 5, i64::MAX)
        .only_enforce_if(!Lit::from(b));
    model.add_linear(Linear::sum([x]), i64::MIN, 1).only_enforce_if(b);
    model.minimize(Linear::new(vec![(x, 1), (b, 10)]));

    let response = pool.solve(&model, &params()).unwrap();
    assert_eq!((response.value(x), response.value(b)), (Some(5), Some(0)));

    // with b forced on, only the enforced upper bound holds
    model.add_linear(Linear::sum([b]), 1, 1);
    let response = pool.solve(&model, &params()).unwrap();
    assert_eq!((response.value(x), response.value(b)), (Some(0), Some(1)));
    assert_eq!(response.objective, 10.0);
}

#[test]
fn bool_or() {
    let Some(pool) = pool(1) else { return };

    // a or not b, b forced: a has to be on, whatever it costs
    let mut model = CpModel::new();
    let a = model.new_bool();
    let b = model.new_bool();

    model.add_bool_or(vec![Lit::from(a), !Lit::from(b)]);
    model.add_linear(Linear::sum([b]), 1, 1);
    model.minimize(Linear::new(vec![(a, 3)]));

    let response = pool.solve(&model, &params()).unwrap();
    assert_eq!(response.status, CpStatus::Optimal);
    assert_eq!(response.values, vec![1, 1]);
    assert_eq!(response.objective, 3.0);
}

#[test]
fn a_hint_leaves_the_answer_alone() {
    let Some(pool) = pool(1) else { return };

    // presolve may settle a small model before it looks at the hint, so only
    // the answer is checked here; python_reads_the_model_as_sent checks that
    // the hint arrives
    for hint in [(1, 3), (10, 10), (0, 0)] {
        let (mut model, x, y) = knapsack();
        model.add_hint(x, hint.0);
        model.add_hint(y, hint.1);

        let response = pool.solve(&model, &params()).unwrap();
        assert_eq!(response.status, CpStatus::Optimal, "{hint:?}");
        assert_eq!(response.objective, 4.0, "{hint:?}");
    }

    // nothing to minimize: any solution, objective 0
    let mut model = CpModel::new();
    let x = model.new_int(0, 10);
    let y = model.new_int(0, 10);
    model.add_linear(Linear::sum([x, y]), 10, 10);
    model.add_hint(x, 7);

    let response = pool.solve(&model, &params()).unwrap();
    assert!(response.status.found());
    assert_eq!(response.values.iter().sum::<i64>(), 10);
    assert_eq!(response.objective, 0.0);
}

#[test]
fn infeasible_and_invalid() {
    let Some(pool) = pool(1) else { return };

    let (mut model, x, _) = knapsack();
    model.add_linear(Linear::sum([x]), 20, 30);
    let response = pool.solve(&model, &params()).unwrap();
    assert_eq!(response.status, CpStatus::Infeasible);
    assert!(response.values.is_empty());

    // a variable that is not in the model: the proto holds it, CP-SAT does not
    let (mut model, _, _) = knapsack();
    model.add_linear(Linear::sum([Var(5)]), 0, 1);
    let response = pool.solve(&model, &params()).unwrap();
    assert_eq!(response.status, CpStatus::ModelInvalid);
    assert!(response.values.is_empty());
}

#[test]
fn a_request_python_cannot_read_is_an_error_and_the_process_lives_on() {
    let Some(pool) = pool(1) else { return };

    // proto variable references are int32
    let mut model = CpModel::new();
    model.new_bool();
    model.add_bool_or(vec![Lit::from(Var(u32::MAX))]);

    let result = pool.solve(&model, &params());
    assert!(
        matches!(&result, Err(PortError::External { message, .. }) if message.contains("bad model")),
        "{result:?}"
    );

    let (model, _, _) = knapsack();
    assert_eq!(pool.solve(&model, &params()).unwrap().status, CpStatus::Optimal);
}

#[test]
fn python_reads_the_model_as_sent() {
    let Some(pool) = pool(1) else { return };

    let (mut model, x, y) = knapsack();
    let on = model.new_bool();
    model
        .add_linear(Linear::new(vec![(x, (1 << 53) + 1), (y, -1)]), i64::MIN, i64::MAX)
        .only_enforce_if(!Lit::from(on));
    model.add_bool_or(vec![Lit::from(on), !Lit::from(x)]);
    model.add_hint(on, 1);

    assert_eq!(pool.echo(&model).unwrap(), model);
}

#[test]
fn a_dead_process_is_restarted() {
    let Some(pool) = pool(2) else { return };
    let (model, _, _) = knapsack();

    pool.kill_workers();
    for _ in 0..3 {
        assert_eq!(pool.solve(&model, &params()).unwrap().status, CpStatus::Optimal);
    }
}

#[test]
fn shared_between_threads() {
    let Some(pool) = pool(2) else { return };
    let (model, _, _) = knapsack();

    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| {
                for _ in 0..5 {
                    let response = pool.solve(&model, &params()).unwrap();
                    assert_eq!(response.objective, 4.0);
                }
            });
        }
    });
}

#[test]
fn round_trip_overhead() {
    let Some(pool) = pool(1) else { return };
    let (model, _, _) = knapsack();

    // the first solve waits for python to import OR-Tools
    let start = Instant::now();
    pool.solve(&model, &params()).unwrap();
    println!("first solve: {:?}", start.elapsed());

    let start = Instant::now();
    for _ in 0..50 {
        pool.solve(&model, &params()).unwrap();
    }
    println!("warm solve: {:?} each", start.elapsed() / 50);
}
