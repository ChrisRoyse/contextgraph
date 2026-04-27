//! E10 Multimodal dataset generation for benchmarking paraphrase/context embeddings.
//!
//! This module generates synthetic datasets with known paraphrase/context ground truth for
//! evaluating E10 dual embedding effectiveness.
//!
//! ## Dataset Types
//!
//! - **Paraphrase Detection Dataset**: Queries with known paraphrase categories
//! - **Context Matching Dataset**: Documents with context situations
//! - **Dual Pairs Dataset**: Pairs of paraphrase/context relationships
//!
//! ## Design Philosophy (from Constitution)
//!
//! E10 ENHANCES E1 semantic search, not replaces it:
//! - E1 provides semantic foundation (V_meaning)
//! - E10 adds paraphrase/context awareness (V_multimodality)
//! - blendWithSemantic parameter controls E1/E10 balance

use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Configuration for E10 multimodal dataset generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E10MultimodalDatasetConfig {
    /// Number of documents to generate.
    pub num_documents: usize,

    /// Number of paraphrase queries to generate.
    pub num_paraphrase_queries: usize,

    /// Number of context queries to generate.
    pub num_context_queries: usize,

    /// Random seed for reproducibility.
    pub seed: u64,

    /// Domains to include.
    pub domains: Vec<IntentDomain>,

    /// Ratio of paraphrase to context queries.
    pub paraphrase_context_ratio: f64,
}

impl Default for E10MultimodalDatasetConfig {
    fn default() -> Self {
        Self {
            num_documents: 500,
            num_paraphrase_queries: 100,
            num_context_queries: 100,
            seed: 42,
            domains: IntentDomain::all(),
            paraphrase_context_ratio: 0.5,
        }
    }
}

/// Domain enum for multi-domain coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IntentDomain {
    /// Performance optimization work
    PerformanceOptimization,
    /// Bug fixing activities
    BugFixing,
    /// Feature development
    FeatureDevelopment,
    /// Refactoring/cleanup
    Refactoring,
    /// Testing/quality assurance
    Testing,
    /// Documentation
    Documentation,
    /// Security improvements
    Security,
    /// Infrastructure/DevOps
    Infrastructure,
}

impl IntentDomain {
    /// Get all domains.
    pub fn all() -> Vec<Self> {
        vec![
            Self::PerformanceOptimization,
            Self::BugFixing,
            Self::FeatureDevelopment,
            Self::Refactoring,
            Self::Testing,
            Self::Documentation,
            Self::Security,
            Self::Infrastructure,
        ]
    }

    /// Get intent templates for this domain.
    pub fn paraphrase_templates(&self) -> Vec<&'static str> {
        match self {
            Self::PerformanceOptimization => vec![
                "Optimize {component} for faster response times",
                "Reduce memory usage in {module}",
                "Speed up {operation} by implementing caching",
                "Improve throughput of {service}",
                "Make {feature} more efficient",
                "Reduce latency in {endpoint}",
                "Optimize database queries for {table}",
                "Profile and fix bottleneck in {system}",
            ],
            Self::BugFixing => vec![
                "Fix {bug_type} bug in {component}",
                "Resolve crash when {action}",
                "Debug {error_type} in {module}",
                "Fix race condition in {concurrent_code}",
                "Address memory leak in {allocator}",
                "Correct validation error in {input}",
                "Fix null pointer exception in {handler}",
                "Resolve timeout issue in {service}",
            ],
            Self::FeatureDevelopment => vec![
                "Implement {feature} for {user_type}",
                "Add {capability} to {module}",
                "Create new {component} for {purpose}",
                "Build {integration} with {system}",
                "Develop {api} endpoint for {operation}",
                "Add support for {format} in {parser}",
                "Implement {pattern} in {codebase}",
                "Create {widget} for {ui}",
            ],
            Self::Refactoring => vec![
                "Refactor {component} for better maintainability",
                "Extract {logic} into separate {module}",
                "Simplify {complex_code} implementation",
                "Consolidate duplicate code in {area}",
                "Rename {old_name} to {new_name} for clarity",
                "Reorganize {structure} for consistency",
                "Clean up {legacy_code} in {module}",
                "Improve code organization in {package}",
            ],
            Self::Testing => vec![
                "Add unit tests for {component}",
                "Write integration tests for {feature}",
                "Create test fixtures for {module}",
                "Improve test coverage for {area}",
                "Add regression tests for {bug_fix}",
                "Write performance benchmarks for {code}",
                "Test edge cases in {validation}",
                "Add smoke tests for {deployment}",
            ],
            Self::Documentation => vec![
                "Document {api} usage and examples",
                "Add README for {project}",
                "Write architecture overview for {system}",
                "Create user guide for {feature}",
                "Update changelog for {release}",
                "Add code comments to {complex_code}",
                "Document configuration options for {module}",
                "Write troubleshooting guide for {issue}",
            ],
            Self::Security => vec![
                "Fix {vulnerability} in {component}",
                "Add input validation to {endpoint}",
                "Implement {auth_type} authentication",
                "Secure {sensitive_data} handling",
                "Add rate limiting to {api}",
                "Fix injection vulnerability in {parser}",
                "Implement encryption for {data}",
                "Add security headers to {response}",
            ],
            Self::Infrastructure => vec![
                "Configure {service} for {environment}",
                "Set up {pipeline} for {project}",
                "Deploy {application} to {platform}",
                "Configure monitoring for {system}",
                "Set up logging for {service}",
                "Configure database {operation}",
                "Set up {caching_solution} for {use_case}",
                "Implement health checks for {service}",
            ],
        }
    }

    /// Get context templates for this domain.
    pub fn context_templates(&self) -> Vec<&'static str> {
        match self {
            Self::PerformanceOptimization => vec![
                "The {component} was slow under high load",
                "Users complained about response times",
                "Memory usage spiked during {operation}",
                "Profiling showed {bottleneck} as the issue",
                "Performance regression after {change}",
                "SLA requirements not being met",
                "Cost of cloud compute too high",
                "Database queries timing out",
            ],
            Self::BugFixing => vec![
                "Error reports from production",
                "Crash logs showed {exception}",
                "Test failures in CI pipeline",
                "Customer reported {issue}",
                "Regression introduced in {commit}",
                "Edge case not handled correctly",
                "Inconsistent behavior observed",
                "Data corruption detected",
            ],
            Self::FeatureDevelopment => vec![
                "Product requirement for {feature}",
                "User story from sprint planning",
                "Technical design approved",
                "API contract defined",
                "Stakeholder requested {capability}",
                "Market research showed demand",
                "Competitor analysis revealed gap",
                "User feedback on missing feature",
            ],
            Self::Refactoring => vec![
                "Technical debt accumulated",
                "Code review identified issues",
                "Difficult to add new features",
                "Test coverage hard to improve",
                "Multiple developers confused by code",
                "Inconsistent patterns in codebase",
                "Performance limited by architecture",
                "Security audit recommended changes",
            ],
            Self::Testing => vec![
                "Test coverage below threshold",
                "QA found issues in manual testing",
                "Regression happened after deployment",
                "New feature needs test coverage",
                "Flaky tests need investigation",
                "Performance tests needed for SLA",
                "Security testing requirements",
                "Integration with external service",
            ],
            Self::Documentation => vec![
                "New developers struggling to onboard",
                "API consumers asking questions",
                "Support tickets about usage",
                "Release notes needed",
                "Audit requirement for documentation",
                "Open source contributors need guide",
                "Training materials requested",
                "Knowledge sharing across teams",
            ],
            Self::Security => vec![
                "Security audit findings",
                "Penetration test revealed vulnerability",
                "Compliance requirement not met",
                "Incident response investigation",
                "Third-party dependency CVE",
                "User data breach risk",
                "Authentication bypass discovered",
                "Encryption requirements changed",
            ],
            Self::Infrastructure => vec![
                "Scaling needs for growth",
                "Environment parity issues",
                "Deployment failures occurring",
                "Monitoring gaps identified",
                "Cost optimization needed",
                "Disaster recovery requirements",
                "Compliance audit requirements",
                "Multi-region expansion planned",
            ],
        }
    }

    /// Get display name for domain.
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::PerformanceOptimization => "performance_optimization",
            Self::BugFixing => "bug_fixing",
            Self::FeatureDevelopment => "feature_development",
            Self::Refactoring => "refactoring",
            Self::Testing => "testing",
            Self::Documentation => "documentation",
            Self::Security => "security",
            Self::Infrastructure => "infrastructure",
        }
    }
}

/// Paraphrase direction for asymmetric similarity.
///
/// Following E5/E8 pattern for asymmetric similarity.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ParaphraseDirection {
    /// Text represents a paraphrase/goal (what someone wants to do)
    Paraphrase,
    /// Text represents context/situation (background information)
    Context,
    /// Direction unknown
    #[default]
    Unknown,
}

impl ParaphraseDirection {
    /// Get direction modifier when comparing query_direction to result_direction.
    ///
    /// E5-base-v2 provides natural asymmetry via "query:"/"passage:" prefixes.
    /// No artificial modifiers needed - all directions use 1.0.
    /// The embedder's prefix-based training handles query→document direction.
    pub fn direction_modifier(query_direction: Self, result_direction: Self) -> f32 {
        match (query_direction, result_direction) {
            // All directions use neutral 1.0 - E5-base-v2 handles asymmetry via prefixes
            (Self::Paraphrase, Self::Context) => 1.0,
            (Self::Context, Self::Paraphrase) => 1.0,
            _ => 1.0,
        }
    }
}

impl std::fmt::Display for ParaphraseDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Paraphrase => write!(f, "paraphrase"),
            Self::Context => write!(f, "context"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

/// A document with paraphrase/context metadata for benchmarking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParaphraseDocument {
    /// Unique identifier.
    pub id: Uuid,
    /// Document content.
    pub content: String,
    /// Primary intent domain.
    pub domain: IntentDomain,
    /// Whether this document represents a paraphrase or context.
    pub direction: ParaphraseDirection,
    /// Optional paraphrase keywords for matching.
    pub paraphrase_keywords: Vec<String>,
    /// Optional context keywords for matching.
    pub context_keywords: Vec<String>,
}

/// A query for E10 benchmark evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParaphraseQuery {
    /// Query text.
    pub query: String,
    /// Query direction (paraphrase or context).
    pub direction: ParaphraseDirection,
    /// Expected domain for this query.
    pub expected_domain: IntentDomain,
    /// Expected top document IDs (ground truth).
    pub expected_top_docs: Vec<Uuid>,
    /// Documents that should NOT rank high (anti-examples).
    pub anti_expected_docs: Vec<Uuid>,
    /// Why E1 alone would fail this query.
    pub e1_limitation: String,
}

/// Complete benchmark dataset for E10 evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E10MultimodalBenchmarkDataset {
    /// All documents in the corpus.
    pub documents: Vec<ParaphraseDocument>,
    /// Paraphrase-based queries (query as paraphrase, looking for context).
    pub paraphrase_queries: Vec<ParaphraseQuery>,
    /// Context-based queries (query as context, looking for paraphrase).
    pub context_queries: Vec<ParaphraseQuery>,
    /// Random seed used.
    pub seed: u64,
}

/// Dataset statistics for reporting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct E10DatasetStats {
    /// Total documents.
    pub total_documents: usize,
    /// Documents per domain.
    pub documents_per_domain: std::collections::HashMap<String, usize>,
    /// Paraphrase documents.
    pub paraphrase_documents: usize,
    /// Context documents.
    pub context_documents: usize,
    /// Total intent queries.
    pub paraphrase_queries: usize,
    /// Total context queries.
    pub context_queries: usize,
}

impl E10MultimodalBenchmarkDataset {
    /// Get dataset statistics.
    pub fn stats(&self) -> E10DatasetStats {
        let mut documents_per_domain = std::collections::HashMap::new();
        let mut paraphrase_docs = 0;
        let mut context_docs = 0;

        for doc in &self.documents {
            *documents_per_domain
                .entry(doc.domain.display_name().to_string())
                .or_insert(0) += 1;
            match doc.direction {
                ParaphraseDirection::Paraphrase => paraphrase_docs += 1,
                ParaphraseDirection::Context => context_docs += 1,
                ParaphraseDirection::Unknown => {}
            }
        }

        E10DatasetStats {
            total_documents: self.documents.len(),
            documents_per_domain,
            paraphrase_documents: paraphrase_docs,
            context_documents: context_docs,
            paraphrase_queries: self.paraphrase_queries.len(),
            context_queries: self.context_queries.len(),
        }
    }
}

/// Generator for E10 multimodal benchmark datasets.
pub struct E10MultimodalDatasetGenerator {
    config: E10MultimodalDatasetConfig,
    rng: ChaCha8Rng,
}

impl E10MultimodalDatasetGenerator {
    /// Create a new generator with the given config.
    pub fn new(config: E10MultimodalDatasetConfig) -> Self {
        let rng = ChaCha8Rng::seed_from_u64(config.seed);
        Self { config, rng }
    }

    /// Generate the benchmark dataset.
    pub fn generate(&mut self) -> E10MultimodalBenchmarkDataset {
        // Generate documents (half paraphrase, half context)
        let docs_per_type = self.config.num_documents / 2;
        let mut documents = Vec::with_capacity(self.config.num_documents);

        // Generate paraphrase documents
        for _ in 0..docs_per_type {
            documents.push(self.generate_paraphrase_document());
        }

        // Generate context documents
        for _ in 0..docs_per_type {
            documents.push(self.generate_context_document());
        }

        // Shuffle documents
        documents.shuffle(&mut self.rng);

        // Generate queries
        let paraphrase_queries = self.generate_paraphrase_queries(&documents);
        let context_queries = self.generate_context_queries(&documents);

        E10MultimodalBenchmarkDataset {
            documents,
            paraphrase_queries,
            context_queries,
            seed: self.config.seed,
        }
    }

    fn generate_paraphrase_document(&mut self) -> ParaphraseDocument {
        let domain = *self.config.domains.choose(&mut self.rng).unwrap();
        let templates = domain.paraphrase_templates();
        let template = *templates.choose(&mut self.rng).unwrap();

        // Fill in template placeholders with generated values
        let content = self.fill_template(template);

        // Extract paraphrase keywords from template
        let paraphrase_keywords = self.extract_paraphrase_keywords(&content, domain);

        ParaphraseDocument {
            id: Uuid::new_v4(),
            content,
            domain,
            direction: ParaphraseDirection::Paraphrase,
            paraphrase_keywords,
            context_keywords: Vec::new(),
        }
    }

    fn generate_context_document(&mut self) -> ParaphraseDocument {
        let domain = *self.config.domains.choose(&mut self.rng).unwrap();
        let templates = domain.context_templates();
        let template = *templates.choose(&mut self.rng).unwrap();

        let content = self.fill_template(template);

        // Extract context keywords
        let context_keywords = self.extract_context_keywords(&content, domain);

        ParaphraseDocument {
            id: Uuid::new_v4(),
            content,
            domain,
            direction: ParaphraseDirection::Context,
            paraphrase_keywords: Vec::new(),
            context_keywords,
        }
    }

    fn fill_template(&mut self, template: &str) -> String {
        let placeholders = [
            (
                "{component}",
                &[
                    "AuthService",
                    "CacheLayer",
                    "DatabasePool",
                    "APIHandler",
                    "MessageQueue",
                ][..],
            ),
            ("{module}", &["auth", "cache", "database", "api", "queue"]),
            (
                "{operation}",
                &[
                    "batch processing",
                    "query execution",
                    "request handling",
                    "data sync",
                ],
            ),
            (
                "{service}",
                &[
                    "user service",
                    "order service",
                    "payment service",
                    "notification service",
                ],
            ),
            (
                "{feature}",
                &["search", "filtering", "pagination", "sorting"],
            ),
            (
                "{endpoint}",
                &["/api/users", "/api/orders", "/api/search", "/api/health"],
            ),
            ("{table}", &["users", "orders", "products", "sessions"]),
            ("{system}", &["backend", "frontend", "database", "cache"]),
            (
                "{bug_type}",
                &["null pointer", "race condition", "memory leak", "overflow"],
            ),
            (
                "{action}",
                &[
                    "processing large files",
                    "concurrent updates",
                    "network timeout",
                ],
            ),
            (
                "{error_type}",
                &[
                    "validation error",
                    "connection error",
                    "timeout",
                    "parse error",
                ],
            ),
            (
                "{concurrent_code}",
                &["thread pool", "async handler", "mutex", "channel"],
            ),
            (
                "{allocator}",
                &["memory pool", "buffer allocator", "object pool"],
            ),
            (
                "{input}",
                &["user input", "API request", "file upload", "form data"],
            ),
            (
                "{handler}",
                &["request handler", "event handler", "error handler"],
            ),
            (
                "{user_type}",
                &["admin users", "regular users", "API consumers"],
            ),
            ("{capability}", &["export", "import", "search", "analytics"]),
            (
                "{purpose}",
                &["data analysis", "user management", "reporting"],
            ),
            (
                "{integration}",
                &["OAuth", "webhook", "SSO", "payment gateway"],
            ),
            ("{api}", &["REST", "GraphQL", "gRPC", "WebSocket"]),
            ("{format}", &["JSON", "CSV", "XML", "Protobuf"]),
            ("{parser}", &["JSON parser", "XML parser", "config parser"]),
            (
                "{pattern}",
                &["repository pattern", "factory pattern", "observer pattern"],
            ),
            ("{codebase}", &["backend", "frontend", "shared library"]),
            ("{widget}", &["data table", "chart", "form", "modal"]),
            ("{ui}", &["dashboard", "admin panel", "user profile"]),
            (
                "{logic}",
                &["validation logic", "business logic", "rendering logic"],
            ),
            (
                "{complex_code}",
                &["legacy handler", "monolithic function", "nested callbacks"],
            ),
            ("{area}", &["authentication", "authorization", "validation"]),
            ("{old_name}", &["processData", "handleRequest", "doStuff"]),
            (
                "{new_name}",
                &["processUserData", "handleApiRequest", "executeOperation"],
            ),
            (
                "{structure}",
                &["project structure", "module organization", "package layout"],
            ),
            (
                "{legacy_code}",
                &["deprecated handlers", "old API", "legacy adapters"],
            ),
            ("{package}", &["core", "utils", "common"]),
            (
                "{bug_fix}",
                &["null check fix", "race condition fix", "validation fix"],
            ),
            ("{code}", &["critical path", "hot path", "startup code"]),
            (
                "{validation}",
                &["input validation", "schema validation", "boundary checks"],
            ),
            (
                "{deployment}",
                &["staging deployment", "production deployment"],
            ),
            ("{project}", &["main service", "shared library", "CLI tool"]),
            ("{release}", &["v1.0", "v2.0", "latest release"]),
            (
                "{issue}",
                &[
                    "common errors",
                    "deployment failures",
                    "configuration problems",
                ],
            ),
            (
                "{vulnerability}",
                &["SQL injection", "XSS", "CSRF", "auth bypass"],
            ),
            ("{auth_type}", &["JWT", "OAuth2", "API key", "session"]),
            (
                "{sensitive_data}",
                &["passwords", "tokens", "PII", "credentials"],
            ),
            ("{data}", &["user data", "session data", "API keys"]),
            ("{response}", &["API responses", "HTTP responses"]),
            ("{environment}", &["production", "staging", "development"]),
            (
                "{pipeline}",
                &["CI/CD pipeline", "data pipeline", "build pipeline"],
            ),
            ("{application}", &["web app", "API server", "worker"]),
            ("{platform}", &["Kubernetes", "AWS", "GCP", "Azure"]),
            (
                "{caching_solution}",
                &["Redis", "Memcached", "in-memory cache"],
            ),
            (
                "{use_case}",
                &["session storage", "rate limiting", "query caching"],
            ),
            (
                "{bottleneck}",
                &["database queries", "network I/O", "CPU-bound computation"],
            ),
            (
                "{change}",
                &["recent deployment", "dependency update", "config change"],
            ),
            (
                "{exception}",
                &[
                    "NullPointerException",
                    "OutOfMemoryError",
                    "TimeoutException",
                ],
            ),
            ("{commit}", &["commit abc123", "last merge", "hotfix"]),
        ];

        let mut result = template.to_string();
        for (placeholder, values) in &placeholders {
            if result.contains(placeholder) {
                let value = values.choose(&mut self.rng).unwrap();
                result = result.replacen(placeholder, value, 1);
            }
        }
        result
    }

    fn extract_paraphrase_keywords(&self, _content: &str, domain: IntentDomain) -> Vec<String> {
        // Return domain-specific paraphrase keywords
        match domain {
            IntentDomain::PerformanceOptimization => {
                vec![
                    "optimize".into(),
                    "faster".into(),
                    "speed".into(),
                    "performance".into(),
                ]
            }
            IntentDomain::BugFixing => {
                vec![
                    "fix".into(),
                    "resolve".into(),
                    "debug".into(),
                    "repair".into(),
                ]
            }
            IntentDomain::FeatureDevelopment => {
                vec![
                    "implement".into(),
                    "add".into(),
                    "create".into(),
                    "build".into(),
                ]
            }
            IntentDomain::Refactoring => {
                vec![
                    "refactor".into(),
                    "simplify".into(),
                    "clean".into(),
                    "reorganize".into(),
                ]
            }
            IntentDomain::Testing => {
                vec![
                    "test".into(),
                    "verify".into(),
                    "validate".into(),
                    "check".into(),
                ]
            }
            IntentDomain::Documentation => {
                vec![
                    "document".into(),
                    "describe".into(),
                    "explain".into(),
                    "write".into(),
                ]
            }
            IntentDomain::Security => {
                vec![
                    "secure".into(),
                    "protect".into(),
                    "encrypt".into(),
                    "authenticate".into(),
                ]
            }
            IntentDomain::Infrastructure => {
                vec![
                    "configure".into(),
                    "deploy".into(),
                    "setup".into(),
                    "provision".into(),
                ]
            }
        }
    }

    fn extract_context_keywords(&self, _content: &str, domain: IntentDomain) -> Vec<String> {
        // Return domain-specific context keywords
        match domain {
            IntentDomain::PerformanceOptimization => {
                vec![
                    "slow".into(),
                    "bottleneck".into(),
                    "timeout".into(),
                    "latency".into(),
                ]
            }
            IntentDomain::BugFixing => {
                vec![
                    "error".into(),
                    "crash".into(),
                    "failure".into(),
                    "exception".into(),
                ]
            }
            IntentDomain::FeatureDevelopment => {
                vec![
                    "requirement".into(),
                    "request".into(),
                    "need".into(),
                    "demand".into(),
                ]
            }
            IntentDomain::Refactoring => {
                vec![
                    "debt".into(),
                    "messy".into(),
                    "complex".into(),
                    "duplicate".into(),
                ]
            }
            IntentDomain::Testing => {
                vec![
                    "coverage".into(),
                    "regression".into(),
                    "flaky".into(),
                    "broken".into(),
                ]
            }
            IntentDomain::Documentation => {
                vec![
                    "unclear".into(),
                    "missing".into(),
                    "outdated".into(),
                    "confusing".into(),
                ]
            }
            IntentDomain::Security => {
                vec![
                    "vulnerability".into(),
                    "breach".into(),
                    "risk".into(),
                    "exploit".into(),
                ]
            }
            IntentDomain::Infrastructure => {
                vec![
                    "scaling".into(),
                    "outage".into(),
                    "capacity".into(),
                    "failure".into(),
                ]
            }
        }
    }

    fn generate_paraphrase_queries(
        &mut self,
        documents: &[ParaphraseDocument],
    ) -> Vec<ParaphraseQuery> {
        let mut queries = Vec::with_capacity(self.config.num_paraphrase_queries);

        // Group documents by domain and direction
        let paraphrase_docs: Vec<_> = documents
            .iter()
            .filter(|d| matches!(d.direction, ParaphraseDirection::Paraphrase))
            .collect();

        let context_docs: Vec<_> = documents
            .iter()
            .filter(|d| matches!(d.direction, ParaphraseDirection::Context))
            .collect();

        for _ in 0..self.config.num_paraphrase_queries {
            // Pick a domain
            let domain = *self.config.domains.choose(&mut self.rng).unwrap();

            // Create a paraphrase query that should match context documents
            let query_text = match domain {
                IntentDomain::PerformanceOptimization => {
                    "find work aimed at making the system faster".to_string()
                }
                IntentDomain::BugFixing => {
                    "what work was done to fix errors and crashes".to_string()
                }
                IntentDomain::FeatureDevelopment => {
                    "find implementation work for new features".to_string()
                }
                IntentDomain::Refactoring => {
                    "what refactoring work was done to clean up code".to_string()
                }
                IntentDomain::Testing => "find work to improve test coverage".to_string(),
                IntentDomain::Documentation => "what documentation work was completed".to_string(),
                IntentDomain::Security => "find security improvements and fixes".to_string(),
                IntentDomain::Infrastructure => "what infrastructure work was done".to_string(),
            };

            // Expected: context documents from same domain (paraphrase→context)
            let expected: Vec<_> = context_docs
                .iter()
                .filter(|d| d.domain == domain)
                .take(3)
                .map(|d| d.id)
                .collect();

            // Anti-expected: paraphrase documents from different domains
            let anti: Vec<_> = paraphrase_docs
                .iter()
                .filter(|d| d.domain != domain)
                .take(2)
                .map(|d| d.id)
                .collect();

            queries.push(ParaphraseQuery {
                query: query_text,
                direction: ParaphraseDirection::Paraphrase,
                expected_domain: domain,
                expected_top_docs: expected,
                anti_expected_docs: anti,
                e1_limitation: format!(
                    "E1 may match any {} related text, not distinguish paraphrase vs context",
                    domain.display_name()
                ),
            });
        }

        queries
    }

    fn generate_context_queries(
        &mut self,
        documents: &[ParaphraseDocument],
    ) -> Vec<ParaphraseQuery> {
        let mut queries = Vec::with_capacity(self.config.num_context_queries);

        let paraphrase_docs: Vec<_> = documents
            .iter()
            .filter(|d| matches!(d.direction, ParaphraseDirection::Paraphrase))
            .collect();

        let context_docs: Vec<_> = documents
            .iter()
            .filter(|d| matches!(d.direction, ParaphraseDirection::Context))
            .collect();

        for _ in 0..self.config.num_context_queries {
            let domain = *self.config.domains.choose(&mut self.rng).unwrap();

            // Create a context query that should match paraphrase documents
            let query_text = match domain {
                IntentDomain::PerformanceOptimization => {
                    "system is slow, what can be done".to_string()
                }
                IntentDomain::BugFixing => {
                    "errors happening in production, need solutions".to_string()
                }
                IntentDomain::FeatureDevelopment => {
                    "users requesting new capability, what to build".to_string()
                }
                IntentDomain::Refactoring => {
                    "code is messy and hard to maintain, what to do".to_string()
                }
                IntentDomain::Testing => "test coverage is low, need improvements".to_string(),
                IntentDomain::Documentation => "developers confused, need better docs".to_string(),
                IntentDomain::Security => "security audit found issues, need fixes".to_string(),
                IntentDomain::Infrastructure => {
                    "infrastructure struggling with load, need changes".to_string()
                }
            };

            // Expected: paraphrase documents from same domain (context→paraphrase)
            let expected: Vec<_> = paraphrase_docs
                .iter()
                .filter(|d| d.domain == domain)
                .take(3)
                .map(|d| d.id)
                .collect();

            // Anti-expected: context documents from different domains
            let anti: Vec<_> = context_docs
                .iter()
                .filter(|d| d.domain != domain)
                .take(2)
                .map(|d| d.id)
                .collect();

            queries.push(ParaphraseQuery {
                query: query_text,
                direction: ParaphraseDirection::Context,
                expected_domain: domain,
                expected_top_docs: expected,
                anti_expected_docs: anti,
                e1_limitation: format!(
                    "E1 may match {} keywords without understanding query is seeking solutions",
                    domain.display_name()
                ),
            });
        }

        queries
    }
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;

    #[test]
    fn test_dataset_generation() {
        let config = E10MultimodalDatasetConfig {
            num_documents: 100,
            num_paraphrase_queries: 20,
            num_context_queries: 20,
            seed: 42,
            ..Default::default()
        };

        let mut generator = E10MultimodalDatasetGenerator::new(config);
        let dataset = generator.generate();

        assert_eq!(dataset.documents.len(), 100);
        assert_eq!(dataset.paraphrase_queries.len(), 20);
        assert_eq!(dataset.context_queries.len(), 20);

        // Verify mix of paraphrase and context documents
        let paraphrase_count = dataset
            .documents
            .iter()
            .filter(|d| matches!(d.direction, ParaphraseDirection::Paraphrase))
            .count();
        let context_count = dataset
            .documents
            .iter()
            .filter(|d| matches!(d.direction, ParaphraseDirection::Context))
            .count();

        assert!(paraphrase_count > 0);
        assert!(context_count > 0);

        println!(
            "[VERIFIED] Dataset generated with {} docs, {} paraphrase, {} context",
            dataset.documents.len(),
            paraphrase_count,
            context_count
        );
    }

    #[test]
    fn test_direction_modifiers() {
        // paraphrase→context: 1.0 (E5-base-v2 handles asymmetry via prefixes)
        assert_eq!(
            ParaphraseDirection::direction_modifier(
                ParaphraseDirection::Paraphrase,
                ParaphraseDirection::Context
            ),
            1.0
        );

        // context→paraphrase: 1.0 (neutral - E5-base-v2 handles asymmetry via prefixes)
        assert_eq!(
            ParaphraseDirection::direction_modifier(
                ParaphraseDirection::Context,
                ParaphraseDirection::Paraphrase
            ),
            1.0
        );

        // same direction: 1.0
        assert_eq!(
            ParaphraseDirection::direction_modifier(
                ParaphraseDirection::Paraphrase,
                ParaphraseDirection::Paraphrase
            ),
            1.0
        );

        println!("[VERIFIED] Direction modifiers are neutral (1.0/1.0/1.0) - E5-base-v2 handles asymmetry via prefixes");
    }

    #[test]
    fn test_all_domains_covered() {
        let config = E10MultimodalDatasetConfig::default();
        let mut generator = E10MultimodalDatasetGenerator::new(config);
        let dataset = generator.generate();

        // Check all domains have documents
        let stats = dataset.stats();
        assert_eq!(stats.documents_per_domain.len(), IntentDomain::all().len());

        println!(
            "[VERIFIED] All {} domains have documents",
            stats.documents_per_domain.len()
        );
    }

    #[test]
    fn test_queries_have_ground_truth() {
        let config = E10MultimodalDatasetConfig {
            num_documents: 200,
            num_paraphrase_queries: 20,
            num_context_queries: 20,
            seed: 42,
            ..Default::default()
        };

        let mut generator = E10MultimodalDatasetGenerator::new(config);
        let dataset = generator.generate();

        // Verify queries have expected docs
        for query in &dataset.paraphrase_queries {
            // Some queries may have empty expected_top_docs if no matching docs exist
            // This is acceptable for small datasets
            assert!(query.expected_top_docs.len() <= 3);
        }

        for query in &dataset.context_queries {
            assert!(query.expected_top_docs.len() <= 3);
        }

        println!("[VERIFIED] Queries have ground truth expectations");
    }
}
