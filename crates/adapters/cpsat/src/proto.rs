//! [`CpModel`] as OR-Tools' `CpModelProto` in protobuf JSON, both ways.
//!
//! Field names are the proto's JSON names (`enforcementLiteral`, `boolOr`,
//! `solutionHint`). A literal is a variable index, or `-index - 1` for its
//! negation. Integers are written as JSON numbers, which Python reads
//! exactly however large; protobuf itself prints int64 as strings, so both
//! are read.

use mow_core::{Constraint, ConstraintKind, CpModel, Domain, Linear, Lit, PortError, Var};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

/// The part of `CpModelProto` a [`CpModel`] fills. Unknown fields are
/// rejected, so a constraint this crate cannot hold is never dropped quietly.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub(crate) struct ModelProto {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    variables: Vec<VariableProto>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    constraints: Vec<ConstraintProto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    objective: Option<ObjectiveProto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    solution_hint: Option<HintProto>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct VariableProto {
    #[serde(deserialize_with = "int64s")]
    domain: Vec<i64>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
struct ConstraintProto {
    #[serde(skip_serializing_if = "Vec::is_empty", deserialize_with = "int64s")]
    enforcement_literal: Vec<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    linear: Option<LinearProto>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bool_or: Option<BoolOrProto>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct LinearProto {
    #[serde(deserialize_with = "int64s")]
    vars: Vec<i64>,
    #[serde(deserialize_with = "int64s")]
    coeffs: Vec<i64>,
    #[serde(deserialize_with = "int64s")]
    domain: Vec<i64>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct BoolOrProto {
    #[serde(deserialize_with = "int64s")]
    literals: Vec<i64>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct ObjectiveProto {
    #[serde(skip_serializing_if = "Vec::is_empty", deserialize_with = "int64s")]
    vars: Vec<i64>,
    #[serde(skip_serializing_if = "Vec::is_empty", deserialize_with = "int64s")]
    coeffs: Vec<i64>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct HintProto {
    #[serde(deserialize_with = "int64s")]
    vars: Vec<i64>,
    #[serde(deserialize_with = "int64s")]
    values: Vec<i64>,
}

/// An int64 as protobuf JSON may write it.
#[derive(Deserialize)]
#[serde(untagged)]
enum Int64 {
    Number(i64),
    Text(String),
}

/// A list of int64, each a JSON number or a decimal string.
pub(crate) fn int64s<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<i64>, D::Error> {
    Vec::<Int64>::deserialize(deserializer)?
        .into_iter()
        .map(|value| match value {
            Int64::Number(n) => Ok(n),
            Int64::Text(text) => text
                .parse()
                .map_err(|e| D::Error::custom(format!("int64 {text:?}: {e}"))),
        })
        .collect()
}

/// The model as one line of protobuf JSON.
pub fn to_proto_json(model: &CpModel) -> String {
    serde_json::to_string(&ModelProto::from_model(model)).expect("plain numbers always serialize")
}

/// Reads protobuf JSON back into a model; anything a [`CpModel`] cannot
/// hold (another constraint kind, a domain of several intervals) is an error.
pub fn from_proto_json(json: &str) -> Result<CpModel, PortError> {
    let proto: ModelProto = serde_json::from_str(json).map_err(|e| bad(e.to_string()))?;

    proto.into_model()
}

impl ModelProto {
    pub(crate) fn from_model(model: &CpModel) -> Self {
        let variables = model
            .domains
            .iter()
            .map(|domain| VariableProto {
                domain: vec![domain.lo, domain.hi],
            })
            .collect();

        let constraints = model
            .constraints
            .iter()
            .map(ConstraintProto::from_constraint)
            .collect();

        let objective = model.objective.as_ref().map(|objective| {
            let (vars, coeffs) = split(objective);
            ObjectiveProto { vars, coeffs }
        });

        let solution_hint = (!model.hint.is_empty()).then(|| HintProto {
            vars: model.hint.iter().map(|&(var, _)| index(var)).collect(),
            values: model.hint.iter().map(|&(_, value)| value).collect(),
        });

        Self {
            variables,
            constraints,
            objective,
            solution_hint,
        }
    }

    fn into_model(self) -> Result<CpModel, PortError> {
        let domains = self
            .variables
            .into_iter()
            .map(|variable| match variable.domain[..] {
                [lo, hi] => Ok(Domain { lo, hi }),
                ref other => Err(bad(format!("domain {other:?} is not one interval"))),
            })
            .collect::<Result<_, _>>()?;

        let constraints = self
            .constraints
            .into_iter()
            .map(ConstraintProto::into_constraint)
            .collect::<Result<_, _>>()?;

        let objective = self
            .objective
            .map(|objective| linear(objective.vars, objective.coeffs))
            .transpose()?;

        let hint = match self.solution_hint {
            Some(hint) => pairs(hint.vars, hint.values)?,
            None => Vec::new(),
        };

        Ok(CpModel {
            domains,
            constraints,
            objective,
            hint,
        })
    }
}

impl ConstraintProto {
    fn from_constraint(constraint: &Constraint) -> Self {
        let enforcement_literal = constraint.enforcement.iter().copied().map(literal).collect();

        let (linear, bool_or) = match &constraint.kind {
            ConstraintKind::Linear { expr, domain } => {
                let (vars, coeffs) = split(expr);
                let domain = vec![domain.lo, domain.hi];
                (Some(LinearProto { vars, coeffs, domain }), None)
            }
            ConstraintKind::BoolOr(literals) => {
                let literals = literals.iter().copied().map(literal).collect();
                (None, Some(BoolOrProto { literals }))
            }
        };

        Self {
            enforcement_literal,
            linear,
            bool_or,
        }
    }

    fn into_constraint(self) -> Result<Constraint, PortError> {
        let enforcement = literals(self.enforcement_literal)?;

        let kind = match (self.linear, self.bool_or) {
            (Some(proto), None) => {
                let domain = match proto.domain[..] {
                    [lo, hi] => Domain { lo, hi },
                    ref other => return Err(bad(format!("domain {other:?} is not one interval"))),
                };
                let expr = linear(proto.vars, proto.coeffs)?;
                ConstraintKind::Linear { expr, domain }
            }
            (None, Some(proto)) => ConstraintKind::BoolOr(literals(proto.literals)?),
            _ => {
                return Err(bad(
                    "a constraint needs exactly one of linear and boolOr".to_owned()
                ))
            }
        };

        Ok(Constraint { enforcement, kind })
    }
}

/// `var` as the proto numbers it.
fn index(var: Var) -> i64 {
    i64::from(var.0)
}

/// A literal as the proto writes it: `-index - 1` when negated.
pub(crate) fn literal(lit: Lit) -> i64 {
    match lit.negated {
        true => -index(lit.var) - 1,
        false => index(lit.var),
    }
}

fn split(linear: &Linear) -> (Vec<i64>, Vec<i64>) {
    let vars = linear.terms.iter().map(|&(var, _)| index(var)).collect();
    let coeffs = linear.terms.iter().map(|&(_, coeff)| coeff).collect();

    (vars, coeffs)
}

fn var(reference: i64) -> Result<Var, PortError> {
    u32::try_from(reference)
        .map(Var)
        .map_err(|_| bad(format!("{reference} is not a variable index")))
}

fn literals(references: Vec<i64>) -> Result<Vec<Lit>, PortError> {
    references
        .into_iter()
        .map(|reference| match reference < 0 {
            true => Ok(!Lit::from(var(-(reference + 1))?)),
            false => Ok(Lit::from(var(reference)?)),
        })
        .collect()
}

fn linear(vars: Vec<i64>, coeffs: Vec<i64>) -> Result<Linear, PortError> {
    pairs(vars, coeffs).map(Linear::new)
}

/// Zips variable indices with their numbers, which must be as many.
fn pairs(vars: Vec<i64>, numbers: Vec<i64>) -> Result<Vec<(Var, i64)>, PortError> {
    if vars.len() != numbers.len() {
        return Err(bad(format!(
            "{} variables for {} numbers",
            vars.len(),
            numbers.len()
        )));
    }

    vars.into_iter()
        .zip(numbers)
        .map(|(reference, number)| Ok((var(reference)?, number)))
        .collect()
}

fn bad(message: String) -> PortError {
    PortError::External {
        context: "CpModelProto JSON".to_owned(),
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `x in [0, 5]`, `b` boolean, `x >= 2` unless `b`, `b or not b`,
    /// minimize `x + 3b`, hint `x = 3`.
    fn small() -> CpModel {
        let mut model = CpModel::new();
        let x = model.new_int(0, 5);
        let b = model.new_bool();

        model
            .add_linear(Linear::sum([x]), 2, i64::MAX)
            .only_enforce_if(!Lit::from(b));
        model.add_bool_or(vec![Lit::from(b), !Lit::from(b)]);
        model.minimize(Linear::new(vec![(x, 1), (b, 3)]));
        model.add_hint(x, 3);

        model
    }

    #[test]
    fn a_hand_built_model_is_the_expected_json() {
        let expected = concat!(
            r#"{"variables":[{"domain":[0,5]},{"domain":[0,1]}],"#,
            r#""constraints":[{"enforcementLiteral":[-2],"linear":{"vars":[0],"coeffs":[1],"domain":[2,9223372036854775807]}},"#,
            r#"{"boolOr":{"literals":[1,-2]}}],"#,
            r#""objective":{"vars":[0,1],"coeffs":[1,3]},"#,
            r#""solutionHint":{"vars":[0],"values":[3]}}"#,
        );

        assert_eq!(to_proto_json(&small()), expected);
    }

    #[test]
    fn the_empty_model_is_an_empty_object() {
        assert_eq!(to_proto_json(&CpModel::new()), "{}");
        assert_eq!(from_proto_json("{}").unwrap(), CpModel::new());
    }

    #[test]
    fn round_trips() {
        let mut model = small();
        let big = model.new_int(i64::MIN, i64::MAX);
        model.add_linear(Linear::new(vec![(big, (1 << 53) + 1)]), i64::MIN, -(1 << 60));
        model.add_hint(big, -(1 << 53) - 1);

        let json = to_proto_json(&model);
        assert!(json.contains("9007199254740993"), "{json}");

        assert_eq!(from_proto_json(&json).unwrap(), model);
    }

    #[test]
    fn int64_strings_are_read_as_protobuf_prints_them() {
        // what json_format.MessageToJson makes of small()
        let json = r#"{"variables": [{"domain": ["0", "5"]}, {"domain": ["0", "1"]}],
            "constraints": [{"enforcementLiteral": [-2], "linear": {"vars": [0], "coeffs": ["1"],
            "domain": ["2", "9223372036854775807"]}}, {"boolOr": {"literals": [1, -2]}}],
            "objective": {"vars": [0, 1], "coeffs": ["1", "3"]},
            "solutionHint": {"vars": [0], "values": ["3"]}}"#;

        assert_eq!(from_proto_json(json).unwrap(), small());

        let min = r#"{"variables":[{"domain":["-9223372036854775808","9007199254740993"]}]}"#;
        let model = from_proto_json(min).unwrap();
        assert_eq!(
            model.domains[0],
            Domain {
                lo: i64::MIN,
                hi: (1 << 53) + 1
            }
        );
    }

    #[test]
    fn literals_are_negative_when_negated() {
        assert_eq!(literal(Lit::from(Var(0))), 0);
        assert_eq!(literal(!Lit::from(Var(0))), -1);
        assert_eq!(literal(!Lit::from(Var(7))), -8);

        assert_eq!(
            literals(vec![-1, 3, -8]).unwrap(),
            vec![!Lit::from(Var(0)), Lit::from(Var(3)), !Lit::from(Var(7))]
        );
    }

    #[test]
    fn what_a_model_cannot_hold_is_an_error() {
        let rejected = [
            r#"{"constraints":[{"atMostOne":{"literals":[0]}}]}"#,
            r#"{"constraints":[{}]}"#,
            r#"{"variables":[{"domain":[0,1,3,4]}]}"#,
            r#"{"constraints":[{"linear":{"vars":[-1],"coeffs":[1],"domain":[0,1]}}]}"#,
            r#"{"objective":{"vars":[0,1],"coeffs":[1]}}"#,
            r#"{"variables":[{"domain":["0x1","2"]}]}"#,
            r#"{"name":"m"}"#,
        ];

        for json in rejected {
            assert!(from_proto_json(json).is_err(), "{json}");
        }
    }
}
