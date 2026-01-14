//! Context Graph MCP Server
//!
//! JSON-RPC 2.0 server implementing the Model Context Protocol (MCP)
//! for the Ultimate Context Graph system.
//!
//! # Transport
//!
//! - stdio: Standard input/output (default)
//! - tcp: TCP socket transport for networked deployments
//!
//! # Usage
//!
//! ```bash
//! # Run with default configuration (stdio transport)
//! context-graph-mcp
//!
//! # Run with custom config
//! context-graph-mcp --config /path/to/config.toml
//!
//! # Run with TCP transport (uses config defaults for port/address)
//! context-graph-mcp --transport tcp
//!
//! # Run with TCP transport on custom port
//! context-graph-mcp --transport tcp --port 4000
//!
//! # Run with TCP transport on custom address
//! context-graph-mcp --transport tcp --bind 0.0.0.0 --port 3100
//!
//! # Environment variable override (used if CLI not specified)
//! CONTEXT_GRAPH_TRANSPORT=tcp context-graph-mcp
//!
//! # Run in debug mode
//! RUST_LOG=debug context-graph-mcp
//! ```
//!
//! # CLI Argument Priority (TASK-INTEG-019)
//!
//! CLI arguments > Environment variables > Config file > Defaults
//! - `--transport` overrides `CONTEXT_GRAPH_TRANSPORT`, `config.mcp.transport`
//! - `--port` overrides `CONTEXT_GRAPH_TCP_PORT`, `config.mcp.tcp_port`
//! - `--bind` overrides `CONTEXT_GRAPH_BIND_ADDRESS`, `config.mcp.bind_address`

mod adapters;
mod handlers;
mod middleware;
mod protocol;
mod server;
mod tools;
mod transport;
mod weights;

use std::env;
use std::io;
use std::path::PathBuf;

use anyhow::Result;
use tracing::{error, info};
use tracing_subscriber::{fmt, EnvFilter};

use context_graph_core::config::Config;
use server::TransportMode;

// ============================================================================
// CLI Argument Parsing
// ============================================================================

/// Parsed CLI arguments for the MCP server.
///
/// TASK-INTEG-019: Simple argument parsing without external dependencies.
/// TASK-42: Added sse_port for SSE transport.
struct CliArgs {
    /// Path to configuration file
    config_path: Option<PathBuf>,
    /// Transport mode override (--transport)
    transport: Option<String>,
    /// TCP port override (--port)
    port: Option<u16>,
    /// SSE port override (--sse-port, TASK-42)
    sse_port: Option<u16>,
    /// TCP/SSE bind address override (--bind)
    bind_address: Option<String>,
    /// Show help
    help: bool,
}

impl CliArgs {
    /// Parse CLI arguments.
    ///
    /// TASK-INTEG-019: Manual parsing without clap to keep binary small.
    /// Supports: --config, --transport, --port, --bind, --help, -h
    fn parse() -> Self {
        let args: Vec<String> = env::args().collect();
        let mut cli = CliArgs {
            config_path: None,
            transport: None,
            port: None,
            sse_port: None,
            bind_address: None,
            help: false,
        };

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--help" | "-h" => {
                    cli.help = true;
                }
                "--config" => {
                    i += 1;
                    if i < args.len() {
                        cli.config_path = Some(PathBuf::from(&args[i]));
                    }
                }
                "--transport" => {
                    i += 1;
                    if i < args.len() {
                        cli.transport = Some(args[i].clone());
                    }
                }
                "--port" => {
                    i += 1;
                    if i < args.len() {
                        if let Ok(port) = args[i].parse::<u16>() {
                            cli.port = Some(port);
                        }
                    }
                }
                "--sse-port" => {
                    // TASK-42: SSE port argument
                    i += 1;
                    if i < args.len() {
                        if let Ok(port) = args[i].parse::<u16>() {
                            cli.sse_port = Some(port);
                        }
                    }
                }
                "--bind" => {
                    i += 1;
                    if i < args.len() {
                        cli.bind_address = Some(args[i].clone());
                    }
                }
                _ => {} // Ignore unknown arguments
            }
            i += 1;
        }

        cli
    }
}

/// Print help message and exit.
fn print_help() {
    eprintln!(
        r#"Context Graph MCP Server

USAGE:
    context-graph-mcp [OPTIONS]

OPTIONS:
    --config <PATH>      Path to configuration file
    --transport <MODE>   Transport mode: stdio (default), tcp, or sse
    --port <PORT>        TCP port (only used with --transport tcp)
    --sse-port <PORT>    SSE port (only used with --transport sse, default: 3101)
    --bind <ADDRESS>     TCP/SSE bind address (default: 127.0.0.1)
    --help, -h           Show this help message

ENVIRONMENT VARIABLES:
    CONTEXT_GRAPH_TRANSPORT     Transport mode (stdio|tcp|sse)
    CONTEXT_GRAPH_TCP_PORT      TCP port number
    CONTEXT_GRAPH_SSE_PORT      SSE port number (TASK-42)
    CONTEXT_GRAPH_BIND_ADDRESS  TCP/SSE bind address
    RUST_LOG                    Log level (error, warn, info, debug, trace)

PRIORITY:
    CLI arguments > Environment variables > Config file > Defaults

EXAMPLES:
    # Run with stdio transport (default)
    context-graph-mcp

    # Run with TCP transport on default port (3100)
    context-graph-mcp --transport tcp

    # Run with TCP transport on custom port
    context-graph-mcp --transport tcp --port 4000

    # Run with TCP on all interfaces
    context-graph-mcp --transport tcp --bind 0.0.0.0 --port 3100

    # Run with SSE transport on default port (3101)
    context-graph-mcp --transport sse

    # Run with SSE transport on custom port
    context-graph-mcp --transport sse --sse-port 8080

    # Run with custom config file
    context-graph-mcp --config /path/to/config.toml
"#
    );
}

/// Determine transport mode from CLI, env, config.
///
/// Priority: CLI > ENV > Config > Default (Stdio)
///
/// TASK-INTEG-019: FAIL FAST if invalid transport is specified.
/// TASK-42: Added SSE transport support.
fn determine_transport_mode(cli: &CliArgs, config: &Config) -> Result<TransportMode> {
    // CLI takes highest priority
    if let Some(ref transport) = cli.transport {
        let transport_lower = transport.to_lowercase();
        return match transport_lower.as_str() {
            "stdio" => Ok(TransportMode::Stdio),
            "tcp" => Ok(TransportMode::Tcp),
            "sse" => Ok(TransportMode::Sse),
            _ => {
                error!(
                    "FATAL: Invalid transport '{}' from CLI. Must be 'stdio', 'tcp', or 'sse'.",
                    transport
                );
                Err(anyhow::anyhow!(
                    "Invalid transport '{}'. Must be 'stdio', 'tcp', or 'sse'.",
                    transport
                ))
            }
        };
    }

    // Environment variable is second priority
    if let Ok(transport) = env::var("CONTEXT_GRAPH_TRANSPORT") {
        let transport_lower = transport.to_lowercase();
        return match transport_lower.as_str() {
            "stdio" => Ok(TransportMode::Stdio),
            "tcp" => Ok(TransportMode::Tcp),
            "sse" => Ok(TransportMode::Sse),
            _ => {
                error!(
                    "FATAL: Invalid CONTEXT_GRAPH_TRANSPORT='{}'. Must be 'stdio', 'tcp', or 'sse'.",
                    transport
                );
                Err(anyhow::anyhow!(
                    "Invalid CONTEXT_GRAPH_TRANSPORT='{}'. Must be 'stdio', 'tcp', or 'sse'.",
                    transport
                ))
            }
        };
    }

    // Config file is third priority
    let transport_lower = config.mcp.transport.to_lowercase();
    match transport_lower.as_str() {
        "stdio" => Ok(TransportMode::Stdio),
        "tcp" => Ok(TransportMode::Tcp),
        "sse" => Ok(TransportMode::Sse),
        _ => {
            // This should not happen if Config::validate() passed, but FAIL FAST anyway
            error!(
                "FATAL: Invalid transport '{}' in config. Must be 'stdio', 'tcp', or 'sse'.",
                config.mcp.transport
            );
            Err(anyhow::anyhow!(
                "Invalid transport '{}' in config. Must be 'stdio', 'tcp', or 'sse'.",
                config.mcp.transport
            ))
        }
    }
}

/// Apply CLI/env overrides to config.
///
/// TASK-INTEG-019: Modifies config in-place with CLI and env overrides.
/// TASK-42: Added sse_port override.
/// Called AFTER config is loaded but BEFORE validation.
fn apply_overrides(config: &mut Config, cli: &CliArgs) {
    // Override TCP port from CLI
    if let Some(port) = cli.port {
        info!("CLI override: tcp_port = {}", port);
        config.mcp.tcp_port = port;
    } else if let Ok(port_str) = env::var("CONTEXT_GRAPH_TCP_PORT") {
        if let Ok(port) = port_str.parse::<u16>() {
            info!("ENV override: tcp_port = {}", port);
            config.mcp.tcp_port = port;
        }
    }

    // TASK-42: Override SSE port from CLI
    if let Some(port) = cli.sse_port {
        info!("CLI override: sse_port = {}", port);
        config.mcp.sse_port = port;
    } else if let Ok(port_str) = env::var("CONTEXT_GRAPH_SSE_PORT") {
        if let Ok(port) = port_str.parse::<u16>() {
            info!("ENV override: sse_port = {}", port);
            config.mcp.sse_port = port;
        }
    }

    // Override bind address from CLI
    if let Some(ref bind) = cli.bind_address {
        info!("CLI override: bind_address = {}", bind);
        config.mcp.bind_address = bind.clone();
    } else if let Ok(bind) = env::var("CONTEXT_GRAPH_BIND_ADDRESS") {
        info!("ENV override: bind_address = {}", bind);
        config.mcp.bind_address = bind;
    }

    // Override transport from CLI
    if let Some(ref transport) = cli.transport {
        info!("CLI override: transport = {}", transport);
        config.mcp.transport = transport.clone();
    } else if let Ok(transport) = env::var("CONTEXT_GRAPH_TRANSPORT") {
        info!("ENV override: transport = {}", transport);
        config.mcp.transport = transport;
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // CRITICAL: MCP servers must be silent - set this BEFORE any config loading
    // This ensures no banners/warnings corrupt the JSON-RPC stdio protocol
    // Especially important for WSL environments where env vars may not pass correctly
    env::set_var("CONTEXT_GRAPH_MCP_QUIET", "1");

    // Parse CLI arguments first (before logging init so --help works cleanly)
    let cli = CliArgs::parse();

    if cli.help {
        print_help();
        return Ok(());
    }

    // Initialize logging - CRITICAL: Must write to stderr, not stdout!
    // MCP protocol requires stdout to be exclusively for JSON-RPC messages
    // Default to error-only to keep stderr clean for MCP clients
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("error"));

    fmt()
        .with_writer(io::stderr) // CRITICAL: stderr only!
        .with_env_filter(filter)
        .with_target(false) // Cleaner output for MCP
        .init();

    info!("Context Graph MCP Server starting...");

    // Load configuration
    let mut config = if let Some(ref path) = cli.config_path {
        info!("Loading configuration from: {:?}", path);
        Config::from_file(path)? // validate() is called inside from_file()
    } else {
        info!("Using default configuration");
        Config::default()
    };

    // Apply CLI/env overrides BEFORE validation
    apply_overrides(&mut config, &cli);

    // CRITICAL: Validate config AFTER overrides applied
    // This catches invalid CLI/env values early with FAIL FAST
    config.validate()?;

    info!("Configuration loaded: phase={:?}", config.phase);

    // Log stub usage for observability
    if config.uses_stubs() {
        info!(
            "Stub backends in use: embedding={}, storage={}, index={}, utl={}",
            config.embedding.model == "stub",
            config.storage.backend == "memory",
            config.index.backend == "memory",
            config.utl.mode == "stub"
        );
    }

    // Determine transport mode (CLI > ENV > config)
    let transport_mode = determine_transport_mode(&cli, &config)?;

    // Create server
    let server = server::McpServer::new(config).await?;

    // Run with selected transport
    match transport_mode {
        TransportMode::Stdio => {
            info!("MCP Server initialized, listening on stdio");
            server.run().await?;
        }
        TransportMode::Tcp => {
            info!("MCP Server initialized, starting TCP transport");
            server.run_tcp().await?;
        }
        TransportMode::Sse => {
            info!("MCP Server initialized, starting SSE transport");
            server.run_sse().await?;
        }
    }

    info!("MCP Server shutdown complete");
    Ok(())
}
