use crate::{InstrumentError, InstrumentResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const STDLIB_REGISTRY_SCHEMA_VERSION: u16 = 1;
pub const EXPECTED_STDLIB_LANGUAGES: &[&str] = &[
    "c",
    "cpp",
    "csharp",
    "go",
    "java",
    "javascript",
    "php",
    "python",
    "ruby",
    "rust",
    "typescript",
];

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StdlibRegistry {
    pub schema_version: u16,
    pub language: String,
    pub patterns: Vec<StdlibPattern>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StdlibPattern {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub stdlib_alternative: String,
    pub confidence: f32,
    pub detection: PatternDetection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatternDetection {
    pub required_substrings: Vec<String>,
    #[serde(default)]
    pub forbidden_substrings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReinventedWheelDetection {
    pub language: String,
    pub pattern_id: String,
    pub pattern_name: String,
    pub stdlib_alternative: String,
    pub confidence: f32,
    pub matched_substrings: Vec<String>,
}

pub fn default_registry_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets/stdlib_registry")
}

pub fn load_registry(path: impl AsRef<Path>) -> InstrumentResult<StdlibRegistry> {
    let path = path.as_ref();
    let text = fs::read_to_string(path).map_err(|err| {
        InstrumentError::invalid(
            "stdlib_registry.path",
            format!("failed to read {}: {err}", path.display()),
            "restore the checked-in per-language stdlib registry file",
        )
    })?;
    let registry: StdlibRegistry = serde_yaml::from_str(&text).map_err(|err| {
        InstrumentError::invalid(
            "stdlib_registry.yaml",
            format!("failed to parse {}: {err}", path.display()),
            "fix the YAML registry file with schema_version, language, and patterns",
        )
    })?;
    registry.validate()?;
    Ok(registry)
}

pub fn load_registry_dir(
    path: impl AsRef<Path>,
) -> InstrumentResult<BTreeMap<String, StdlibRegistry>> {
    let path = path.as_ref();
    let mut registries = BTreeMap::new();
    for language in EXPECTED_STDLIB_LANGUAGES {
        let file = path.join(format!("{language}.yml"));
        if !file.is_file() {
            return Err(InstrumentError::invalid(
                "stdlib_registry.dir",
                format!("missing registry file {}", file.display()),
                "check in one YAML registry per required language",
            ));
        }
        let registry = load_registry(&file)?;
        if registry.language != *language {
            return Err(InstrumentError::invalid(
                "stdlib_registry.language",
                format!(
                    "{} declares language {}, expected {}",
                    file.display(),
                    registry.language,
                    language
                ),
                "rename or fix the YAML language field so registry routing is explicit",
            ));
        }
        registries.insert(registry.language.clone(), registry);
    }
    Ok(registries)
}

pub fn detect_reinvented_wheels(
    registries: &BTreeMap<String, StdlibRegistry>,
    language: &str,
    source: &str,
) -> InstrumentResult<Vec<ReinventedWheelDetection>> {
    let registry = registries.get(language).ok_or_else(|| {
        InstrumentError::invalid(
            "stdlib_registry.language",
            format!("no stdlib registry loaded for language {language}"),
            "load the complete stdlib registry directory before detection",
        )
    })?;
    registry.detect(source)
}

impl StdlibRegistry {
    pub fn validate(&self) -> InstrumentResult<()> {
        if self.schema_version != STDLIB_REGISTRY_SCHEMA_VERSION {
            return Err(InstrumentError::invalid(
                "stdlib_registry.schema_version",
                format!(
                    "schema_version {} does not match runtime schema {}",
                    self.schema_version, STDLIB_REGISTRY_SCHEMA_VERSION
                ),
                "migrate the stdlib registry file before loading it",
            ));
        }
        validate_slug("stdlib_registry.language", &self.language)?;
        if self.patterns.is_empty() {
            return Err(InstrumentError::invalid(
                "stdlib_registry.patterns",
                format!("{} registry has no patterns", self.language),
                "add at least one ReinventedWheel pattern for the language",
            ));
        }
        for pattern in &self.patterns {
            pattern.validate(&self.language)?;
        }
        Ok(())
    }

    pub fn detect(&self, source: &str) -> InstrumentResult<Vec<ReinventedWheelDetection>> {
        self.validate()?;
        let mut detections = Vec::new();
        for pattern in &self.patterns {
            if pattern.matches(source) {
                detections.push(ReinventedWheelDetection {
                    language: self.language.clone(),
                    pattern_id: pattern.id.clone(),
                    pattern_name: pattern.name.clone(),
                    stdlib_alternative: pattern.stdlib_alternative.clone(),
                    confidence: pattern.confidence,
                    matched_substrings: pattern.detection.required_substrings.clone(),
                });
            }
        }
        Ok(detections)
    }
}

impl StdlibPattern {
    fn validate(&self, language: &str) -> InstrumentResult<()> {
        validate_slug("stdlib_registry.pattern.id", &self.id)?;
        if self.kind != "ReinventedWheel" {
            return Err(InstrumentError::invalid(
                "stdlib_registry.pattern.kind",
                format!(
                    "pattern {} in {language} has unsupported kind {}",
                    self.id, self.kind
                ),
                "Phase D stdlib registry supports ReinventedWheel patterns only",
            ));
        }
        if self.name.trim().is_empty() {
            return Err(InstrumentError::invalid(
                "stdlib_registry.pattern.name",
                format!("pattern {} in {language} has an empty name", self.id),
                "give each registry pattern a human-readable name",
            ));
        }
        if self.stdlib_alternative.trim().is_empty() {
            return Err(InstrumentError::invalid(
                "stdlib_registry.pattern.stdlib_alternative",
                format!("pattern {} in {language} has an empty alternative", self.id),
                "record the concrete stdlib or platform API that should replace the hand-rolled code",
            ));
        }
        if !self.confidence.is_finite() || !(0.0..=1.0).contains(&self.confidence) {
            return Err(InstrumentError::invalid(
                "stdlib_registry.pattern.confidence",
                format!("pattern {} confidence must be finite in [0, 1]", self.id),
                "fix registry confidence to a calibrated probability-like score",
            ));
        }
        if self.detection.required_substrings.is_empty() {
            return Err(InstrumentError::invalid(
                "stdlib_registry.pattern.detection.required_substrings",
                format!(
                    "pattern {} in {language} has no required substrings",
                    self.id
                ),
                "record a concrete structured detection signature",
            ));
        }
        if self
            .detection
            .required_substrings
            .iter()
            .chain(self.detection.forbidden_substrings.iter())
            .any(|value| value.is_empty())
        {
            return Err(InstrumentError::invalid(
                "stdlib_registry.pattern.detection.substrings",
                format!(
                    "pattern {} in {language} contains an empty substring",
                    self.id
                ),
                "remove empty detection entries so matches are meaningful",
            ));
        }
        Ok(())
    }

    fn matches(&self, source: &str) -> bool {
        self.detection
            .required_substrings
            .iter()
            .all(|needle| source.contains(needle))
            && self
                .detection
                .forbidden_substrings
                .iter()
                .all(|needle| !source.contains(needle))
    }
}

fn validate_slug(field: &'static str, value: &str) -> InstrumentResult<()> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(InstrumentError::invalid(
            field,
            format!("{value:?} is not a lowercase registry slug"),
            "use a non-empty lowercase ASCII slug",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_in_registry_loads_all_languages() {
        let registries = load_registry_dir(default_registry_dir()).unwrap();
        assert_eq!(registries.len(), EXPECTED_STDLIB_LANGUAGES.len());
        for language in EXPECTED_STDLIB_LANGUAGES {
            assert!(registries.contains_key(*language));
        }
    }

    #[test]
    fn python_retry_loop_detects_reinvented_wheel() {
        let registries = load_registry_dir(default_registry_dir()).unwrap();
        let source = r#"
import time

for attempt in range(5):
    try:
        call_remote()
        break
    except Exception:
        time.sleep(2 ** attempt)
"#;
        let detections = detect_reinvented_wheels(&registries, "python", source).unwrap();
        assert!(detections
            .iter()
            .any(|detection| detection.pattern_id == "python_retry_backoff"));
    }

    #[test]
    fn unknown_language_fails_closed() {
        let registries = load_registry_dir(default_registry_dir()).unwrap();
        let err = detect_reinvented_wheels(&registries, "unknown", "").unwrap_err();
        assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");
    }
}
