//! Sub-configuration structures for Context Graph components.
//!
//! This module contains all the individual configuration structs
//! that make up the main `Config` structure.

use serde::{Deserialize, Serialize};

/// Server configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub name: String,
    pub version: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            name: "context-graph".to_string(),
            version: "0.1.0-ghost".to_string(),
        }
    }
}

/// MCP (Model Context Protocol) configuration.
///
/// TASK-INTEG-017: Extended with TCP transport configuration fields.
/// TASK-42: Extended with SSE transport configuration fields.
/// Supports stdio (default), TCP, and SSE transports.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpConfig {
    /// Transport type: "stdio", "tcp", "http", or "sse"
    #[serde(default = "default_transport")]
    pub transport: String,

    /// Maximum payload size in bytes (default: 10MB)
    #[serde(default = "default_max_payload_size")]
    pub max_payload_size: usize,

    /// Request timeout in seconds (default: 30)
    #[serde(default = "default_request_timeout")]
    pub request_timeout: u64,

    /// TCP/SSE bind address (default: "127.0.0.1")
    /// Used when transport = "tcp" or "sse"
    #[serde(default = "default_bind_address")]
    pub bind_address: String,

    /// TCP port number (default: 3100)
    /// Only used when transport = "tcp"
    #[serde(default = "default_tcp_port")]
    pub tcp_port: u16,

    /// SSE/Streamable HTTP port number (default: 3101)
    /// Used when transport = "sse" or "http"
    /// TASK-42: Added for SSE transport support
    #[serde(default = "default_sse_port")]
    pub sse_port: u16,

    /// Maximum concurrent TCP/SSE/HTTP connections (default: 32)
    /// Used when transport = "tcp", "sse", or "http"
    #[serde(default = "default_max_connections")]
    pub max_connections: usize,
}

// ============================================================================
// Serde Default Functions for McpConfig
// ============================================================================

fn default_transport() -> String {
    "stdio".to_string()
}

fn default_max_payload_size() -> usize {
    10_485_760 // 10MB
}

fn default_request_timeout() -> u64 {
    30
}

fn default_bind_address() -> String {
    "127.0.0.1".to_string()
}

fn default_tcp_port() -> u16 {
    3100
}

/// TASK-42: Default SSE port
fn default_sse_port() -> u16 {
    3101
}

fn default_max_connections() -> usize {
    32
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            transport: default_transport(),
            max_payload_size: default_max_payload_size(),
            request_timeout: default_request_timeout(),
            bind_address: default_bind_address(),
            tcp_port: default_tcp_port(),
            sse_port: default_sse_port(), // TASK-42
            max_connections: default_max_connections(),
        }
    }
}

impl McpConfig {
    /// Validate the MCP configuration.
    ///
    /// TASK-INTEG-017: Validates all MCP configuration fields.
    /// TASK-42: Extended to support SSE transport validation.
    /// FAIL FAST: Returns error immediately on invalid configuration.
    ///
    /// # Validation Rules
    ///
    /// - `transport`: Must be "stdio", "tcp", "http", or "sse" (case-insensitive)
    /// - `max_payload_size`: Must be > 0
    /// - `request_timeout`: Must be > 0
    /// - `bind_address`: Must be non-empty when transport = "tcp", "http", or "sse"
    /// - `tcp_port`: Must be in valid range (1-65535) when transport = "tcp"
    /// - `sse_port`: Must be in valid range (1-65535) when transport = "http" or "sse"
    /// - `max_connections`: Must be > 0 when transport = "tcp", "http", or "sse"
    ///
    /// # Errors
    ///
    /// Returns `CoreError::ConfigError` with descriptive message for validation failures.
    pub fn validate(&self) -> crate::error::CoreResult<()> {
        use crate::error::CoreError;

        // Validate transport type (TASK-42: Added "sse"; 2026-05-10 added Streamable HTTP)
        let transport_lower = self.transport.to_lowercase();
        if transport_lower != "stdio"
            && transport_lower != "tcp"
            && transport_lower != "http"
            && transport_lower != "sse"
        {
            return Err(CoreError::ConfigError(format!(
                "McpConfig validation failed: transport must be 'stdio', 'tcp', 'http', or 'sse', got '{}'",
                self.transport
            )));
        }

        // Validate max_payload_size
        if self.max_payload_size == 0 {
            return Err(CoreError::ConfigError(
                "McpConfig validation failed: max_payload_size must be > 0".to_string(),
            ));
        }

        // Validate request_timeout
        if self.request_timeout == 0 {
            return Err(CoreError::ConfigError(
                "McpConfig validation failed: request_timeout must be > 0".to_string(),
            ));
        }

        // TCP-specific validation
        if transport_lower == "tcp" {
            // Validate bind_address
            if self.bind_address.trim().is_empty() {
                return Err(CoreError::ConfigError(
                    "McpConfig validation failed: bind_address must be non-empty for TCP transport"
                        .to_string(),
                ));
            }

            // Validate tcp_port (u16 is already 0-65535, but 0 is reserved)
            if self.tcp_port == 0 {
                return Err(CoreError::ConfigError(
                    "McpConfig validation failed: tcp_port must be in range 1-65535, got 0"
                        .to_string(),
                ));
            }

            // Validate max_connections
            if self.max_connections == 0 {
                return Err(CoreError::ConfigError(
                    "McpConfig validation failed: max_connections must be > 0 for TCP transport"
                        .to_string(),
                ));
            }
        }

        // Streamable HTTP/SSE validation. Both use sse_port as the configured
        // HTTP port to preserve the existing config shape.
        if transport_lower == "sse" || transport_lower == "http" {
            // Validate bind_address
            if self.bind_address.trim().is_empty() {
                return Err(CoreError::ConfigError(
                    "McpConfig validation failed: bind_address must be non-empty for HTTP/SSE transport"
                        .to_string(),
                ));
            }

            // Validate sse_port (u16 is already 0-65535, but 0 is reserved)
            if self.sse_port == 0 {
                return Err(CoreError::ConfigError(
                    "McpConfig validation failed: sse_port must be in range 1-65535 for HTTP/SSE transport, got 0"
                        .to_string(),
                ));
            }

            // Validate max_connections
            if self.max_connections == 0 {
                return Err(CoreError::ConfigError(
                    "McpConfig validation failed: max_connections must be > 0 for HTTP/SSE transport"
                        .to_string(),
                ));
            }
        }

        Ok(())
    }
}

/// Logging configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    pub level: String,
    pub format: String,
    pub include_location: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "pretty".to_string(),
            include_location: false,
        }
    }
}

/// Storage backend configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    pub backend: String,
    pub path: String,
    pub compression: bool,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            backend: "memory".to_string(),
            path: "./data/storage".to_string(),
            compression: true,
        }
    }
}

/// Embedding model configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EmbeddingConfig {
    pub model: String,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            model: "stub".to_string(),
        }
    }
}

/// Index backend configuration (HNSW parameters).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IndexConfig {
    pub backend: String,
    pub hnsw_m: usize,
    pub hnsw_ef_construction: usize,
}

impl Default for IndexConfig {
    fn default() -> Self {
        Self {
            backend: "memory".to_string(),
            hnsw_m: 16,
            hnsw_ef_construction: 200,
        }
    }
}

/// UTL (Unified Theory of Learning) configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct UtlConfig {
    pub mode: String,
    pub consolidation_threshold: f32,
}

impl Default for UtlConfig {
    fn default() -> Self {
        Self {
            mode: "stub".to_string(),
            consolidation_threshold: 0.7,
        }
    }
}

/// CUDA/GPU configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CudaConfig {
    pub device_id: u32,
    pub memory_limit_gb: f32,
}

impl Default for CudaConfig {
    fn default() -> Self {
        Self {
            device_id: 0,
            memory_limit_gb: 4.0,
        }
    }
}

/// File watcher configuration for monitoring markdown documentation.
///
/// When enabled, the MCP server will monitor the specified directories
/// for .md file changes and automatically index them as memories with
/// MDFileChunk source metadata.
///
/// # Example Configuration
///
/// ```toml
/// [watcher]
/// enabled = true
/// watch_paths = ["./docs"]
/// session_id = "docs-watcher"
/// ```
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WatcherConfig {
    /// Enable the file watcher (default: false).
    #[serde(default)]
    pub enabled: bool,

    /// Directories to watch for .md files (default: ["./docs"]).
    /// Subdirectories are watched recursively.
    #[serde(default = "default_watch_paths")]
    pub watch_paths: Vec<String>,

    /// Session ID for captured memories (default: "docs-watcher").
    #[serde(default = "default_watcher_session_id")]
    pub session_id: String,
}

fn default_watch_paths() -> Vec<String> {
    vec!["./docs".to_string()]
}

fn default_watcher_session_id() -> String {
    "docs-watcher".to_string()
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            // Disabled by default - enable via config or CONTEXT_GRAPH_WATCHER_ENABLED=true
            enabled: false,
            watch_paths: default_watch_paths(),
            session_id: default_watcher_session_id(),
        }
    }
}
