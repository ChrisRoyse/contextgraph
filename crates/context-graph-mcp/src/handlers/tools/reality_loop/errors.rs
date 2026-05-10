use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct SmallString(Box<str>);

impl SmallString {
    fn new(value: impl Into<String>) -> Self {
        Self(value.into().into_boxed_str())
    }
}

impl std::fmt::Display for SmallString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::ops::Deref for SmallString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PartialEq<&str> for SmallString {
    fn eq(&self, other: &&str) -> bool {
        self.0.as_ref() == *other
    }
}

impl PartialEq<SmallString> for &str {
    fn eq(&self, other: &SmallString) -> bool {
        *self == other.0.as_ref()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CCRealityError {
    pub status: &'static str,
    pub error_code: SmallString,
    pub message: SmallString,
    pub field_path: SmallString,
    pub remediation: SmallString,
    pub details: Box<Value>,
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
            error_code: SmallString::new(error_code),
            message: SmallString::new(message),
            field_path: SmallString::new(field_path),
            remediation: SmallString::new(remediation),
            details: Box::new(details),
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
        write!(
            f,
            "[{}] {} ({})",
            self.error_code, self.message, self.field_path
        )
    }
}

impl std::error::Error for CCRealityError {}

pub type Result<T> = std::result::Result<T, CCRealityError>;
