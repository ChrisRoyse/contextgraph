//! Configuration management for the Context Graph system.

pub mod constants;
pub mod embedder_config;
mod sub_configs;

#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::{CoreError, CoreResult};

// Re-export all sub-config types for backwards compatibility
pub use sub_configs::{
    CudaConfig, EmbeddingConfig, IndexConfig, LoggingConfig, McpConfig, ServerConfig,
    StorageConfig, UtlConfig, WatcherConfig,
};

// Re-export embedder configuration types (TASK-L04)
pub use embedder_config::{
    check_deprecated_names, deserialize_embedder, deserialize_embedder_opt, DeprecatedNameUsage,
    EmbedderConfig, EmbedderWeightsConfig,
};

/// System development phase.
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    /// Ghost system phase - stubs and scaffolding
    #[default]
    Ghost,
    /// Development phase - active implementation
    Development,
    /// Production phase - fully operational
    Production,
}

/// Main configuration structure.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    /// Current system phase
    #[serde(default)]
    pub phase: Phase,
    pub server: ServerConfig,
    pub mcp: McpConfig,
    pub logging: LoggingConfig,
    pub storage: StorageConfig,
    pub embedding: EmbeddingConfig,
    pub index: IndexConfig,
    pub utl: UtlConfig,
    pub cuda: CudaConfig,
    /// File watcher configuration for ./docs/ monitoring
    #[serde(default)]
    pub watcher: WatcherConfig,
}

impl Config {
    /// Load configuration from files and environment.
    ///
    /// Configuration is loaded in order:
    /// 1. config/default.toml (base settings)
    /// 2. config/{CONTEXT_GRAPH_ENV}.toml (environment-specific)
    /// 3. Environment variables with CONTEXT_GRAPH_ prefix
    pub fn load() -> CoreResult<Self> {
        let env = std::env::var("CONTEXT_GRAPH_ENV").unwrap_or_else(|_| "development".to_string());

        let builder = config::Config::builder()
            .add_source(config::File::with_name("config/default").required(false))
            .add_source(config::File::with_name(&format!("config/{}", env)).required(false))
            .add_source(config::Environment::with_prefix("CONTEXT_GRAPH").separator("__"));

        let config: Config = builder.build()?.try_deserialize()?;
        config.validate()?;
        Ok(config)
    }

    /// Load configuration with defaults for testing/development.
    pub fn default_config() -> Self {
        Self {
            phase: Phase::default(),
            server: ServerConfig::default(),
            mcp: McpConfig::default(),
            logging: LoggingConfig::default(),
            storage: StorageConfig::default(),
            embedding: EmbeddingConfig::default(),
            index: IndexConfig::default(),
            utl: UtlConfig::default(),
            cuda: CudaConfig::default(),
            watcher: WatcherConfig::default(),
        }
    }

    /// Load configuration from a TOML file.
    pub fn from_file(path: &std::path::Path) -> CoreResult<Self> {
        let content = std::fs::read_to_string(path).map_err(|e| {
            CoreError::ConfigError(format!(
                "Failed to read config file {}: {}",
                path.display(),
                e
            ))
        })?;

        let config: Config = toml::from_str(&content)
            .map_err(|e| CoreError::ConfigError(format!("Failed to parse config file: {}", e)))?;

        config.validate()?;
        Ok(config)
    }

    /// Validate configuration values.
    ///
    /// # Phase-Aware Validation
    ///
    /// - **Ghost**: Allows stubs and in-memory backends (development scaffolding)
    /// - **Development**: Warns about stubs but allows them for active development
    /// - **Production**: FAILS if any stubs or in-memory backends are configured
    ///
    /// This prevents accidentally running Production with fake embeddings or
    /// ephemeral storage that loses all data on restart.
    ///
    /// # TASK-INTEG-017 Update
    ///
    /// Added call to `self.mcp.validate()` to validate TCP transport configuration.
    pub fn validate(&self) -> CoreResult<()> {
        // TASK-INTEG-017: Validate MCP config including TCP transport fields
        // FAIL FAST: Any invalid MCP config stops startup immediately
        self.mcp.validate()?;

        if self.storage.backend != "memory" {
            let path = PathBuf::from(&self.storage.path);
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() && !parent.exists() {
                    return Err(CoreError::ConfigError(format!(
                        "storage.path parent directory does not exist: {}",
                        parent.display()
                    )));
                }
            }
        }

        // Phase-aware validation
        self.validate_phase_safety()?;

        Ok(())
    }

    /// Validate that configuration is safe for the current phase.
    ///
    /// # Constitution Compliance
    ///
    /// Per AP-007: No stubs or fallbacks in production code paths.
    /// This method enforces that Production phase CANNOT use:
    /// - `embedding.model = "stub"` (fake embeddings)
    /// - `storage.backend = "memory"` (ephemeral storage)
    /// - `index.backend = "memory"` (ephemeral HNSW)
    /// - `utl.mode = "stub"` (fake UTL processing)
    fn validate_phase_safety(&self) -> CoreResult<()> {
        // Collect all stub/dangerous configurations
        let mut dangerous_configs = Vec::new();

        if self.embedding.model == "stub" {
            dangerous_configs.push(
                "embedding.model = \"stub\" → System will use FAKE embeddings that return \
                 deterministic garbage. ALL similarity computations will be MEANINGLESS.",
            );
        }

        if self.storage.backend == "memory" {
            dangerous_configs.push(
                "storage.backend = \"memory\" → ALL DATA WILL BE LOST on restart. \
                 No persistence, no durability, no recovery.",
            );
        }

        if self.index.backend == "memory" {
            dangerous_configs.push(
                "index.backend = \"memory\" → HNSW indexes are ephemeral. \
                 No persistent vector search capability.",
            );
        }

        if self.utl.mode == "stub" {
            dangerous_configs.push(
                "utl.mode = \"stub\" → UTL computations will use FAKE processing. \
                 Learning scores, consolidation, and lifecycle are MEANINGLESS.",
            );
        }

        // Phase-specific handling
        // Check if MCP mode suppresses warnings (for clean stdio MCP protocol)
        let suppress_warnings = std::env::var("CONTEXT_GRAPH_MCP_QUIET").is_ok();

        match self.phase {
            Phase::Ghost => {
                // Ghost phase allows stubs - it's the scaffolding phase
                // But emit warning to stderr so developers know what's happening
                // Unless CONTEXT_GRAPH_MCP_QUIET is set (for MCP servers)
                if !dangerous_configs.is_empty() && !suppress_warnings {
                    eprintln!(
                        "\n╔════════════════════════════════════════════════════════════════╗"
                    );
                    eprintln!("║ ⚠️  GHOST PHASE: Running with stub/development configuration   ║");
                    eprintln!("╠════════════════════════════════════════════════════════════════╣");
                    for config in &dangerous_configs {
                        eprintln!("║ • {}", config.split('→').next().unwrap_or(config).trim());
                    }
                    eprintln!(
                        "╚════════════════════════════════════════════════════════════════╝\n"
                    );
                }
                Ok(())
            }
            Phase::Development => {
                // Development phase warns but allows stubs for active development
                // Unless CONTEXT_GRAPH_MCP_QUIET is set (for MCP servers)
                if !dangerous_configs.is_empty() && !suppress_warnings {
                    eprintln!(
                        "\n╔════════════════════════════════════════════════════════════════╗"
                    );
                    eprintln!("║ ⚠️  DEVELOPMENT PHASE: Stub configurations detected            ║");
                    eprintln!("║ You should be implementing real backends, not using stubs!     ║");
                    eprintln!("╠════════════════════════════════════════════════════════════════╣");
                    for config in &dangerous_configs {
                        eprintln!("║ • {}", config.split('→').next().unwrap_or(config).trim());
                    }
                    eprintln!(
                        "╚════════════════════════════════════════════════════════════════╝\n"
                    );
                }
                Ok(())
            }
            Phase::Production => {
                // Production phase FAILS HARD if any stubs are detected
                if !dangerous_configs.is_empty() {
                    let mut error_msg =
                        String::from("PRODUCTION PHASE SAFETY VIOLATION - REFUSING TO START\n\n");
                    error_msg.push_str("The following dangerous configurations were detected:\n\n");

                    for (i, config) in dangerous_configs.iter().enumerate() {
                        error_msg.push_str(&format!("{}. {}\n\n", i + 1, config));
                    }

                    error_msg.push_str("REMEDIATION:\n");
                    error_msg.push_str(
                        "  • Set embedding.model to a real model (e.g., \"multi_array_13\")\n",
                    );
                    error_msg.push_str("  • Set storage.backend to \"rocksdb\" with valid path\n");
                    error_msg.push_str("  • Set index.backend to \"hnsw\" with persistence\n");
                    error_msg
                        .push_str("  • Set utl.mode to \"real\" for actual UTL computation\n\n");
                    error_msg.push_str("Or, if testing, set phase = \"ghost\" or \"development\"");

                    return Err(CoreError::ConfigError(error_msg));
                }
                Ok(())
            }
        }
    }

    /// Check if configuration uses any stub or in-memory backends.
    ///
    /// Returns true if any stub configurations are detected.
    pub fn uses_stubs(&self) -> bool {
        self.embedding.model == "stub"
            || self.storage.backend == "memory"
            || self.index.backend == "memory"
            || self.utl.mode == "stub"
    }

    /// Check if this is a production-safe configuration.
    ///
    /// Returns true only if NO stubs are used and phase is Production.
    pub fn is_production_safe(&self) -> bool {
        self.phase == Phase::Production && !self.uses_stubs()
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::default_config()
    }
}
