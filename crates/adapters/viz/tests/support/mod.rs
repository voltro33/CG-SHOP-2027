//! Small problems built by hand, and a minimal check that SVG text is well formed.

#![allow(dead_code)]

use mow_core::{BitGrid, Bounds, Cutter, Point, Problem};

/// A `width x height` rectangle of cells at the origin without the cells in
/// `holes`, mowed by a centered `side x side` square cutter.
pub fn rectangle(width: i32, height: i32, side: i32, k: usize, holes: &[(i32, i32)]) -> Problem {
    let mut region = BitGrid::empty(bounds(0, 0, width - 1, height - 1)).unwrap();
    for y in 0..height {
        region.insert_run(y, 0, width - 1);
    }
    for &(x, y) in holes {
        region.remove(Point::new(x, y));
    }

    let half = side / 2;
    let mut cutter = BitGrid::empty(bounds(-half, -half, side - 1 - half, side - 1 - half)).unwrap();
    for y in -half..side - half {
        cutter.insert_run(y, -half, side - 1 - half);
    }

    Problem::new("rect<&>", region, Cutter::new(cutter).unwrap(), k).unwrap()
}

pub fn bounds(x_min: i32, y_min: i32, x_max: i32, y_max: i32) -> Bounds {
    Bounds {
        x_min,
        y_min,
        x_max,
        y_max,
    }
}

pub fn points(raw: &[(i32, i32)]) -> Vec<Point> {
    raw.iter().map(|&p| p.into()).collect()
}

/// How often `needle` occurs in `text`.
pub fn count(text: &str, needle: &str) -> usize {
    text.matches(needle).count()
}

/// Panics unless every tag is closed in order and the root is `<svg>`.
/// Returns the names of the opened elements, in document order.
pub fn assert_well_formed(svg: &str) -> Vec<String> {
    let mut stack: Vec<String> = Vec::new();
    let mut opened = Vec::new();
    let mut rest = svg;

    while let Some(start) = rest.find('<') {
        let end = rest[start..].find('>').expect("a tag is not closed") + start;
        let tag = &rest[start + 1..end];
        rest = &rest[end + 1..];

        if tag.starts_with('?') || tag.starts_with('!') {
            continue;
        }

        if let Some(name) = tag.strip_prefix('/') {
            assert_eq!(stack.pop().as_deref(), Some(name), "mismatched </{name}>");
            continue;
        }

        let name: String = tag
            .chars()
            .take_while(|c| !c.is_whitespace() && *c != '/')
            .collect();
        assert!(!tag.contains('<'), "raw '<' inside a tag: {tag}");
        opened.push(name.clone());
        if !tag.ends_with('/') {
            stack.push(name);
        }
    }

    assert!(stack.is_empty(), "unclosed elements: {stack:?}");
    assert_eq!(opened.first().map(String::as_str), Some("svg"));
    assert!(!svg.contains("& "), "an unescaped ampersand");

    opened
}

/// The value of attribute `name` in the first tag containing `marker`.
pub fn attribute<'a>(svg: &'a str, marker: &str, name: &str) -> &'a str {
    let at = svg.find(marker).unwrap_or_else(|| panic!("no tag with {marker}"));
    let tag_start = svg[..at].rfind('<').unwrap();
    let tag_end = at + svg[at..].find('>').unwrap();
    let tag = &svg[tag_start..tag_end];

    let key = format!(" {name}=\"");
    let value_start = tag.find(&key).unwrap_or_else(|| panic!("no {name} in {tag}")) + key.len();
    let value_end = value_start + tag[value_start..].find('"').unwrap();

    &tag[value_start..value_end]
}
