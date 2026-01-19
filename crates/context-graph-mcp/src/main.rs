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
/// TASK-EMB-WARMUP: Added warm_first flag for blocking model warmup at startup.
/// TASK-DAEMON: Added daemon mode for shared MCP server across multiple terminals.
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
    /// Block startup until embedding models are loaded into VRAM (--warm-first)
    /// Default: true per constitution ARCH-08 (CUDA GPU required for production)
    warm_first: bool,
    /// Skip model warmup entirely (--no-warm)
    /// WARNING: Embedding operations will fail until models load in background
    no_warm: bool,
    /// Use daemon mode: connect to existing TCP daemon if running, or start one (--daemon)
    /// This allows multiple Claude Code terminals to share one MCP server with models
    /// loaded only once into VRAM.
    daemon: bool,
    /// Daemon TCP port (--daemon-port, default: 3199)
    daemon_port: u16,
}

impl CliArgs {
    /// Parse CLI arguments.
    ///
    /// TASK-INTEG-019: Manual parsing without clap to keep binary small.
    /// Supports: --config, --transport, --port, --bind, --help, -h, --warm-first, --no-warm, --daemon, --daemon-port
    fn parse() -> Self {
        let args: Vec<String> = env::args().collect();
        let mut cli = CliArgs {
            config_path: None,
            transport: None,
            port: None,
            sse_port: None,
            bind_address: None,
            help: false,
            warm_first: true, // Default: block until models are warm
            no_warm: false,
            daemon: false,
            daemon_port: 3199, // Default daemon port (different from TCP port 3100)
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
                "--warm-first" => {
                    // TASK-EMB-WARMUP: Block startup until models are warm
                    cli.warm_first = true;
                }
                "--no-warm" => {
                    // TASK-EMB-WARMUP: Skip blocking warmup (use background loading)
                    cli.no_warm = true;
                }
                "--daemon" => {
                    // TASK-DAEMON: Use daemon mode for shared server
                    cli.daemon = true;
                }
                "--daemon-port" => {
                    // TASK-DAEMON: Custom daemon port
                    i += 1;
                    if i < args.len() {
                        if let Ok(port) = args[i].parse::<u16>() {
                            cli.daemon_port = port;
                        }
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
    --warm-first         Block startup until embedding models are loaded into VRAM (default)
    --no-warm            Skip blocking warmup (embeddings fail until background load completes)
    --daemon             Share one server across multiple terminals (RECOMMENDED)
    --daemon-port <PORT> Daemon TCP port (default: 3199)
    --help, -h           Show this help message

ENVIRONMENT VARIABLES:
    CONTEXT_GRAPH_TRANSPORT     Transport mode (stdio|tcp|sse)
    CONTEXT_GRAPH_TCP_PORT      TCP port number
    CONTEXT_GRAPH_SSE_PORT      SSE port number (TASK-42)
    CONTEXT_GRAPH_BIND_ADDRESS  TCP/SSE bind address
    CONTEXT_GRAPH_WARM_FIRST    Set to "0" to disable blocking warmup (default: "1")
    CONTEXT_GRAPH_DAEMON        Set to "1" to enable daemon mode (default: "0")
    CONTEXT_GRAPH_DAEMON_PORT   Daemon port number (default: 3199)
    RUST_LOG                    Log level (error, warn, info, debug, trace)

PRIORITY:
    CLI arguments > Environment variables > Config file > Defaults

DAEMON MODE (--daemon):
    Allows multiple Claude Code terminals to share ONE MCP server, with embedding
    models loaded only ONCE into VRAM. This prevents GPU OOM when using multiple
    terminals.

    How it works:
    1. First terminal: Starts daemon (loads 32GB models into VRAM once)
    2. Other terminals: Connect to existing daemon via stdio-to-TCP proxy
    3. All terminals share the same warm models

    Usage:
      context-graph-mcp --daemon           # Enable daemon mode (recommended)
      context-graph-mcp --daemon-port 4000 # Use custom daemon port

GPU WARMUP:
    By default, the server blocks startup until all 13 embedding models are loaded
    into VRAM. This ensures embedding operations are available immediately.

    RTX 5090 (32GB VRAM) warmup takes approximately 20-30 seconds.
    Use --no-warm only if you accept embedding failures during the warmup period.

EXAMPLES:
    # RECOMMENDED: Run with daemon mode (share server across terminals)
    context-graph-mcp --daemon

    # Run with stdio transport (default, blocks until models warm)
    context-graph-mcp

    # Run with fast startup (embeddings fail until background load completes)
    context-graph-mcp --no-warm

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

/// Determine warm_first mode from CLI and environment.
///
/// TASK-EMB-WARMUP: Controls whether MCP server blocks startup until embedding
/// models are loaded into VRAM.
///
/// Priority: CLI > ENV > Default (true)
///
/// - `--no-warm` disables blocking warmup (fast startup, embeddings fail until ready)
/// - `--warm-first` enables blocking warmup (default behavior)
/// - `CONTEXT_GRAPH_WARM_FIRST=0` disables blocking warmup via environment
///
/// # Returns
///
/// `true` to block startup until models are warm (default)
/// `false` to use background loading (fast startup)
fn determine_warm_first(cli: &CliArgs) -> bool {
    // CLI --no-warm takes highest priority
    if cli.no_warm {
        info!("CLI override: warm_first = false (--no-warm)");
        return false;
    }

    // CLI --warm-first explicitly enabled
    if cli.warm_first {
        // Check if it's the default vs explicitly set
        // (Since warm_first defaults to true, we log only if --warm-first was used)
        // This is a no-op since warm_first defaults to true, but log if env var is set
    }

    // Environment variable is second priority
    if let Ok(warm_env) = env::var("CONTEXT_GRAPH_WARM_FIRST") {
        let warm_first = warm_env != "0" && warm_env.to_lowercase() != "false";
        if !warm_first {
            info!(
                "ENV override: warm_first = false (CONTEXT_GRAPH_WARM_FIRST={})",
                warm_env
            );
        }
        return warm_first;
    }

    // Default: true (block until models are warm)
    // This is the correct behavior per constitution ARCH-08 (CUDA GPU required)
    true
}

/// Determine daemon mode from CLI and environment.
///
/// TASK-DAEMON: Controls whether MCP server uses daemon mode for shared server.
///
/// Priority: CLI > ENV > Default (false)
fn determine_daemon_mode(cli: &CliArgs) -> bool {
    // CLI --daemon takes highest priority
    if cli.daemon {
        info!("CLI: daemon mode enabled (--daemon)");
        return true;
    }

    // Environment variable is second priority
    if let Ok(daemon_env) = env::var("CONTEXT_GRAPH_DAEMON") {
        let daemon = daemon_env == "1" || daemon_env.to_lowercase() == "true";
        if daemon {
            info!(
                "ENV: daemon mode enabled (CONTEXT_GRAPH_DAEMON={})",
                daemon_env
            );
        }
        return daemon;
    }

    // Default: false (standalone mode)
    false
}

/// Determine daemon port from CLI and environment.
fn determine_daemon_port(cli: &CliArgs) -> u16 {
    // CLI --daemon-port takes highest priority
    if cli.daemon_port != 3199 {
        return cli.daemon_port;
    }

    // Environment variable is second priority
    if let Ok(port_env) = env::var("CONTEXT_GRAPH_DAEMON_PORT") {
        if let Ok(port) = port_env.parse::<u16>() {
            return port;
        }
    }

    // Default: 3199
    3199
}

/// Check if daemon is already running on the specified port.
///
/// Returns true if a connection can be established, false otherwise.
async fn is_daemon_running(port: u16) -> bool {
    use tokio::net::TcpStream;
    use tokio::time::{timeout, Duration};

    let addr = format!("127.0.0.1:{}", port);
    match timeout(Duration::from_millis(500), TcpStream::connect(&addr)).await {
        Ok(Ok(_stream)) => {
            info!("Daemon already running on port {}", port);
            true
        }
        _ => {
            info!("No daemon found on port {}", port);
            false
        }
    }
}

/// Run as stdio-to-TCP proxy, connecting to the daemon.
///
/// Reads JSON-RPC messages from stdin, forwards to daemon via TCP,
/// and writes responses to stdout.
async fn run_stdio_to_tcp_proxy(daemon_port: u16) -> Result<()> {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpStream;

    let addr = format!("127.0.0.1:{}", daemon_port);
    info!("Connecting to daemon at {}...", addr);

    let stream = TcpStream::connect(&addr).await.map_err(|e| {
        error!("Failed to connect to daemon at {}: {}", addr, e);
        anyhow::anyhow!("Failed to connect to daemon: {}", e)
    })?;

    info!("Connected to daemon, starting stdio proxy");

    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    // Spawn task to read from daemon and write to stdout
    let stdout_task = tokio::spawn(async move {
        let mut stdout = tokio::io::stdout();
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => {
                    // EOF from daemon
                    info!("Daemon closed connection");
                    break;
                }
                Ok(_) => {
                    if let Err(e) = stdout.write_all(line.as_bytes()).await {
                        error!("Failed to write to stdout: {}", e);
                        break;
                    }
                    if let Err(e) = stdout.flush().await {
                        error!("Failed to flush stdout: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    error!("Failed to read from daemon: {}", e);
                    break;
                }
            }
        }
    });

    // Read from stdin and write to daemon
    let stdin = tokio::io::stdin();
    let mut stdin = BufReader::new(stdin);
    let mut line = String::new();

    loop {
        line.clear();
        match stdin.read_line(&mut line).await {
            Ok(0) => {
                // EOF from stdin
                info!("Stdin closed");
                break;
            }
            Ok(_) => {
                if let Err(e) = writer.write_all(line.as_bytes()).await {
                    error!("Failed to write to daemon: {}", e);
                    break;
                }
                if let Err(e) = writer.flush().await {
                    error!("Failed to flush to daemon: {}", e);
                    break;
                }
            }
            Err(e) => {
                error!("Failed to read from stdin: {}", e);
                break;
            }
        }
    }

    // Wait for stdout task to complete
    let _ = stdout_task.await;
    info!("Stdio proxy shutdown");
    Ok(())
}

/// Start daemon server in background and return when ready.
///
/// Spawns a task that runs the TCP server and waits until it's accepting connections.
async fn start_daemon_server(config: Config, warm_first: bool, daemon_port: u16) -> Result<()> {
    use tokio::time::{sleep, Duration};

    info!("Starting daemon server on port {}...", daemon_port);

    // Create a modified config for the daemon
    let mut daemon_config = config;
    daemon_config.mcp.tcp_port = daemon_port;
    daemon_config.mcp.transport = "tcp".to_string();

    // Create the server
    let server = server::McpServer::new(daemon_config, warm_first).await?;

    // Spawn the daemon server
    tokio::spawn(async move {
        if let Err(e) = server.run_tcp().await {
            error!("Daemon server error: {}", e);
        }
    });

    // Wait for daemon to be ready (accept connections)
    for _ in 0..50 {
        // 5 seconds max
        sleep(Duration::from_millis(100)).await;
        if is_daemon_running(daemon_port).await {
            info!("Daemon server ready on port {}", daemon_port);
            return Ok(());
        }
    }

    Err(anyhow::anyhow!(
        "Daemon server failed to start within 5 seconds"
    ))
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

    // Determine warm_first mode (CLI > ENV > default)
    // TASK-EMB-WARMUP: Block startup until models are warm by default
    let warm_first = determine_warm_first(&cli);

    // TASK-DAEMON: Check if daemon mode is enabled
    let daemon_mode = determine_daemon_mode(&cli);
    let daemon_port = determine_daemon_port(&cli);

    if daemon_mode {
        // ==================================================================
        // DAEMON MODE: Share one MCP server across multiple Claude terminals
        // ==================================================================
        info!("Daemon mode enabled (port {})", daemon_port);

        // Check if daemon is already running
        if is_daemon_running(daemon_port).await {
            // Daemon exists - just proxy to it
            info!("Connecting to existing daemon on port {}...", daemon_port);
            run_stdio_to_tcp_proxy(daemon_port).await?;
        } else {
            // No daemon - start one and then proxy to it
            info!("No daemon found, starting new daemon server...");
            start_daemon_server(config, warm_first, daemon_port).await?;
            info!("Daemon started, connecting as proxy...");
            run_stdio_to_tcp_proxy(daemon_port).await?;
        }
    } else {
        // ==================================================================
        // STANDALONE MODE: Each terminal has its own MCP server (original behavior)
        // ==================================================================

        // Create server with warmup configuration
        let server = server::McpServer::new(config, warm_first).await?;

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
    }

    info!("MCP Server shutdown complete");
    Ok(())
}
