use super::*;

fn record(uid: &str, label: &str, objective: Option<u64>, seconds: f64) -> JobRecord {
    JobRecord {
        job_id: format!("{uid}|{label}"),
        instance: uid.to_owned(),
        round: -3,
        label: label.to_owned(),
        objective,
        seconds,
        error: None,
    }
}

fn pass(number: u32, objective: u64, seconds: f64) -> Pass {
    Pass {
        number,
        objective,
        seconds,
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-6 * b.abs().max(1.0)
}

#[test]
fn reads_the_pass_of_lns_labels_only() {
    assert_eq!(pass_number("lns/temperature=0.0003/pass=3"), Some(3));
    assert_eq!(pass_number("lns/temperature=0.0003/pass=3/seed=7"), Some(3));
    assert_eq!(pass_number("lns/temperature=0.0003"), Some(0));
    assert_eq!(pass_number("polish/side=40/pass=2"), None);
    assert_eq!(pass_number("lnsx/pass=2"), None);
}

#[test]
fn passes_skip_failures_other_methods_and_the_running_pass() {
    let mut failed = record("a", "lns/temperature=0.0003/pass=2", Some(1), 9.0);
    failed.error = Some("boom".to_owned());
    let records = vec![
        record("a", "lns/temperature=0.0003/pass=3", Some(90), 300.0),
        record("a", "lns/temperature=0.0003/pass=1", Some(100), 60.0),
        failed,
        record("a", "lns/temperature=0.0003/pass=4", Some(80), 300.0),
        record("a", "polish/side=40", Some(70), 300.0),
        record("b", "lns/temperature=0.0003/pass=1", None, 60.0),
    ];

    let found = passes(&records, Some(4));

    assert_eq!(found["a"], vec![pass(1, 100, 60.0), pass(3, 90, 300.0)]);
    assert!(!found.contains_key("b"));
    assert_eq!(passes(&records, None)["a"].len(), 3);
}

#[test]
fn the_gain_rate_spans_the_last_window_of_passes() {
    let history = [
        pass(1, 1000, 60.0),
        pass(2, 900, 300.0),
        pass(3, 880, 300.0),
        pass(4, 864, 300.0),
    ];

    // from pass 2 to 4: 36 of 900 in 600 seconds
    let rate = gain_rate(&history).unwrap();
    assert!(close(rate, 36.0 / 900.0 / 600.0));

    let rate = gain_rate(&history[..2]).unwrap();
    assert!(close(rate, 100.0 / 1000.0 / 300.0));

    assert_eq!(gain_rate(&history[..1]), None);
    assert_eq!(gain_rate(&[pass(1, 9, 0.0), pass(2, 8, 0.0)]), None);
}

#[test]
fn a_worse_pass_is_no_gain_not_a_loss() {
    let rate = gain_rate(&[pass(1, 100, 10.0), pass(2, 120, 10.0)]);

    assert_eq!(rate, Some(0.0));
}

#[test]
fn the_gain_is_on_the_best_so_far_not_on_what_a_pass_reported() {
    // pass 3 reports 95 from its own walk, but the best after it is still 90
    let history = [
        pass(1, 100, 10.0),
        pass(2, 90, 10.0),
        pass(3, 95, 10.0),
        pass(4, 81, 10.0),
    ];

    let rate = gain_rate(&history).unwrap();
    assert!(close(rate, 9.0 / 90.0 / 20.0));

    assert_eq!(gain_rate(&history[..3]), Some(10.0 / 100.0 / 20.0));
}

#[test]
fn weights_floor_stalls_and_fill_in_the_median() {
    let points = [1.0, 0.5, 0.5, 1.0];
    let rates = [Some(4.0), Some(2.0), None, Some(0.0)];

    let found = weights(&points, &rates);

    // the median of 4, 2, 0 is 2: the floor is 0.4
    assert_eq!(found.len(), 4);
    assert!(close(found[0], 4.0));
    assert!(close(found[1], 1.0));
    assert!(close(found[2], 1.0));
    assert!(close(found[3], FLOOR_SHARE * 2.0));
}

#[test]
fn split_keeps_the_total_and_follows_the_weights() {
    let found = split(&[1.0, 2.0, 3.0], 100.0);

    assert!(close(found.iter().sum(), 300.0));
    assert!(close(found[0], 50.0));
    assert!(close(found[1], 100.0));
    assert!(close(found[2], 150.0));
}

#[test]
fn split_clamps_and_shares_out_what_the_clamp_frees() {
    let found = split(&[1000.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0], 100.0);

    assert!(close(found.iter().sum(), 1000.0));
    assert!(close(found[0], LONGEST * 100.0));
    for seconds in &found[1..] {
        assert!(close(*seconds, 500.0 / 9.0));
    }

    let found = split(&[1.0, 1e-9, 1e-9], 100.0);
    assert!(close(found.iter().sum(), 300.0));
    assert!(close(found[1], SHORTEST * 100.0));
    assert!(close(found[0], 300.0 - 2.0 * SHORTEST * 100.0));
}

#[test]
fn split_is_even_when_the_weights_say_nothing() {
    assert_eq!(split(&[0.0, 0.0], 60.0), vec![60.0, 60.0]);
    assert_eq!(split(&[1.0, f64::NAN], 60.0), vec![60.0, 60.0]);
    assert_eq!(split(&[2.0, 2.0], 60.0), vec![60.0, 60.0]);
    assert!(split(&[], 60.0).is_empty());
}

#[test]
fn spread_is_least_median_most() {
    assert_eq!(spread(&[3.0, 1.0, 2.0, 10.0]), Some((1.0, 2.5, 10.0)));
    assert_eq!(spread(&[]), None);
}

#[test]
fn parses_the_allocation() {
    assert_eq!(Allocation::parse("even"), Ok(Allocation::Even));
    assert_eq!(Allocation::parse("weighted"), Ok(Allocation::Weighted));
    assert!(Allocation::parse("random").is_err());
}
