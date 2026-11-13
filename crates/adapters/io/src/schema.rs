//! The official JSON files, field for field, as in
//! `cgshop2027_pyutils/schemas`. Unknown fields are ignored, as pydantic does.

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

use crate::error::IoError;

pub const INSTANCE_CONTENT_TYPE: &str = "CGSHOP2027_Instance";
pub const SOLUTION_CONTENT_TYPE: &str = "CGSHOP2027_Solution";

/// A ring as the file stores it: separate `x` and `y` lists, implicitly closed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PointSequence {
    pub x: Vec<i32>,
    pub y: Vec<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolyominoWithHoles {
    pub outer_boundary: PointSequence,
    pub inner_boundaries: Vec<PointSequence>,
}

/// `*.instance.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceFile {
    #[serde(default = "instance_content_type")]
    pub content_type: String,
    pub instance_uid: String,
    pub region_to_cover: PolyominoWithHoles,
    pub cutter: PointSequence,
    pub cutter_center: (i32, i32),
    pub number_of_cutters: usize,
}

/// `*.solution.json`. `meta` is kept verbatim, so a file read and written
/// again comes out byte for byte the same.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolutionFile {
    #[serde(default = "solution_content_type")]
    pub content_type: String,
    pub instance_uid: String,
    pub tours: Vec<PointSequence>,
    #[serde(default = "empty_meta")]
    pub meta: Box<RawValue>,
}

impl InstanceFile {
    pub fn parse(json: &str) -> Result<Self, IoError> {
        let file: Self = serde_json::from_str(json)?;
        check_content_type(&file.content_type, INSTANCE_CONTENT_TYPE)?;

        Ok(file)
    }
}

impl SolutionFile {
    pub fn parse(json: &str) -> Result<Self, IoError> {
        let file: Self = serde_json::from_str(json)?;
        check_content_type(&file.content_type, SOLUTION_CONTENT_TYPE)?;

        Ok(file)
    }

    /// Compact, in field order, as pydantic's `model_dump_json`.
    pub fn to_json(&self) -> Result<String, IoError> {
        Ok(serde_json::to_string(self)?)
    }
}

fn check_content_type(found: &str, expected: &'static str) -> Result<(), IoError> {
    if found == expected {
        return Ok(());
    }

    Err(IoError::ContentType {
        found: found.to_owned(),
        expected,
    })
}

fn instance_content_type() -> String {
    INSTANCE_CONTENT_TYPE.to_owned()
}

fn solution_content_type() -> String {
    SOLUTION_CONTENT_TYPE.to_owned()
}

pub(crate) fn empty_meta() -> Box<RawValue> {
    RawValue::from_string("{}".to_owned()).expect("{} is valid JSON")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_parses_and_ignores_extra_fields() {
        let json = r#"{"instance_uid":"a","extra":1,
            "region_to_cover":{"outer_boundary":{"x":[0,1,1,0],"y":[0,0,1,1]},"inner_boundaries":[]},
            "cutter":{"x":[0,1,1,0],"y":[0,0,1,1]},"cutter_center":[0,0],"number_of_cutters":2}"#;
        let file = InstanceFile::parse(json).unwrap();

        assert_eq!(file.instance_uid, "a");
        assert_eq!(file.content_type, INSTANCE_CONTENT_TYPE);
        assert_eq!(file.number_of_cutters, 2);
    }

    #[test]
    fn wrong_content_type_and_bad_json_are_errors() {
        let json = r#"{"content_type":"CGSHOP2027_Instance","instance_uid":"a","tours":[]}"#;

        assert!(matches!(
            SolutionFile::parse(json),
            Err(IoError::ContentType { .. })
        ));
        assert!(matches!(SolutionFile::parse("{"), Err(IoError::Json(_))));
        assert!(matches!(
            SolutionFile::parse(r#"{"instance_uid":"a","tours":[{"x":[1.5],"y":[0]}]}"#),
            Err(IoError::Json(_))
        ));
    }

    #[test]
    fn solution_writes_compact_in_field_order() {
        let file = SolutionFile::parse(r#"{"instance_uid":"a","tours":[{"x":[0],"y":[1]}]}"#).unwrap();

        assert_eq!(
            file.to_json().unwrap(),
            r#"{"content_type":"CGSHOP2027_Solution","instance_uid":"a","tours":[{"x":[0],"y":[1]}],"meta":{}}"#
        );
    }
}
