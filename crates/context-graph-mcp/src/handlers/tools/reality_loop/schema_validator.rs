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
    use std::fs;
    use std::path::Path;

    fn repo_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn real_phase5_schema_validates_sample_recommendation() {
        let root = repo_root();
        let schema_path = root.join("config/ccreality_recommendation_schema.json");
        let sample_path =
            root.join("backupdocs/ccreality/phase-05-config-files/sample_recommendation.json");
        let schema: Value =
            serde_json::from_str(&fs::read_to_string(&schema_path).expect("schema file"))
                .expect("schema json");
        let sample: Value =
            serde_json::from_str(&fs::read_to_string(&sample_path).expect("sample file"))
                .expect("sample json");
        let validator = RecommendationValidator::from_schema_value(
            schema,
            Some(format!("file:{}", schema_path.display())),
        )
        .expect("schema");
        validator.validate(&sample).expect("sample is valid");
    }

    #[test]
    fn real_phase5_schema_rejects_over_limit_changed_recommendation() {
        let root = repo_root();
        let schema_path = root.join("config/ccreality_recommendation_schema.json");
        let sample_path =
            root.join("backupdocs/ccreality/phase-05-config-files/sample_recommendation.json");
        let schema: Value =
            serde_json::from_str(&fs::read_to_string(&schema_path).expect("schema file"))
                .expect("schema json");
        let mut sample: Value =
            serde_json::from_str(&fs::read_to_string(&sample_path).expect("sample file"))
                .expect("sample json");
        sample["source_files_changed"] = Value::Array(
            (0..26)
                .map(|i| Value::String(format!("docs/ccreality/phase-05-config-files/file-{i}.md")))
                .collect(),
        );
        let validator = RecommendationValidator::from_schema_value(
            schema,
            Some(format!("file:{}", schema_path.display())),
        )
        .expect("schema");
        let err = validator.validate(&sample).expect_err("invalid");
        assert_eq!(err.error_code, "CCREALITY_RECOMMENDATION_SCHEMA_INVALID");
        assert!(err
            .details
            .to_string()
            .contains("\"schema_path\":\"/properties/source_files_changed/maxItems\""));
    }

    #[test]
    fn real_phase5_schema_rejects_unknown_fields() {
        let root = repo_root();
        let schema_path = root.join("config/ccreality_recommendation_schema.json");
        let sample_path =
            root.join("backupdocs/ccreality/phase-05-config-files/sample_recommendation.json");
        let schema: Value =
            serde_json::from_str(&fs::read_to_string(&schema_path).expect("schema file"))
                .expect("schema json");
        let mut sample: Value =
            serde_json::from_str(&fs::read_to_string(&sample_path).expect("sample file"))
                .expect("sample json");
        sample["unexpected_field"] = Value::String("must fail".to_string());
        let validator = RecommendationValidator::from_schema_value(
            schema,
            Some(format!("file:{}", schema_path.display())),
        )
        .expect("schema");
        let err = validator.validate(&sample).expect_err("invalid");
        assert_eq!(err.error_code, "CCREALITY_RECOMMENDATION_SCHEMA_INVALID");
        assert!(err
            .details
            .to_string()
            .contains("\"schema_path\":\"/additionalProperties\""));
    }

    #[test]
    fn real_phase5_schema_accepts_no_change_with_empty_change_sets() {
        let root = repo_root();
        let schema_path = root.join("config/ccreality_recommendation_schema.json");
        let sample_path =
            root.join("backupdocs/ccreality/phase-05-config-files/sample_recommendation.json");
        let schema: Value =
            serde_json::from_str(&fs::read_to_string(&schema_path).expect("schema file"))
                .expect("schema json");
        let mut sample: Value =
            serde_json::from_str(&fs::read_to_string(&sample_path).expect("sample file"))
                .expect("sample json");
        sample["status"] = Value::String("no_change".to_string());
        sample["source_files_changed"] = Value::Array(Vec::new());
        sample["harness_transitions_minted"] = Value::Array(Vec::new());
        sample["shift_log_excerpt"] = Value::Array(Vec::new());
        let validator = RecommendationValidator::from_schema_value(
            schema,
            Some(format!("file:{}", schema_path.display())),
        )
        .expect("schema");
        validator
            .validate(&sample)
            .expect("no_change recommendation is valid");
    }
}
