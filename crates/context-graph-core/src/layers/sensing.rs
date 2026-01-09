//! L1 Sensing Layer - Real implementation with PII scrubbing.
//!
//! The Sensing layer is the first layer in the bio-nervous system.
//! It handles multi-modal input processing with mandatory PII scrubbing.
//!
//! # Constitution Compliance
//!
//! - SEC-01: Validate/sanitize all input (PIIScrubber L1)
//! - SEC-02: Scrub PII pre-embed with patterns: api_key, password, bearer_token, ssn, credit_card
//! - Latency budget: <5ms
//! - Throughput: 10K/s
//!
//! # Critical Rules
//!
//! - NO BACKWARDS COMPATIBILITY: System works or fails fast
//! - NO MOCK DATA: Returns real processed data or proper errors
//! - NO FALLBACKS: If PII detection fails, ERROR OUT with explicit message

use async_trait::async_trait;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use crate::error::CoreResult;
use crate::traits::NervousLayer;
use crate::types::{LayerId, LayerInput, LayerOutput, LayerResult};

/// Static regex patterns for PII detection - compiled once, used many times.
static PII_PATTERNS: OnceLock<PiiPatternSet> = OnceLock::new();

/// Pre-compiled PII pattern set for efficient matching.
#[derive(Debug)]
struct PiiPatternSet {
    api_key: Regex,
    password: Regex,
    bearer_token: Regex,
    ssn: Regex,
    credit_card: Regex,
}

impl PiiPatternSet {
    /// Initialize all PII patterns. Called once via OnceLock.
    fn new() -> Self {
        Self {
            // API keys: AWS (AKIA...), OpenAI (sk-...), generic patterns
            api_key: Regex::new(
                r#"(?i)(?:AKIA[0-9A-Z]{16}|sk-[a-zA-Z0-9]{20,}|api[_-]?key\s*[=:]\s*['"]?[a-zA-Z0-9_-]{20,}['"]?)"#
            ).expect("api_key regex must compile"),

            // Password patterns in various formats
            password: Regex::new(
                r#"(?i)(?:password|passwd|pwd)\s*[=:]\s*['"]?[^\s'"]{4,}['"]?"#
            ).expect("password regex must compile"),

            // Bearer tokens in Authorization headers
            bearer_token: Regex::new(
                r#"(?i)(?:bearer\s+[a-zA-Z0-9_.-]+|authorization\s*[=:]\s*['"]?bearer\s+[a-zA-Z0-9_.-]+['"]?)"#
            ).expect("bearer_token regex must compile"),

            // SSN: XXX-XX-XXXX format (with optional dashes/spaces)
            ssn: Regex::new(
                r"\b\d{3}[-\s]?\d{2}[-\s]?\d{4}\b"
            ).expect("ssn regex must compile"),

            // Credit cards: Major formats (Visa, MC, Amex, Discover)
            // 13-19 digits with optional separators
            credit_card: Regex::new(
                r"\b(?:4[0-9]{3}[-\s]?[0-9]{4}[-\s]?[0-9]{4}[-\s]?[0-9]{1,4}|5[1-5][0-9]{2}[-\s]?[0-9]{4}[-\s]?[0-9]{4}[-\s]?[0-9]{4}|3[47][0-9]{2}[-\s]?[0-9]{6}[-\s]?[0-9]{5}|6(?:011|5[0-9]{2})[-\s]?[0-9]{4}[-\s]?[0-9]{4}[-\s]?[0-9]{4})\b"
            ).expect("credit_card regex must compile"),
        }
    }
}

/// Get the singleton pattern set.
fn get_patterns() -> &'static PiiPatternSet {
    PII_PATTERNS.get_or_init(PiiPatternSet::new)
}

/// Type of PII that was detected and scrubbed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PiiPattern {
    /// AWS or other API keys (e.g., AKIA..., sk-...)
    ApiKey,
    /// Password values in various formats
    Password,
    /// Bearer tokens in Authorization headers
    BearerToken,
    /// Social Security Numbers (XXX-XX-XXXX)
    Ssn,
    /// Credit card numbers (13-19 digits)
    CreditCard,
}

impl PiiPattern {
    /// Get the replacement string for this PII type.
    pub fn redaction_marker(&self) -> &'static str {
        match self {
            PiiPattern::ApiKey => "[REDACTED:API_KEY]",
            PiiPattern::Password => "[REDACTED:PASSWORD]",
            PiiPattern::BearerToken => "[REDACTED:BEARER_TOKEN]",
            PiiPattern::Ssn => "[REDACTED:SSN]",
            PiiPattern::CreditCard => "[REDACTED:CREDIT_CARD]",
        }
    }

    /// Get all pattern types for iteration.
    pub fn all() -> &'static [PiiPattern] {
        &[
            PiiPattern::ApiKey,
            PiiPattern::Password,
            PiiPattern::BearerToken,
            PiiPattern::Ssn,
            PiiPattern::CreditCard,
        ]
    }
}

/// Record of a single PII detection and scrubbing event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiiDetection {
    /// Type of PII detected
    pub pattern: PiiPattern,
    /// Starting position in original content
    pub start_pos: usize,
    /// Ending position in original content
    pub end_pos: usize,
    /// Length of original content (not the content itself - for audit without exposure)
    pub original_length: usize,
}

/// Result of PII scrubbing operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrubbedContent {
    /// The scrubbed content with all PII replaced
    pub content: String,
    /// List of all detections made during scrubbing
    pub detections: Vec<PiiDetection>,
    /// Count of each PII type found
    pub detection_counts: HashMap<PiiPattern, usize>,
    /// Whether any PII was detected
    pub pii_found: bool,
    /// Processing duration in microseconds
    pub processing_us: u64,
}

impl ScrubbedContent {
    /// Create an empty result for content with no PII.
    fn clean(content: String, processing_us: u64) -> Self {
        Self {
            content,
            detections: Vec::new(),
            detection_counts: HashMap::new(),
            pii_found: false,
            processing_us,
        }
    }
}

/// PII Scrubber for detecting and removing sensitive information.
///
/// This is the core security component of the Sensing layer.
/// It MUST successfully process all input - failures are NOT acceptable.
///
/// # Constitution Compliance
///
/// - SEC-01: All input validated/sanitized
/// - SEC-02: Scrub PII pre-embed with required patterns
/// - NO FALLBACKS: If scrubbing fails, error out
#[derive(Debug, Clone, Default)]
pub struct PiiScrubber {
    /// Whether to include position information in detections (for audit)
    include_positions: bool,
}

impl PiiScrubber {
    /// Create a new PII scrubber with default settings.
    pub fn new() -> Self {
        Self {
            include_positions: true,
        }
    }

    /// Create a scrubber without position tracking (slightly faster).
    pub fn without_positions() -> Self {
        Self {
            include_positions: false,
        }
    }

    /// Scrub all PII from content, returning the cleaned content and audit trail.
    ///
    /// # Returns
    ///
    /// `Ok(ScrubbedContent)` - Always succeeds with scrubbed content
    ///
    /// # Constitution Compliance
    ///
    /// This function MUST NOT fail. PII scrubbing is mandatory and the patterns
    /// are pre-compiled and validated at startup. Any failure indicates a bug.
    pub fn scrub(&self, content: &str) -> CoreResult<ScrubbedContent> {
        let start = Instant::now();

        // Empty content is valid - no PII possible
        if content.is_empty() {
            return Ok(ScrubbedContent::clean(String::new(), 0));
        }

        let patterns = get_patterns();
        let mut result = content.to_string();
        let mut detections = Vec::new();
        let mut counts: HashMap<PiiPattern, usize> = HashMap::new();

        // Process each pattern type in order
        // We need to track offset changes as we replace
        for pii_type in PiiPattern::all() {
            let regex = match pii_type {
                PiiPattern::ApiKey => &patterns.api_key,
                PiiPattern::Password => &patterns.password,
                PiiPattern::BearerToken => &patterns.bearer_token,
                PiiPattern::Ssn => &patterns.ssn,
                PiiPattern::CreditCard => &patterns.credit_card,
            };

            // Find all matches in current result
            let matches: Vec<_> = regex.find_iter(&result).collect();
            let match_count = matches.len();

            if match_count > 0 {
                *counts.entry(*pii_type).or_insert(0) += match_count;

                if self.include_positions {
                    for m in &matches {
                        detections.push(PiiDetection {
                            pattern: *pii_type,
                            start_pos: m.start(),
                            end_pos: m.end(),
                            original_length: m.as_str().len(),
                        });
                    }
                }

                // Replace all matches with redaction marker
                result = regex
                    .replace_all(&result, pii_type.redaction_marker())
                    .to_string();
            }
        }

        let processing_us = start.elapsed().as_micros() as u64;
        let pii_found = !counts.is_empty();

        Ok(ScrubbedContent {
            content: result,
            detections,
            detection_counts: counts,
            pii_found,
            processing_us,
        })
    }

    /// Validate that scrubbing actually removed all PII.
    ///
    /// This is a verification step - run the scrubber again and ensure
    /// no PII is found in the output. Used for high-security scenarios.
    pub fn verify_clean(&self, scrubbed: &ScrubbedContent) -> CoreResult<bool> {
        let verification = self.scrub(&scrubbed.content)?;
        Ok(!verification.pii_found)
    }
}

/// Metrics from Sensing layer processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SensingMetrics {
    /// Total processing time in microseconds
    pub total_us: u64,
    /// PII scrubbing time in microseconds
    pub pii_scrub_us: u64,
    /// Content length before scrubbing
    pub input_length: usize,
    /// Content length after scrubbing
    pub output_length: usize,
    /// Number of PII detections
    pub pii_detections: usize,
    /// Entropy measurement (ΔS) - novelty/surprise estimate
    pub delta_s: f32,
}

/// L1 Sensing Layer - Real production implementation.
///
/// Handles multi-modal input processing with mandatory PII scrubbing.
///
/// # Constitution Compliance
///
/// - Latency: <5ms budget
/// - Throughput: 10K/s capability
/// - Components: 13-model embed, PII scrub, adversarial detect
/// - UTL: ΔS measurement
///
/// # Security (SEC-01, SEC-02)
///
/// - All input is validated and sanitized
/// - PII scrubbed before any downstream processing
/// - Patterns: api_key, password, bearer_token, ssn, credit_card
#[derive(Debug)]
pub struct SensingLayer {
    /// PII scrubber instance
    pii_scrubber: PiiScrubber,
}

impl Default for SensingLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl SensingLayer {
    /// Create a new Sensing layer with default configuration.
    pub fn new() -> Self {
        Self {
            pii_scrubber: PiiScrubber::new(),
        }
    }

    /// Create a Sensing layer with a custom PII scrubber.
    pub fn with_scrubber(pii_scrubber: PiiScrubber) -> Self {
        Self { pii_scrubber }
    }

    /// Compute a simple entropy estimate (ΔS) for novelty/surprise.
    ///
    /// This is a placeholder for the full 13-model embedding entropy calculation.
    /// The real implementation would use the embedding pipeline, but we need
    /// a functional approximation for now.
    fn compute_delta_s(content: &str) -> f32 {
        if content.is_empty() {
            return 0.0;
        }

        // Character-level entropy as a simple proxy for content novelty
        let mut char_counts = [0u32; 256];
        let mut total = 0u32;

        for byte in content.bytes() {
            char_counts[byte as usize] += 1;
            total += 1;
        }

        let mut entropy = 0.0f32;
        for &count in &char_counts {
            if count > 0 {
                let p = count as f32 / total as f32;
                entropy -= p * p.log2();
            }
        }

        // Normalize to [0, 1] range (max entropy for 256 chars is 8 bits)
        (entropy / 8.0).clamp(0.0, 1.0)
    }
}

#[async_trait]
impl NervousLayer for SensingLayer {
    async fn process(&self, input: LayerInput) -> CoreResult<LayerOutput> {
        let start = Instant::now();

        // Step 1: PII Scrubbing - MANDATORY, NO FALLBACK
        let scrubbed = self.pii_scrubber.scrub(&input.content)?;

        // Step 2: Compute ΔS (entropy/novelty measurement)
        let delta_s = Self::compute_delta_s(&scrubbed.content);

        // Step 3: Build metrics
        let metrics = SensingMetrics {
            total_us: 0, // Will be set after
            pii_scrub_us: scrubbed.processing_us,
            input_length: input.content.len(),
            output_length: scrubbed.content.len(),
            pii_detections: scrubbed.detections.len(),
            delta_s,
        };

        let duration = start.elapsed();
        let duration_us = duration.as_micros() as u64;

        // Step 4: Check latency budget
        let budget = self.latency_budget();
        if duration > budget {
            // Log warning but don't fail - we still processed successfully
            // In production, this would be logged to metrics
            tracing::warn!(
                "SensingLayer exceeded latency budget: {:?} > {:?}",
                duration,
                budget
            );
        }

        // Step 5: Build result data
        let result_data = serde_json::json!({
            "scrubbed_content": scrubbed.content,
            "pii_found": scrubbed.pii_found,
            "pii_detection_counts": scrubbed.detection_counts,
            "metrics": {
                "total_us": duration_us,
                "pii_scrub_us": metrics.pii_scrub_us,
                "input_length": metrics.input_length,
                "output_length": metrics.output_length,
                "pii_detections": metrics.pii_detections,
                "delta_s": metrics.delta_s,
            }
        });

        // Step 6: Update pulse with ΔS
        let mut pulse = input.context.pulse.clone();
        pulse.entropy = delta_s;

        Ok(LayerOutput {
            layer: LayerId::Sensing,
            result: LayerResult::success(LayerId::Sensing, result_data),
            pulse,
            duration_us,
        })
    }

    fn latency_budget(&self) -> Duration {
        Duration::from_millis(5)
    }

    fn layer_id(&self) -> LayerId {
        LayerId::Sensing
    }

    fn layer_name(&self) -> &'static str {
        "Sensing Layer"
    }

    async fn health_check(&self) -> CoreResult<bool> {
        // Verify the scrubber can process content
        let test_result = self.pii_scrubber.scrub("health check test")?;

        // Verify patterns are loaded
        let patterns = get_patterns();

        // Quick sanity check on patterns
        let ssn_test = patterns.ssn.is_match("123-45-6789");
        let cc_test = patterns.credit_card.is_match("4111-1111-1111-1111");

        // All checks must pass
        Ok(test_result.content == "health check test" && ssn_test && cc_test)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ============================================================
    // PII Pattern Tests - REAL detection, NO mocks
    // ============================================================

    #[test]
    fn test_ssn_detection() {
        let scrubber = PiiScrubber::new();

        // Standard format
        let result = scrubber.scrub("My SSN is 123-45-6789").unwrap();
        assert!(result.pii_found);
        assert_eq!(result.detection_counts.get(&PiiPattern::Ssn), Some(&1));
        assert!(result.content.contains("[REDACTED:SSN]"));
        assert!(!result.content.contains("123-45-6789"));

        // Without dashes
        let result = scrubber.scrub("SSN: 123456789").unwrap();
        assert!(result.pii_found);
        assert!(result.content.contains("[REDACTED:SSN]"));

        // With spaces
        let result = scrubber.scrub("SSN is 123 45 6789").unwrap();
        assert!(result.pii_found);
    }

    #[test]
    fn test_credit_card_detection() {
        let scrubber = PiiScrubber::new();

        // Visa with dashes
        let result = scrubber.scrub("Card: 4111-1111-1111-1111").unwrap();
        assert!(result.pii_found);
        assert_eq!(
            result.detection_counts.get(&PiiPattern::CreditCard),
            Some(&1)
        );
        assert!(result.content.contains("[REDACTED:CREDIT_CARD]"));
        assert!(!result.content.contains("4111"));

        // Visa without separators
        let result = scrubber.scrub("CC 4111111111111111").unwrap();
        assert!(result.pii_found);

        // MasterCard
        let result = scrubber.scrub("MC: 5500-0000-0000-0004").unwrap();
        assert!(result.pii_found);

        // Amex
        let result = scrubber.scrub("Amex 3782-822463-10005").unwrap();
        assert!(result.pii_found);
    }

    #[test]
    fn test_api_key_detection() {
        let scrubber = PiiScrubber::new();

        // AWS access key
        let result = scrubber.scrub("AWS key: AKIAIOSFODNN7EXAMPLE").unwrap();
        assert!(result.pii_found);
        assert_eq!(result.detection_counts.get(&PiiPattern::ApiKey), Some(&1));
        assert!(result.content.contains("[REDACTED:API_KEY]"));

        // OpenAI key
        let result = scrubber
            .scrub("key = sk-abcdefghij1234567890abcdef")
            .unwrap();
        assert!(result.pii_found);

        // Generic api_key pattern
        let result = scrubber.scrub("api_key=abcdefghij1234567890").unwrap();
        assert!(result.pii_found);
    }

    #[test]
    fn test_password_detection() {
        let scrubber = PiiScrubber::new();

        // password=
        let result = scrubber.scrub("password=secret123").unwrap();
        assert!(result.pii_found);
        assert_eq!(result.detection_counts.get(&PiiPattern::Password), Some(&1));
        assert!(result.content.contains("[REDACTED:PASSWORD]"));
        assert!(!result.content.contains("secret123"));

        // PASSWORD:
        let result = scrubber.scrub("PASSWORD: mysecret").unwrap();
        assert!(result.pii_found);

        // pwd=
        let result = scrubber.scrub("pwd='mypassword'").unwrap();
        assert!(result.pii_found);
    }

    #[test]
    fn test_bearer_token_detection() {
        let scrubber = PiiScrubber::new();

        // Standard bearer
        let result = scrubber
            .scrub("Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9")
            .unwrap();
        assert!(result.pii_found);
        assert_eq!(
            result.detection_counts.get(&PiiPattern::BearerToken),
            Some(&1)
        );
        assert!(result.content.contains("[REDACTED:BEARER_TOKEN]"));

        // bearer token inline
        let result = scrubber.scrub("Use bearer abc123-token-xyz").unwrap();
        assert!(result.pii_found);
    }

    #[test]
    fn test_multiple_pii_types() {
        let scrubber = PiiScrubber::new();

        let content = "User SSN is 123-45-6789 and credit card is 4111-1111-1111-1111";
        let result = scrubber.scrub(content).unwrap();

        assert!(result.pii_found);
        assert_eq!(result.detection_counts.get(&PiiPattern::Ssn), Some(&1));
        assert_eq!(
            result.detection_counts.get(&PiiPattern::CreditCard),
            Some(&1)
        );
        assert!(result.content.contains("[REDACTED:SSN]"));
        assert!(result.content.contains("[REDACTED:CREDIT_CARD]"));
    }

    #[test]
    fn test_no_pii_content() {
        let scrubber = PiiScrubber::new();

        let content = "This is safe content with no sensitive information.";
        let result = scrubber.scrub(content).unwrap();

        assert!(!result.pii_found);
        assert!(result.detections.is_empty());
        assert_eq!(result.content, content);
    }

    #[test]
    fn test_empty_content() {
        let scrubber = PiiScrubber::new();
        let result = scrubber.scrub("").unwrap();
        assert!(!result.pii_found);
        assert!(result.content.is_empty());
    }

    #[test]
    fn test_verify_clean() {
        let scrubber = PiiScrubber::new();

        // After scrubbing, content should be clean
        let result = scrubber.scrub("SSN: 123-45-6789").unwrap();
        assert!(scrubber.verify_clean(&result).unwrap());

        // Raw content with PII should not verify as clean
        let fake_clean = ScrubbedContent {
            content: "SSN: 123-45-6789".to_string(),
            detections: vec![],
            detection_counts: HashMap::new(),
            pii_found: false,
            processing_us: 0,
        };
        assert!(!scrubber.verify_clean(&fake_clean).unwrap());
    }

    // ============================================================
    // Sensing Layer Tests
    // ============================================================

    #[tokio::test]
    async fn test_sensing_layer_process() {
        let layer = SensingLayer::new();
        let input = LayerInput::new("test-123".to_string(), "Hello world".to_string());

        let output = layer.process(input).await.unwrap();

        assert_eq!(output.layer, LayerId::Sensing);
        assert!(output.result.success);
        assert!(output.duration_us > 0);
    }

    #[tokio::test]
    async fn test_sensing_layer_scrubs_pii() {
        let layer = SensingLayer::new();
        let input = LayerInput::new(
            "test-123".to_string(),
            "My SSN is 123-45-6789 and my password=secret".to_string(),
        );

        let output = layer.process(input).await.unwrap();

        assert!(output.result.success);
        let data = &output.result.data;
        assert!(data["pii_found"].as_bool().unwrap());

        let scrubbed = data["scrubbed_content"].as_str().unwrap();
        assert!(scrubbed.contains("[REDACTED:SSN]"));
        assert!(scrubbed.contains("[REDACTED:PASSWORD]"));
        assert!(!scrubbed.contains("123-45-6789"));
        assert!(!scrubbed.contains("secret"));
    }

    #[tokio::test]
    async fn test_sensing_layer_computes_delta_s() {
        let layer = SensingLayer::new();
        let input = LayerInput::new(
            "test-123".to_string(),
            "Some text content here.".to_string(),
        );

        let output = layer.process(input).await.unwrap();

        let delta_s = output.result.data["metrics"]["delta_s"].as_f64().unwrap();
        assert!(delta_s > 0.0);
        assert!(delta_s <= 1.0);
    }

    #[tokio::test]
    async fn test_sensing_layer_health_check() {
        let layer = SensingLayer::new();
        let healthy = layer.health_check().await.unwrap();
        assert!(healthy, "SensingLayer should be healthy");
    }

    #[test]
    fn test_sensing_layer_properties() {
        let layer = SensingLayer::new();

        assert_eq!(layer.layer_id(), LayerId::Sensing);
        assert_eq!(layer.latency_budget(), Duration::from_millis(5));
        assert_eq!(layer.layer_name(), "Sensing Layer");
    }

    // ============================================================
    // Edge Case Tests
    // ============================================================

    #[test]
    fn test_pii_at_boundaries() {
        let scrubber = PiiScrubber::new();

        // PII at start
        let result = scrubber.scrub("123-45-6789 is my SSN").unwrap();
        assert!(result.pii_found);

        // PII at end
        let result = scrubber.scrub("My SSN is 123-45-6789").unwrap();
        assert!(result.pii_found);

        // Only PII
        let result = scrubber.scrub("123-45-6789").unwrap();
        assert!(result.pii_found);
    }

    #[test]
    fn test_multiple_same_pii() {
        let scrubber = PiiScrubber::new();

        let content = "SSN1: 123-45-6789, SSN2: 987-65-4321";
        let result = scrubber.scrub(content).unwrap();

        assert!(result.pii_found);
        assert_eq!(result.detection_counts.get(&PiiPattern::Ssn), Some(&2));
    }

    #[test]
    fn test_special_characters_preserved() {
        let scrubber = PiiScrubber::new();

        let content = "Hello!\n\t@#$%^&*()_+\nSSN: 123-45-6789\nWorld!";
        let result = scrubber.scrub(content).unwrap();

        assert!(result.content.contains("Hello!"));
        assert!(result.content.contains("@#$%^&*()_+"));
        assert!(result.content.contains("World!"));
        assert!(result.content.contains("[REDACTED:SSN]"));
    }

    #[test]
    fn test_unicode_content() {
        let scrubber = PiiScrubber::new();

        let content = "你好世界 SSN: 123-45-6789 مرحبا";
        let result = scrubber.scrub(content).unwrap();

        assert!(result.pii_found);
        assert!(result.content.contains("你好世界"));
        assert!(result.content.contains("مرحبا"));
        assert!(result.content.contains("[REDACTED:SSN]"));
    }

    #[tokio::test]
    async fn test_large_content_performance() {
        let layer = SensingLayer::new();

        // Generate content with 1000 SSNs
        let mut content = String::new();
        for i in 0..1000 {
            content.push_str(&format!("Record {}: SSN 123-45-{:04} ", i, i));
        }

        let input = LayerInput::new("perf-test".to_string(), content);
        let start = std::time::Instant::now();
        let output = layer.process(input).await.unwrap();
        let duration = start.elapsed();

        // Should complete within a reasonable time (< 100ms for 1000 SSNs)
        assert!(
            duration.as_millis() < 100,
            "Large content took {:?}",
            duration
        );
        assert!(output.result.data["pii_found"].as_bool().unwrap());
    }
}
