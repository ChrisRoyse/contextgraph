use crate::dynamicjepa::error::{DynamicJepaError, DynamicJepaResult};
use crate::dynamicjepa::validation::{ensure_no_duplicates, Validate};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateSchema {
    pub fields: Vec<FieldSpec>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionSchema {
    pub fields: Vec<FieldSpec>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutcomeSchema {
    pub fields: Vec<FieldSpec>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EntitySchema {
    pub fields: Vec<FieldSpec>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeSchema {
    pub field: FieldSpec,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldSpec {
    pub name: String,
    pub kind: FieldKind,
    pub required: bool,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FieldKind {
    I64,
    F64,
    Bool,
    String,
    Categorical { variants: Vec<String> },
    Vector { dim: usize },
    UnixMs,
}

impl FieldKind {
    pub fn validate(&self, field: &str) -> DynamicJepaResult<()> {
        match self {
            FieldKind::Categorical { variants } => {
                if variants.is_empty() {
                    return Err(DynamicJepaError::schema(
                        field,
                        "categorical variants must not be empty",
                        "declare every allowed variant explicitly",
                    ));
                }
                ensure_no_duplicates(variants.iter().map(String::as_str), field)?;
            }
            FieldKind::Vector { dim } if *dim == 0 => {
                return Err(DynamicJepaError::schema(
                    field,
                    "vector dim must be positive",
                    "declare a strict positive vector dimension",
                ));
            }
            _ => {}
        }
        Ok(())
    }
}

impl Validate for FieldSpec {
    fn validate(&self) -> DynamicJepaResult<()> {
        if self.name.trim().is_empty() {
            return Err(DynamicJepaError::schema(
                "FieldSpec.name",
                "field name must not be empty",
                "declare an explicit schema field name",
            ));
        }
        if self.name.contains('.') {
            return Err(DynamicJepaError::schema(
                "FieldSpec.name",
                format!("field name {:?} must not contain dots", self.name),
                "use bare field names inside schemas and dotted paths only in adapter/instrument specs",
            ));
        }
        if let (Some(min), Some(max)) = (self.min, self.max) {
            if min > max {
                return Err(DynamicJepaError::schema(
                    format!("FieldSpec.{}", self.name),
                    format!("min {min} exceeds max {max}"),
                    "fix field bounds before registration",
                ));
            }
        }
        self.kind.validate(&format!("FieldSpec.{}.kind", self.name))
    }
}

fn validate_fields(
    fields: &[FieldSpec],
    schema_name: &str,
    allow_empty: bool,
) -> DynamicJepaResult<()> {
    if fields.is_empty() && !allow_empty {
        return Err(DynamicJepaError::schema(
            schema_name,
            "schema must contain at least one field",
            "declare the domain state/action/outcome fields",
        ));
    }
    for field in fields {
        field.validate()?;
    }
    ensure_no_duplicates(fields.iter().map(|f| f.name.as_str()), schema_name)
}

impl Validate for StateSchema {
    fn validate(&self) -> DynamicJepaResult<()> {
        validate_fields(&self.fields, "state_schema.fields", false)
    }
}

impl Validate for ActionSchema {
    fn validate(&self) -> DynamicJepaResult<()> {
        validate_fields(&self.fields, "action_schema.fields", false)
    }
}

impl Validate for OutcomeSchema {
    fn validate(&self) -> DynamicJepaResult<()> {
        validate_fields(&self.fields, "outcome_schema.fields", false)
    }
}

impl Validate for EntitySchema {
    fn validate(&self) -> DynamicJepaResult<()> {
        validate_fields(&self.fields, "entity_schema.fields", true)
    }
}

impl Validate for TimeSchema {
    fn validate(&self) -> DynamicJepaResult<()> {
        self.field.validate()
    }
}
