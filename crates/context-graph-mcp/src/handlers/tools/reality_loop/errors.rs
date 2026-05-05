use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub struct CCRealityError {
    pub status: &'static str,
    pub error_code: String,
    pub message: String,
    pub field_path: String,
    pub remediation: String,
    pub details: Value,
    pub source_of_truth: Option<String>,
}

impl CCRealityError {
    pub fn new(
        error_code: impl Into<String>,
        message: impl Into<String>,
        field_path: impl Into<String>,
        remediation: impl Into<String>,
        details: Value,
        source_of_truth: Option<String>,
    ) -> Self {
        Self {
            status: "error",
            error_code: error_code.into(),
            message: message.into(),
            field_path: field_path.into(),
            remediation: remediation.into(),
            details,
            source_of_truth,
        }
    }

    pub fn into_value(self) -> Value {
        serde_json::to_value(self).unwrap_or_else(|_| {
            serde_json::json!({
                "status": "error",
                "error_code": "CCREALITY_ERROR_SERIALIZATION_FAILED"
            })
        })
    }
}

impl std::fmt::Display for CCRealityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {} ({})", self.error_code, self.message, self.field_path)
    }
}

impl std::error::Error for CCRealityError {}

pub type Result<T> = std::result::Result<T, CCRealityError>;
