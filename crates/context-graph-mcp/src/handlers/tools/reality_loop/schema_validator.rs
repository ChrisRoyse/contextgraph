use super::errors::{CCRealityError, Result};
use super::helpers::file_sot;
use jsonschema::{Draft, JSONSchema};
use serde_json::{json, Value};
use std::path::PathBuf;

pub struct RecommendationValidator {
    compiled: JSONSchema,
}

impl RecommendationValidator {
    pub fn load() -> Result<Self> {
        let path = std::env::var("CGREALITY_RECOMMENDATION_SCHEMA")
            .map(PathBuf::from)
            .map_err(|_| {
                CCRealityError::new(
                    "CCREALITY_RECOMMENDATION_SCHEMA_PATH_MISSING",
                    "CGREALITY_RECOMMENDATION_SCHEMA is not set",
                    "env.CGREALITY_RECOMMENDATION_SCHEMA",
                    "set CGREALITY_RECOMMENDATION_SCHEMA to the exact schema path",
                    json!({}),
                    None,
                )
            })?;
        let raw = std::fs::read_to_string(&path).map_err(|e| {
            CCRealityError::new(
                "CCREALITY_RECOMMENDATION_SCHEMA_READ_FAILED",
                format!("failed to read schema: {e}"),
                "config.recommendation_schema",
                "ensure the schema file exists",
                json!({"path": path.display().to_string()}),
                Some(file_sot(&path)),
            )
        })?;
        let schema_value: Value = serde_json::from_str(&raw).map_err(|e| {
            CCRealityError::new(
                "CCREALITY_RECOMMENDATION_SCHEMA_PARSE_FAILED",
                format!("schema is not valid JSON: {e}"),
                "config.recommendation_schema.json",
                "fix schema JSON syntax",
                json!({"path": path.display().to_string()}),
                Some(file_sot(&path)),
            )
        })?;
        Self::from_schema_value(schema_value, Some(file_sot(&path)))
    }

    pub fn from_schema_value(schema_value: Value, source_of_truth: Option<String>) -> Result<Self> {
        let compiled = JSONSchema::options()
            .with_draft(Draft::Draft202012)
            .compile(&schema_value)
            .map_err(|e| {
                CCRealityError::new(
                    "CCREALITY_RECOMMENDATION_SCHEMA_COMPILE_FAILED",
                    format!("schema compile error: {e}"),
                    "config.recommendation_schema",
                    "fix schema definition",
                    json!({"compile_error": e.to_string()}),
                    source_of_truth,
                )
            })?;
        Ok(Self { compiled })
    }

    pub fn validate(&self, value: &Value) -> Result<()> {
        if let Err(errors) = self.compiled.validate(value) {
            let details = errors
                .map(|e| {
                    json!({
                        "instance_path": e.instance_path.to_string(),
                        "schema_path": e.schema_path.to_string(),
                        "message": e.to_string()
                    })
                })
                .collect::<Vec<_>>();
            return Err(CCRealityError::new(
                "CCREALITY_RECOMMENDATION_SCHEMA_INVALID",
                format!(
                    "recommendation failed schema validation; {} errors",
                    details.len()
                ),
                "recommendation",
                "fix the recommendation per the listed errors",
                json!({"errors": details, "value": value}),
                None,
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_required_field() {
        let schema = json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "required": ["record_kind"],
            "properties": { "record_kind": { "const": "ccreality_optimizer_recommendation" } }
        });
        let validator = RecommendationValidator::from_schema_value(schema, None).expect("schema");
        validator
            .validate(&json!({"record_kind": "ccreality_optimizer_recommendation"}))
            .expect("valid");
        let err = validator.validate(&json!({})).expect_err("invalid");
        assert_eq!(err.error_code, "CCREALITY_RECOMMENDATION_SCHEMA_INVALID");
    }
}
