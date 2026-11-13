mod support;

use mow_core::{lower_bound, BitGrid, Order, Route, Segment, Visit};
use mow_viz::{palette, Scene};
use support::{assert_well_formed, attribute, bounds, count, points, rectangle};

fn route(raw: &[(i32, i32)]) -> Route {
    Route::new(points(raw)).unwrap()
}

#[test]
fn region_alone_is_well_formed_and_draws_one_merged_rectangle() {
    let problem = rectangle(10, 4, 3, 1, &[]);
    let svg = Scene::new(&problem).to_svg();

    let elements = assert_well_formed(&svg);

    assert_eq!(count(&svg, "class=\"region\""), 1);
    assert_eq!(count(attribute(&svg, "class=\"region\"", "d"), "M"), 1);
    assert!(!elements.is_empty());
    assert_eq!(count(&svg, "class=\"route\""), 0);
}

#[test]
fn holes_split_the_region_into_more_rectangles() {
    let problem = rectangle(10, 4, 3, 1, &[(3, 1)]);
    let svg = Scene::new(&problem).to_svg();

    assert_well_formed(&svg);
    assert_eq!(count(attribute(&svg, "class=\"region\"", "d"), "M"), 4);
}

#[test]
fn y_grows_upward() {
    // one column of six cells, a 1x1 cutter, a route going straight up
    let problem = rectangle(1, 6, 1, 1, &[]);
    let svg = Scene::new(&problem).routes(&[route(&[(0, 0), (0, 4)])]).to_svg();

    let d = attribute(&svg, "class=\"route\"", "d");
    let numbers: Vec<i64> = d
        .split(|c: char| !c.is_ascii_digit() && c != '-')
        .filter(|s| !s.is_empty())
        .map(|s| s.parse().unwrap())
        .collect();

    // M x y0 V y1 Z: going up in the world is going down in SVG y
    let (start_y, end_y) = (numbers[1], numbers[2]);
    assert!(d.starts_with('M') && d.contains('V') && d.ends_with('Z'), "{d}");
    assert_eq!(start_y - end_y, 8, "two units per cell: {d}");
}

#[test]
fn one_color_per_route() {
    let problem = rectangle(12, 12, 3, 3, &[]);
    let routes = [
        route(&[(1, 1), (10, 1)]),
        route(&[(1, 5), (10, 5)]),
        route(&[(1, 9), (10, 9)]),
    ];
    let svg = Scene::new(&problem).routes(&routes).to_svg();

    assert_well_formed(&svg);
    assert_eq!(count(&svg, "class=\"route\""), 3);
    assert_eq!(count(&svg, "class=\"start\""), 3);

    let colors: Vec<&str> = (0..3)
        .map(|i| attribute(&svg, &format!("data-cutter=\"{i}\""), "stroke"))
        .collect();
    let expected: Vec<&str> = (0..3).map(palette::route_color).collect();
    assert_eq!(colors, expected);
    assert!(colors[0] != colors[1] && colors[1] != colors[2] && colors[0] != colors[2]);
}

#[test]
fn legend_shows_uid_k_objective_bound_and_gap() {
    let problem = rectangle(30, 9, 3, 1, &[]);
    let routes = [route(&[
        (1, 1),
        (28, 1),
        (28, 4),
        (1, 4),
        (1, 7),
        (28, 7),
        (28, 8),
        (1, 8),
    ])];
    let svg = Scene::new(&problem).routes(&routes).to_svg();

    assert_well_formed(&svg);
    let bound = lower_bound(&problem);
    let objective = routes[0].length();
    let gap = 100.0 * (objective as f64 - bound as f64) / bound as f64;

    assert!(svg.contains("rect&lt;&amp;&gt;"), "uid escaped");
    assert!(svg.contains("k = 1"));
    assert!(svg.contains(&format!("objective {objective}")));
    assert!(svg.contains(&format!("lower bound {bound}")));
    assert!(svg.contains(&format!("gap {gap:.2}%")), "{svg}");
}

#[test]
fn titles_are_escaped() {
    let problem = rectangle(4, 4, 1, 1, &[]);
    let svg = Scene::new(&problem).title("a < b & \"c\"").to_svg();

    assert_well_formed(&svg);
    assert!(svg.contains("a &lt; b &amp; &quot;c&quot;"));
}

#[test]
fn segments_and_order_links() {
    let problem = rectangle(10, 6, 3, 1, &[]);
    let segments = [
        Segment::horizontal(1, 1, 8).unwrap(),
        Segment::horizontal(4, 1, 8).unwrap(),
        Segment::vertical(9, 0, 5).unwrap(),
    ];
    let order = Order::new(
        vec![
            Visit {
                segment: 0,
                reversed: false,
            },
            Visit {
                segment: 1,
                reversed: true,
            },
            Visit {
                segment: 2,
                reversed: false,
            },
        ],
        3,
    )
    .unwrap();
    let svg = Scene::new(&problem).order(&segments, &order).to_svg();

    assert_well_formed(&svg);
    assert_eq!(count(attribute(&svg, "class=\"segments\"", "d"), "M"), 3);
    assert_eq!(count(attribute(&svg, "class=\"links\"", "d"), "M"), 2);
}

#[test]
fn segments_alone_have_no_links() {
    let problem = rectangle(10, 6, 3, 1, &[]);
    let segments = [Segment::horizontal(1, 1, 8).unwrap()];
    let svg = Scene::new(&problem).segments(&segments).to_svg();

    assert_eq!(count(&svg, "class=\"segments\""), 1);
    assert_eq!(count(&svg, "class=\"links\""), 0);
}

#[test]
fn uncovered_cells_only_when_there_are_some() {
    let problem = rectangle(10, 6, 3, 1, &[]);
    let mut uncovered = BitGrid::empty(bounds(0, 0, 9, 5)).unwrap();

    let none = Scene::new(&problem).uncovered(&uncovered).to_svg();
    assert_eq!(count(&none, "class=\"uncovered\""), 0);

    uncovered.insert_run(2, 3, 5);
    uncovered.insert_run(3, 3, 5);
    let some = Scene::new(&problem).uncovered(&uncovered).to_svg();

    assert_well_formed(&some);
    assert_eq!(count(&some, "class=\"uncovered\""), 1);
    assert_eq!(count(attribute(&some, "class=\"uncovered\"", "d"), "M"), 1);
    assert!(some.contains("6 uncovered cells"));
}

#[test]
fn streaming_matches_the_string() {
    let problem = rectangle(8, 8, 3, 2, &[(4, 4)]);
    let routes = [route(&[(1, 1), (6, 1)]), route(&[(1, 6), (6, 6)])];
    let scene = Scene::new(&problem).routes(&routes);

    let mut bytes = Vec::new();
    scene.render(&mut bytes).unwrap();

    assert_eq!(String::from_utf8(bytes).unwrap(), scene.to_svg());
}

#[test]
fn single_corner_routes_still_get_a_start_marker() {
    let problem = rectangle(3, 3, 3, 1, &[]);
    let svg = Scene::new(&problem).routes(&[route(&[(1, 1)])]).to_svg();

    assert_well_formed(&svg);
    assert_eq!(count(&svg, "class=\"start\""), 1);
}
