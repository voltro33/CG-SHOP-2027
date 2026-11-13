//! From an instance file to a [`Problem`], as `Problem.from_instance` in
//! `solver/model.py`.

use std::path::Path;

use mow_core::{BitGrid, Cutter, ModelError, Problem};

use crate::error::IoError;
use crate::raster::{rasterize_rings, Ring};
use crate::schema::{InstanceFile, PointSequence};

/// Reads, checks and rasterizes one `*.instance.json`.
pub fn load_problem(path: &Path) -> Result<Problem, IoError> {
    let json = crate::read_text(path)?;
    parse_problem(&json).map_err(|e| e.in_file(path))
}

/// The same from the file's text.
pub fn parse_problem(json: &str) -> Result<Problem, IoError> {
    problem_from_file(&InstanceFile::parse(json)?)
}

/// The region with its holes, and the cutter translated so its center is
/// the origin.
pub fn problem_from_file(file: &InstanceFile) -> Result<Problem, IoError> {
    let region = &file.region_to_cover;

    let mut rings = vec![ring(&region.outer_boundary, "outer_boundary".into())?];
    for (index, hole) in region.inner_boundaries.iter().enumerate() {
        rings.push(ring(hole, format!("inner_boundaries[{index}]"))?);
    }
    let region = rasterize_rings(&rings)?;

    let (center_x, center_y) = file.cutter_center;
    let cutter = rasterize_rings(&[ring(&file.cutter, "cutter".into())?])?;
    let cutter = Cutter::new(translated(&cutter, center_x, center_y)?)?;

    Ok(Problem::new(
        &file.instance_uid,
        region,
        cutter,
        file.number_of_cutters,
    )?)
}

fn ring(points: &PointSequence, name: String) -> Result<Ring, IoError> {
    Ring::from_coordinates(&points.x, &points.y).map_err(|reason| IoError::Ring { ring: name, reason })
}

/// Moves the cutter by `-center`, refusing a center so far away that the
/// frame would overflow.
fn translated(cells: &BitGrid, center_x: i32, center_y: i32) -> Result<BitGrid, IoError> {
    let frame = cells.frame();
    let shift = |low: i32, high: i32, center: i32| {
        let by = center.checked_neg()?;
        low.checked_add(by).and(high.checked_add(by)).map(|_| by)
    };

    match (
        shift(frame.x_min, frame.x_max, center_x),
        shift(frame.y_min, frame.y_max, center_y),
    ) {
        (Some(dx), Some(dy)) => Ok(cells.translated(dx, dy)),
        _ => Err(ModelError::OutOfRange("cutter center").into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mow_core::{Bounds, Point};

    fn instance(cutter_center: &str, k: usize) -> String {
        format!(
            r#"{{"content_type":"CGSHOP2027_Instance","instance_uid":"t",
            "region_to_cover":{{"outer_boundary":{{"x":[0,5,5,0],"y":[0,0,4,4]}},
              "inner_boundaries":[{{"x":[1,1,3,3],"y":[1,3,3,1]}}]}},
            "cutter":{{"x":[10,13,13,10],"y":[20,20,23,23]}},
            "cutter_center":{cutter_center},"number_of_cutters":{k}}}"#
        )
    }

    #[test]
    fn builds_region_with_hole_and_centered_cutter() {
        let p = parse_problem(&instance("[11,21]", 2)).unwrap();

        assert_eq!(p.uid(), "t");
        assert_eq!(p.k(), 2);
        assert_eq!(p.num_cells(), 20 - 4);
        assert!(!p.region().contains(Point::new(2, 2)));

        assert_eq!(p.cutter().area(), 9);
        assert_eq!(
            p.cutter().bounds(),
            Bounds {
                x_min: -1,
                y_min: -1,
                x_max: 1,
                y_max: 1
            }
        );
    }

    #[test]
    fn bad_values_are_errors() {
        let too_many = parse_problem(&instance("[11,21]", 4));
        assert!(matches!(
            too_many,
            Err(IoError::Model(ModelError::CutterCount(4)))
        ));

        let bad_hole = instance("[11,21]", 1).replace("[1,1,3,3]", "[1,1,3]");
        let err = parse_problem(&bad_hole).unwrap_err();
        assert!(matches!(err, IoError::Ring { ref ring, .. } if ring == "inner_boundaries[0]"));

        let far = parse_problem(&instance("[-2147483648,0]", 1));
        assert!(matches!(far, Err(IoError::Model(_))));
    }

    #[test]
    fn missing_file_is_a_read_error() {
        let err = load_problem(Path::new("no/such/file.instance.json")).unwrap_err();
        assert!(matches!(err, IoError::Read { .. }));
    }
}
