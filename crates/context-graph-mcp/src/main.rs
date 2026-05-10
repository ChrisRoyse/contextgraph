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
mod monitoring;
mod protocol;
mod server;
mod tools;
mod weights;

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use anyhow::Result;
use tracing::{debug, error, info, warn};
use tracing_subscriber::{fmt, EnvFilter};

use context_graph_core::config::Config;
use server::TransportMode;

// ============================================================================
// CLI Argument Parsing
// ============================================================================

/// Parsed CLI arguments for the MCP server.
///
/// TASK-INTEG-019: Simple argument parsing without external dependencies.
/// TASK-EMB-WARMUP: Added warm_first flag for blocking model warmup at startup.
/// TASK-DAEMON: Added daemon mode for shared MCP server across multiple terminals.
#[derive(Debug)]
struct CliArgs {
    /// Path to configuration file
    config_path: Option<PathBuf>,
    /// Transport mode override (--transport)
    transport: Option<String>,
    /// MCP server mode (--mode): default or reality-loop
    mode: String,
    /// TCP port override (--port)
    port: Option<u16>,
    /// TCP bind address override (--bind)
    bind_address: Option<String>,
    /// Show help
    help: bool,
    /// Show version
    version: bool,
    // L3 FIX: warm_first field removed — determine_warm_first() uses only no_warm + env var.
    // Default is true (block until warm). --warm-first flag just clears no_warm.
    /// Skip model warmup entirely (--no-warm)
    /// WARNING: Embedding operations will fail until models load in background
    no_warm: bool,
    /// Use daemon mode: connect to existing TCP daemon if running, or start one (--daemon)
    /// This allows multiple Claude Code terminals to share one MCP server with models
    /// loaded only once into VRAM.
    daemon: bool,
    /// Daemon TCP port (--daemon-port, default: 3100)
    daemon_port: u16,
    /// Whether --daemon-port was explicitly passed on CLI
    explicit_daemon_port: bool,
    /// Internal flag: this process IS the headless daemon (--daemon-server-only).
    /// Set automatically by spawn_daemon_process(). Not exposed in --help.
    daemon_server_only: bool,
}

impl CliArgs {
    /// Parse CLI arguments.
    ///
    /// TASK-INTEG-019: Manual parsing without clap to keep binary small.
    /// Supports: --config, --transport, --port, --bind, --help, -h, --version, -V, --warm-first, --no-warm, --daemon, --daemon-port
    fn parse() -> Result<Self> {
        Self::parse_from(env::args())
    }

    fn parse_from<I, S>(args: I) -> Result<Self>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let args: Vec<String> = args.into_iter().map(Into::into).collect();
        let mut cli = CliArgs {
            config_path: None,
            transport: None,
            mode: "default".to_string(),
            port: None,
            bind_address: None,
            help: false,
            version: false,
            no_warm: false,
            daemon: false,
            daemon_port: 3100, // Default daemon port (aligned with .mcp.json)
            explicit_daemon_port: false,
            daemon_server_only: false,
        };

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--help" | "-h" => {
                    cli.help = true;
                }
                "--version" | "-V" => {
                    cli.version = true;
                }
                "--config" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(anyhow::anyhow!("Missing value for --config"));
                    }
                    cli.config_path = Some(PathBuf::from(&args[i]));
                }
                "--transport" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(anyhow::anyhow!("Missing value for --transport"));
                    }
                    cli.transport = Some(args[i].clone());
                }
                "--mode" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(anyhow::anyhow!("Missing value for --mode"));
                    }
                    match args[i].as_str() {
                        "default" | "reality-loop" => cli.mode = args[i].clone(),
                        other => {
                            return Err(anyhow::anyhow!(
                                "Invalid --mode value '{}': must be default or reality-loop",
                                other
                            ));
                        }
                    }
                }
                "--port" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(anyhow::anyhow!("Missing value for --port"));
                    }
                    cli.port = Some(parse_port_number("--port", &args[i])?);
                }
                "--bind" => {
                    i += 1;
                    if i >= args.len() {
                        return Err(anyhow::anyhow!("Missing value for --bind"));
                    }
                    cli.bind_address = Some(args[i].clone());
                }
                "--warm-first" => {
                    // Explicit opt-in to default behavior. Useful to override --no-warm when both are passed.
                    cli.no_warm = false;
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
                    if i >= args.len() {
                        return Err(anyhow::anyhow!("Missing value for --daemon-port"));
                    }
                    cli.daemon_port = parse_port_number("--daemon-port", &args[i])?;
                    cli.explicit_daemon_port = true;
                }
                "--daemon-server-only" => {
                    // Internal flag: this process IS the headless daemon.
                    // Spawned by spawn_daemon_process() — not for direct user use.
                    cli.daemon_server_only = true;
                    cli.daemon = true;
                }
                arg => {
                    return Err(anyhow::anyhow!(
                        "Unknown argument '{}'. Use --help for usage.",
                        arg
                    ));
                }
            }
            i += 1;
        }

        Ok(cli)
    }
}

fn parse_port_number(source: &str, value: &str) -> Result<u16> {
    let port = value.parse::<u16>().map_err(|e| {
        anyhow::anyhow!(
            "Invalid {source} value '{value}': must be a valid port number (1-65535): {e}"
        )
    })?;
    if port == 0 {
        return Err(anyhow::anyhow!(
            "Invalid {source} value '{value}': must be a valid port number (1-65535)"
        ));
    }
    Ok(port)
}

/// Print help message and exit.
fn print_help() {
    eprintln!(
        r#"Context Graph MCP Server

USAGE:
    context-graph-mcp [OPTIONS]

OPTIONS:
    --config <PATH>      Path to configuration file
    --transport <MODE>   Transport mode: stdio (default) or tcp
    --mode <MODE>        MCP server mode: default or reality-loop
    --port <PORT>        TCP port (only used with --transport tcp)
    --bind <ADDRESS>     TCP bind address (default: 127.0.0.1)
    --warm-first         Block startup until embedding models are loaded into VRAM (default)
    --no-warm            Skip blocking warmup (embeddings fail until background load completes)
    --daemon             Share one server across multiple terminals (RECOMMENDED)
    --daemon-port <PORT> Daemon TCP port (default: 3100)
    --version, -V        Show version and exit
    --help, -h           Show this help message

ENVIRONMENT VARIABLES:
    CONTEXT_GRAPH_TRANSPORT     Transport mode (stdio|tcp). SSE is not supported.
    CONTEXT_GRAPH_TCP_PORT      TCP port number
    CONTEXT_GRAPH_BIND_ADDRESS  TCP bind address
    CONTEXT_GRAPH_WARM_FIRST    Set to "0" to disable blocking warmup (default: "1")
    CONTEXT_GRAPH_DAEMON        Set to "1" to enable daemon mode (default: "0")
    CONTEXT_GRAPH_DAEMON_PORT   Daemon port number (default: 3100)
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
    By default, the server blocks startup until all 14 embedding models are loaded
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
fn determine_transport_mode(cli: &CliArgs, config: &Config) -> Result<TransportMode> {
    // CLI takes highest priority
    if let Some(ref transport) = cli.transport {
        let transport_lower = transport.to_lowercase();
        return match transport_lower.as_str() {
            "stdio" => Ok(TransportMode::Stdio),
            "tcp" => Ok(TransportMode::Tcp),
            "sse" => {
                error!("FATAL: SSE transport is not supported. Use 'stdio' or 'tcp'.");
                Err(anyhow::anyhow!(
                    "SSE transport is not supported. Use 'stdio' or 'tcp'."
                ))
            }
            _ => {
                error!(
                    "FATAL: Invalid transport '{}' from CLI. Must be 'stdio' or 'tcp'.",
                    transport
                );
                Err(anyhow::anyhow!(
                    "Invalid transport '{}'. Must be 'stdio' or 'tcp'.",
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
            "sse" => {
                error!("FATAL: SSE transport is not supported. Use 'stdio' or 'tcp'.");
                Err(anyhow::anyhow!(
                    "SSE transport is not supported. Use 'stdio' or 'tcp'."
                ))
            }
            _ => {
                error!(
                    "FATAL: Invalid CONTEXT_GRAPH_TRANSPORT='{}'. Must be 'stdio' or 'tcp'.",
                    transport
                );
                Err(anyhow::anyhow!(
                    "Invalid CONTEXT_GRAPH_TRANSPORT='{}'. Must be 'stdio' or 'tcp'.",
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
        "sse" => {
            error!("FATAL: SSE transport is not supported. Use 'stdio' or 'tcp'.");
            Err(anyhow::anyhow!(
                "SSE transport is not supported. Use 'stdio' or 'tcp'."
            ))
        }
        _ => {
            // This should not happen if Config::validate() passed, but FAIL FAST anyway
            error!(
                "FATAL: Invalid transport '{}' in config. Must be 'stdio' or 'tcp'.",
                config.mcp.transport
            );
            Err(anyhow::anyhow!(
                "Invalid transport '{}' in config. Must be 'stdio' or 'tcp'.",
                config.mcp.transport
            ))
        }
    }
}

/// Apply CLI/env overrides to config.
///
/// TASK-INTEG-019: Modifies config in-place with CLI and env overrides.
/// Called AFTER config is loaded but BEFORE validation.
fn apply_overrides(config: &mut Config, cli: &CliArgs) -> Result<()> {
    // Override TCP port from CLI
    if let Some(port) = cli.port {
        info!("CLI override: tcp_port = {}", port);
        config.mcp.tcp_port = port;
    } else if let Ok(port_str) = env::var("CONTEXT_GRAPH_TCP_PORT") {
        let port = parse_port_number("CONTEXT_GRAPH_TCP_PORT", &port_str)?;
        info!("ENV override: tcp_port = {}", port);
        config.mcp.tcp_port = port;
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

    // Override request timeout for long-running real embedder operations.
    // Supports both the direct MCP-server env name and the nested config-style name.
    if let Some((name, value)) = env::var("CONTEXT_GRAPH_MCP_REQUEST_TIMEOUT")
        .ok()
        .map(|v| ("CONTEXT_GRAPH_MCP_REQUEST_TIMEOUT", v))
        .or_else(|| {
            env::var("CONTEXT_GRAPH__MCP__REQUEST_TIMEOUT")
                .ok()
                .map(|v| ("CONTEXT_GRAPH__MCP__REQUEST_TIMEOUT", v))
        })
    {
        let timeout = value.parse::<u64>().map_err(|e| {
            anyhow::anyhow!(
                "Invalid {}='{}': request timeout must be a positive integer number of seconds ({})",
                name,
                value,
                e
            )
        })?;
        info!("ENV override: mcp.request_timeout = {}", timeout);
        config.mcp.request_timeout = timeout;
    }

    // CRITICAL: Override storage path and enable RocksDB backend
    // When CONTEXT_GRAPH_STORAGE_PATH is set, use RocksDB instead of in-memory stub
    if let Ok(storage_path) = env::var("CONTEXT_GRAPH_STORAGE_PATH") {
        info!("ENV override: storage.path = {}", storage_path);
        config.storage.path = storage_path;
        // CRITICAL: Switch from "memory" stub to "rocksdb" real backend
        if config.storage.backend == "memory" {
            info!("ENV override: storage.backend = rocksdb (was memory stub)");
            config.storage.backend = "rocksdb".to_string();
        }
    }

    // CRITICAL: Override models path and enable real embeddings
    // When CONTEXT_GRAPH_MODELS_PATH is set, use real models instead of stub
    if let Ok(models_path) = env::var("CONTEXT_GRAPH_MODELS_PATH") {
        info!("ENV override: embedding.model_path = {}", models_path);
        // Switch from "stub" to real model
        if config.embedding.model == "stub" {
            info!("ENV override: embedding.model = e5-large-v2 (was stub)");
            config.embedding.model = "e5-large-v2".to_string();
        }
        // Note: The actual models_path is used by ProductionMultiArrayProvider
        // which reads from this env var directly in server.rs
    }

    // CRITICAL: Override index backend when storage is real
    // If storage is RocksDB, index should be HNSW not memory
    if config.storage.backend == "rocksdb" && config.index.backend == "memory" {
        info!("ENV override: index.backend = hnsw (storage is rocksdb)");
        config.index.backend = "hnsw".to_string();
    }

    // CRITICAL: Override UTL mode when using real backends
    if config.storage.backend == "rocksdb" && config.utl.mode == "stub" {
        info!("ENV override: utl.mode = production (storage is rocksdb)");
        config.utl.mode = "production".to_string();
    }

    // Override file watcher enabled from environment
    if let Ok(watcher_env) = env::var("CONTEXT_GRAPH_WATCHER_ENABLED") {
        let enabled = watcher_env == "1" || watcher_env.to_lowercase() == "true";
        info!("ENV override: watcher.enabled = {}", enabled);
        config.watcher.enabled = enabled;
    }

    Ok(())
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
fn determine_daemon_port(cli: &CliArgs) -> Result<u16> {
    // CLI --daemon-port takes highest priority
    if cli.explicit_daemon_port {
        return Ok(cli.daemon_port);
    }

    // Environment variable is second priority
    if let Ok(port_env) = env::var("CONTEXT_GRAPH_DAEMON_PORT") {
        return parse_port_number("CONTEXT_GRAPH_DAEMON_PORT", &port_env);
    }

    // Default: 3100 (aligned with .mcp.json convention)
    Ok(3100)
}

// ============================================================================
// PID File Guard — prevents multiple processes from opening the same RocksDB
// ============================================================================

/// PID file guard that prevents multiple MCP server instances from opening
/// the same RocksDB database concurrently.
///
/// Root cause of corruption: if two processes open the same DB (e.g., one
/// standalone stdio + one TCP on a different port), a kill/crash during
/// compaction leaves the MANIFEST referencing SST files that the other
/// process's compaction already deleted → corruption.
///
/// The guard writes `<PID>` to `<db_path>/mcp.pid` and holds an exclusive
/// `flock()` on it for the lifetime of this struct. Drop releases the lock
/// and removes the file.
struct PidFileGuard {
    path: PathBuf,
    #[cfg(unix)]
    _file: std::fs::File, // kept open to hold the flock
}

impl PidFileGuard {
    /// Acquire the PID file lock for the given database path.
    ///
    /// Returns `Ok(guard)` if we are the sole owner.
    /// Returns `Err` with a descriptive message if another live process holds it.
    fn acquire(db_path: &Path) -> Result<Self> {
        fs::create_dir_all(db_path)?;

        let pid_path = db_path.join("mcp.pid");

        // Open or create the PID file
        let file = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&pid_path)
            .map_err(|e| anyhow::anyhow!("Cannot open PID file '{}': {}", pid_path.display(), e))?;

        #[cfg(unix)]
        {
            use std::io::{Read, Seek, Write};
            use std::os::unix::io::AsRawFd;

            let fd = file.as_raw_fd();
            let result = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };

            if result != 0 {
                let errno = io::Error::last_os_error();
                if errno.raw_os_error() == Some(libc::EWOULDBLOCK) {
                    // Another process holds the lock — read its PID for diagnostics
                    let mut contents = String::new();
                    let mut f = &file;
                    if let Err(e) = f.read_to_string(&mut contents) {
                        error!(
                            error = %e,
                            pid_path = %pid_path.display(),
                            "E_PID_READ_FAIL: Failed to read PID file while lock is held. \
                             Treating lock as HELD (not stale) to prevent concurrent DB access."
                        );
                        return Err(anyhow::anyhow!(
                            "Database '{}' is locked by another process (PID file unreadable: {}). \
                             Cannot safely determine if lock holder is alive.",
                            db_path.display(), e
                        ));
                    }
                    let holder_pid_str = contents.trim().to_string();

                    // Check if the holding process is actually alive
                    if let Ok(pid) = holder_pid_str.parse::<i32>() {
                        // kill(pid, 0) sends no signal — just checks existence
                        let alive = unsafe { libc::kill(pid, 0) };

                        let is_stale = if alive != 0 {
                            // Process is dead — stale lock from a crash/kill.
                            warn!(
                                "Stale PID file: process {} is dead, attempting to reclaim lock on '{}'",
                                pid, db_path.display()
                            );
                            true
                        } else {
                            // Process is alive — check for zombie state
                            let status_path = format!("/proc/{}/status", pid);
                            let is_zombie = std::fs::read_to_string(&status_path)
                                .map(|s| s.contains("State:\tZ") || s.contains("State:\tX"))
                                .unwrap_or(false);
                            if is_zombie {
                                warn!(
                                    "PID {} is zombie/dead — reclaiming stale lock on '{}'",
                                    pid,
                                    db_path.display()
                                );
                            }
                            is_zombie
                        };

                        if is_stale {
                            drop(file);
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            let file = fs::OpenOptions::new()
                                .create(true)
                                .truncate(true)
                                .read(true)
                                .write(true)
                                .open(&pid_path)
                                .map_err(|e| {
                                    anyhow::anyhow!(
                                        "Cannot reopen PID file '{}': {}",
                                        pid_path.display(),
                                        e
                                    )
                                })?;
                            let fd = file.as_raw_fd();
                            let result = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
                            if result == 0 {
                                // Audit-7 MCP7-L2 FIX: Log warnings on PID write failures
                                let mut f = &file;
                                if let Err(e) = f.seek(io::SeekFrom::Start(0)) {
                                    warn!("PID file seek failed (reclaim): {}", e);
                                }
                                if let Err(e) = f.set_len(0) {
                                    warn!("PID file truncate failed (reclaim): {}", e);
                                }
                                if let Err(e) = write!(f, "{}", std::process::id()) {
                                    warn!("PID file write failed (reclaim): {}", e);
                                }
                                if let Err(e) = f.flush() {
                                    warn!("PID file flush failed (reclaim): {}", e);
                                }
                                info!(
                                    "Reclaimed stale PID lock (was process {}): new pid={}",
                                    pid,
                                    std::process::id()
                                );
                                return Ok(PidFileGuard {
                                    path: pid_path,
                                    _file: file,
                                });
                            }
                            warn!("Failed to reclaim lock — another process acquired it first");
                        }
                    }

                    return Err(anyhow::anyhow!(
                        "Another context-graph-mcp process (PID {}) is already using database at '{}'. \
                         Kill the existing process first, or use --daemon mode to share a single server.\n\
                         To kill: kill {} && sleep 1",
                        holder_pid_str, db_path.display(), holder_pid_str
                    ));
                }
                return Err(anyhow::anyhow!(
                    "flock() on PID file '{}' failed: {}",
                    pid_path.display(),
                    errno
                ));
            }

            // We hold the lock -- write our PID
            // Audit-7 MCP7-L2 FIX: Log warnings on PID write failures instead of
            // silently discarding with `let _ =`. If writing fails, other processes
            // reading the PID file get stale/partial data, breaking stale detection.
            let mut f = &file;
            if let Err(e) = f.seek(io::SeekFrom::Start(0)) {
                warn!(
                    "PID file seek failed: {} -- stale detection may be unreliable",
                    e
                );
            }
            if let Err(e) = f.set_len(0) {
                warn!(
                    "PID file truncate failed: {} -- stale detection may be unreliable",
                    e
                );
            }
            if let Err(e) = write!(f, "{}", std::process::id()) {
                warn!(
                    "PID file write failed: {} -- stale detection may be unreliable",
                    e
                );
            }
            if let Err(e) = f.flush() {
                warn!(
                    "PID file flush failed: {} -- stale detection may be unreliable",
                    e
                );
            }

            info!(
                "PID file guard acquired: pid={}, path='{}'",
                std::process::id(),
                pid_path.display()
            );

            Ok(PidFileGuard {
                path: pid_path,
                _file: file,
            })
        }

        #[cfg(not(unix))]
        {
            // CLI-M4 FIX: On non-Unix, PidFileGuard writes PID but acquires NO flock.
            // Without a file lock, there is no protection against multiple daemon instances.
            // Concurrent instances sharing the same RocksDB directory WILL corrupt data.
            use std::io::Write;
            // Audit-7 MCP7-L2 FIX: Log warnings on PID write failures (non-Unix path)
            let mut f = &file;
            if let Err(e) = f.set_len(0) {
                warn!("PID file truncate failed (non-unix): {}", e);
            }
            if let Err(e) = write!(f, "{}", std::process::id()) {
                warn!("PID file write failed (non-unix): {}", e);
            }
            if let Err(e) = f.flush() {
                warn!("PID file flush failed (non-unix): {}", e);
            }

            warn!(
                "E_PID_NO_LOCK: PidFileGuard on non-Unix platform provides no lock protection. \
                 Multiple instances may corrupt RocksDB. Ensure only one daemon runs at a time."
            );

            Ok(PidFileGuard { path: pid_path })
        }
    }
}

impl Drop for PidFileGuard {
    fn drop(&mut self) {
        // Release flock (implicit on file close) and remove PID file
        if let Err(e) = fs::remove_file(&self.path) {
            warn!(
                error = %e,
                pid_path = %self.path.display(),
                "E_PID_CLEANUP: Failed to remove PID file on guard drop. \
                 Stale PID file may confuse next startup."
            );
        } else {
            debug!("PID file guard released: '{}'", self.path.display());
        }
    }
}

/// Check if a healthy, responsive daemon is running on the specified port.
///
/// Performs a full JSON-RPC round-trip (tools/list) to verify the daemon
/// is not just listening but actually processing requests. This catches:
/// - Deadlocked servers (accept loop hung)
/// - Half-initialized servers (TCP bound but handlers not wired)
/// - Different processes (another service on the same port)
///
/// Returns true only if the daemon responds with a valid JSON-RPC response
/// within 3 seconds total (500ms connect + 2.5s request/response).
async fn is_daemon_healthy(port: u16) -> bool {
    use tokio::io::{AsyncWriteExt, BufReader};
    use tokio::net::TcpStream;
    use tokio::time::{timeout, Duration};

    let addr = format!("127.0.0.1:{}", port);

    // Phase 1: TCP connect (500ms timeout)
    let stream = match timeout(Duration::from_millis(500), TcpStream::connect(&addr)).await {
        Ok(Ok(s)) => s,
        Ok(Err(_)) => {
            debug!("Health check: TCP connect refused on port {}", port);
            return false;
        }
        Err(_) => {
            debug!("Health check: TCP connect timed out on port {}", port);
            return false;
        }
    };

    // Phase 2: Send tools/list, expect JSON-RPC response (2.5s timeout)
    let result = timeout(Duration::from_millis(2500), async {
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);

        // MCP protocol: tools/list is always available, has no side effects,
        // and exercises the full handler dispatch pipeline.
        let probe = r#"{"jsonrpc":"2.0","id":"_health_check","method":"tools/list","params":{}}"#;
        writer.write_all(probe.as_bytes()).await?;
        writer.write_all(b"\n").await?;
        writer.flush().await?;

        let mut response = String::new();
        context_graph_mcp::server::transport::read_line_bounded(
            &mut reader,
            &mut response,
            context_graph_mcp::server::transport::MAX_LINE_BYTES,
        )
        .await?;

        // Verify it's a valid JSON-RPC response (not some other protocol)
        let is_valid = response.contains("\"jsonrpc\"") && response.contains("\"result\"");
        Ok::<bool, std::io::Error>(is_valid)
    })
    .await;

    match result {
        Ok(Ok(true)) => {
            info!("Health check: daemon on port {} is healthy", port);
            true
        }
        Ok(Ok(false)) => {
            warn!(
                "Health check: port {} responded but not valid JSON-RPC",
                port
            );
            false
        }
        Ok(Err(e)) => {
            warn!("Health check: I/O error on port {}: {}", port, e);
            false
        }
        Err(_) => {
            warn!("Health check: daemon on port {} timed out (2.5s)", port);
            false
        }
    }
}

/// Kill a process that is holding a TCP port but not responding to health checks.
///
/// Uses Linux `fuser` to find the PID owning the port. Sends SIGTERM first,
/// waits 200ms, then SIGKILL if still alive. Skips our own PID.
///
/// This is Linux-specific. Context Graph only targets Linux (WSL2 + native).
#[cfg(unix)]
async fn kill_process_on_port(port: u16) -> Result<()> {
    use tokio::time::Duration;

    let output = tokio::process::Command::new("fuser")
        .arg(format!("{}/tcp", port))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .await
        .map_err(|e| {
            anyhow::anyhow!(
                "fuser command failed: {}. Is fuser installed? (apt install psmisc)",
                e
            )
        })?;

    let pids_str = String::from_utf8_lossy(&output.stdout);
    let our_pid = std::process::id() as i32;

    for token in pids_str.split_whitespace() {
        let pid_str: String = token.chars().filter(|c| c.is_ascii_digit()).collect();
        if let Ok(pid) = pid_str.parse::<i32>() {
            if pid == our_pid || pid <= 1 {
                continue;
            }

            warn!("Sending SIGTERM to stuck daemon process PID {}", pid);
            unsafe {
                libc::kill(pid, libc::SIGTERM);
            }
            tokio::time::sleep(Duration::from_millis(200)).await;

            if unsafe { libc::kill(pid, 0) } == 0 {
                warn!("PID {} did not respond to SIGTERM, sending SIGKILL", pid);
                unsafe {
                    libc::kill(pid, libc::SIGKILL);
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }

            info!("Stuck process PID {} terminated", pid);
        }
    }

    Ok(())
}

/// Kill a stale lock holder: a process that holds the PID file flock but is
/// NOT serving on the expected daemon port.
///
/// This handles the critical failure mode where:
/// 1. A previous MCP server process started (standalone or daemon)
/// 2. Its transport layer died (TCP listener crashed, stdio disconnected)
/// 3. The OS process stayed alive (RocksDB background threads, stuck futex)
/// 4. The process holds flock() on mcp.pid forever
/// 5. No new daemon can start → all Claude Code terminals lose MCP access
///
/// Detection: PID file exists → holder alive → NOT serving on daemon port → stale.
/// Recovery: SIGTERM (2s grace) → SIGKILL → wait for flock release.
///
/// ONLY called in daemon mode. In standalone mode, PidFileGuard::acquire()
/// handles lock contention with its existing error path.
#[cfg(unix)]
async fn kill_stale_lock_holder(db_path: &Path, daemon_port: u16) -> bool {
    use tokio::time::{sleep, Duration};

    let pid_path = db_path.join("mcp.pid");

    let pid_str = match fs::read_to_string(&pid_path) {
        Ok(s) => s.trim().to_string(),
        Err(_) => return false, // No PID file — nothing to do
    };

    let pid: i32 = match pid_str.parse() {
        Ok(p) => p,
        Err(_) => {
            warn!(
                "PID file contains non-numeric value '{}' — removing stale file",
                pid_str
            );
            if let Err(e) = fs::remove_file(&pid_path) {
                warn!(
                    "Failed to remove corrupt PID file {}: {}",
                    pid_path.display(),
                    e
                );
            }
            return true;
        }
    };

    let our_pid = std::process::id() as i32;
    if pid <= 1 || pid == our_pid {
        return false;
    }

    // Check if process is alive
    let alive = unsafe { libc::kill(pid, 0) } == 0;
    if !alive {
        // Dead process — flock already released by kernel.
        // PidFileGuard::acquire() will handle cleanup on next attempt.
        info!("PID file holder {} is dead — lock will be reclaimable", pid);
        return false;
    }

    // Process is alive — re-check health to guard against race conditions
    // (daemon might have recovered since the caller's Step 1 check).
    if is_daemon_healthy(daemon_port).await {
        info!(
            "PID {} is now serving on port {} — no kill needed",
            pid, daemon_port
        );
        return false;
    }

    // Process alive, NOT serving on daemon port — stale.
    warn!(
        "STALE LOCK DETECTED: PID {} holds flock on '{}' but is NOT serving on port {}. \
         Sending SIGTERM for graceful shutdown.",
        pid,
        pid_path.display(),
        daemon_port
    );

    // SIGTERM first — gives RocksDB time to flush
    unsafe {
        libc::kill(pid, libc::SIGTERM);
    }

    // Wait up to 2s for graceful shutdown
    for i in 0..20 {
        sleep(Duration::from_millis(100)).await;
        if unsafe { libc::kill(pid, 0) } != 0 {
            info!(
                "Stale PID {} terminated after SIGTERM ({}ms)",
                pid,
                (i + 1) * 100
            );
            sleep(Duration::from_millis(200)).await;
            return true;
        }
    }

    // Still alive after 2s — force kill
    warn!(
        "PID {} did not exit after SIGTERM (2s), sending SIGKILL",
        pid
    );
    unsafe {
        libc::kill(pid, libc::SIGKILL);
    }

    // Wait up to 1s for kernel to reap the process
    for i in 0..10 {
        sleep(Duration::from_millis(100)).await;
        if unsafe { libc::kill(pid, 0) } != 0 {
            info!(
                "Stale PID {} terminated after SIGKILL ({}ms)",
                pid,
                (i + 1) * 100
            );
            sleep(Duration::from_millis(200)).await;
            return true;
        }
        // Check for zombie state — flock IS released even though kill(pid,0)==0
        let status_path = format!("/proc/{}/status", pid);
        let is_zombie = fs::read_to_string(&status_path)
            .map(|s| s.contains("State:\tZ") || s.contains("State:\tX"))
            .unwrap_or(false);
        if is_zombie {
            info!(
                "PID {} is zombie after SIGKILL — flock released, lock reclaimable",
                pid
            );
            return true;
        }
    }

    error!(
        "FATAL: PID {} still alive after SIGKILL (1s) — OS may be stuck. \
         Manual intervention required: kill -9 {}",
        pid, pid
    );
    false
}

/// Kill a stale standalone (stdio) process holding the PID file flock.
///
/// Unlike `kill_stale_lock_holder` (daemon mode), this doesn't check daemon
/// health. Instead, it detects stale standalone processes by checking if their
/// stdio file descriptors are broken (dead sockets from a disconnected Claude
/// Code session) or if the process is stuck with no active transport.
///
/// A standalone MCP server's stdin/stdout should be connected pipes/sockets.
/// If both are dead (socket:[deleted], pipe:[broken]), the process is orphaned
/// and can never receive new requests — it's safe to kill.
#[cfg(unix)]
async fn kill_stale_standalone_holder(db_path: &Path) -> bool {
    use tokio::time::{sleep, Duration};

    let pid_path = db_path.join("mcp.pid");

    let pid_str = match fs::read_to_string(&pid_path) {
        Ok(s) => s.trim().to_string(),
        Err(_) => return false,
    };

    let pid: i32 = match pid_str.parse() {
        Ok(p) => p,
        Err(_) => {
            warn!(
                "PID file contains non-numeric value '{}' — removing",
                pid_str
            );
            if let Err(e) = fs::remove_file(&pid_path) {
                warn!(
                    "Failed to remove corrupt PID file {}: {}",
                    pid_path.display(),
                    e
                );
            }
            return true;
        }
    };

    let our_pid = std::process::id() as i32;
    if pid <= 1 || pid == our_pid {
        return false;
    }

    // Check if process is alive
    if unsafe { libc::kill(pid, 0) } != 0 {
        info!("PID file holder {} is dead — lock will be reclaimable", pid);
        return false;
    }

    // Process is alive — check if its stdio is still connected.
    // A healthy standalone MCP server has stdin (fd/0) connected to a live
    // socket or pipe. An orphaned one has fd/0 pointing to a dead socket
    // or the process is blocked on a futex with no active I/O.
    let fd0_path = format!("/proc/{}/fd/0", pid);
    let fd0_target = fs::read_link(&fd0_path).ok();
    let fd0_str = fd0_target
        .as_ref()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();

    // Check if stdin is a socket — if so, verify it's not in ESTABLISHED state.
    // An orphaned MCP server's socket will be in CLOSE_WAIT or not in ss at all.
    let stdin_is_dead = if fd0_str.starts_with("socket:") || fd0_str.starts_with("pipe:") {
        // If we can read /proc/pid/fdinfo/0 and the process cmdline matches
        // ours, and the socket is not in /proc/net/tcp ESTABLISHED, it's dead.
        // Simpler heuristic: the process was started without --daemon, has
        // a socket stdin, and isn't listening on any port → it's a disconnected
        // stdio server.
        let cmdline = match fs::read_to_string(format!("/proc/{}/cmdline", pid)) {
            Ok(c) => c.replace('\0', " "),
            Err(e) => {
                warn!(
                    pid = pid,
                    error = %e,
                    "Cannot read /proc/{}/cmdline — treating as alive (not stale) to prevent misclassification",
                    pid
                );
                // Cannot determine if standalone or daemon — assume alive, don't kill
                return false;
            }
        };
        let is_standalone = !cmdline.contains("--daemon");
        if is_standalone {
            // Audit-7 MCP7-M2 FIX: Check if THIS process owns any listening socket.
            // Previously used /proc/{pid}/net/tcp which shows ALL sockets in the
            // network namespace, not just those owned by pid. Any system TCP listener
            // (sshd, nginx, etc.) would prevent stale detection from ever triggering.
            // Now we iterate /proc/{pid}/fd/ and check which fds are sockets in LISTEN state.
            let owns_listener = (|| -> bool {
                let fd_dir = format!("/proc/{}/fd", pid);
                let entries = match fs::read_dir(&fd_dir) {
                    Ok(e) => e,
                    Err(_) => return false,
                };
                // Read /proc/{pid}/net/tcp once for inode matching
                let tcp_data =
                    std::fs::read_to_string(format!("/proc/{}/net/tcp", pid)).unwrap_or_default();
                for entry in entries.flatten() {
                    // Read where each fd points
                    let link = match fs::read_link(entry.path()) {
                        Ok(l) => l.to_string_lossy().to_string(),
                        Err(_) => continue,
                    };
                    // Only check socket fds
                    if !link.starts_with("socket:[") {
                        continue;
                    }
                    // Extract inode number from "socket:[12345]"
                    let inode = link.trim_start_matches("socket:[").trim_end_matches(']');
                    // Check if this inode is in LISTEN state in /proc/{pid}/net/tcp
                    for tcp_line in tcp_data.lines().skip(1) {
                        let cols: Vec<&str> = tcp_line.split_whitespace().collect();
                        // Column 3 (0-indexed) = state, Column 9 = inode
                        if cols.len() >= 10 && cols[3] == "0A" && cols[9] == inode {
                            return true;
                        }
                    }
                }
                false
            })();
            if owns_listener {
                info!(
                    "PID {} is standalone and owns a listening socket -- not stale",
                    pid
                );
                false
            } else {
                info!(
                    "PID {} is a standalone MCP server (stdin={}) -- treating as stale",
                    pid, fd0_str
                );
                true
            }
        } else {
            false
        }
    } else {
        // Audit-7 MCP7-M3 FIX: fd/0 is neither socket nor pipe.
        // If fd0_str is empty, read_link failed (process fd unreadable) -- treat as dead.
        // If fd0_str is "/dev/null", stdin is disconnected -- treat as dead.
        // If fd0_str points to a real file/device (e.g., /dev/pts/0), stdin may be alive.
        // Previously: `!fd0_str.is_empty()` treated empty (unreadable) as healthy -- inverted.
        fd0_str.is_empty() || fd0_str.contains("/dev/null")
    };

    if !stdin_is_dead {
        warn!(
            "PID {} holds flock but appears to be a healthy process — not killing",
            pid
        );
        return false;
    }

    // Kill the stale standalone process
    warn!(
        "STALE STANDALONE DETECTED: PID {} holds flock on '{}' with dead stdio. \
         Sending SIGTERM.",
        pid,
        pid_path.display()
    );

    unsafe {
        libc::kill(pid, libc::SIGTERM);
    }

    for i in 0..20 {
        sleep(Duration::from_millis(100)).await;
        if unsafe { libc::kill(pid, 0) } != 0 {
            info!(
                "Stale PID {} terminated after SIGTERM ({}ms)",
                pid,
                (i + 1) * 100
            );
            sleep(Duration::from_millis(200)).await;
            return true;
        }
    }

    warn!(
        "PID {} did not exit after SIGTERM (2s), sending SIGKILL",
        pid
    );
    unsafe {
        libc::kill(pid, libc::SIGKILL);
    }

    for i in 0..10 {
        sleep(Duration::from_millis(100)).await;
        if unsafe { libc::kill(pid, 0) } != 0 {
            info!(
                "Stale PID {} terminated after SIGKILL ({}ms)",
                pid,
                (i + 1) * 100
            );
            sleep(Duration::from_millis(200)).await;
            return true;
        }
        let status_path = format!("/proc/{}/status", pid);
        let is_zombie = fs::read_to_string(&status_path)
            .map(|s| s.contains("State:\tZ") || s.contains("State:\tX"))
            .unwrap_or(false);
        if is_zombie {
            info!("PID {} is zombie after SIGKILL — flock released", pid);
            return true;
        }
    }

    error!(
        "FATAL: PID {} still alive after SIGKILL — manual kill required: kill -9 {}",
        pid, pid
    );
    false
}

// ============================================================================
// Reality-Loop Stdio Singleton Guard
//
// Root cause being fixed (2026-05-07): `run_reality_loop_stdio()` previously
// performed NO singleton enforcement. Each `/mcp` reconnect from Claude Code
// spawns a fresh subprocess; the previous one's stdin is left half-open and
// never EOFs (Claude Code's stdio transport does not call shutdown(SHUT_WR)
// on disconnect — see github.com/anthropics/claude-code issues #1935, #11778,
// #22612, #33947, #43177). The orphaned MCP would then sit in `read_line`
// forever until either MAX_STDIO_IDLE_SECS (1h) elapsed or the operator
// killed it manually. Result: 2-3 reality-loop MCP processes alive at once,
// each holding a separate copy of in-memory state.
//
// This module implements a takeover-style flock singleton:
//   1. New MCP attempts flock(LOCK_EX|LOCK_NB) on a fixed lockfile.
//   2. If acquired → writes its PID, proceeds.
//   3. If EWOULDBLOCK → reads the holder PID, verifies it is also a
//      `--mode reality-loop` MCP (refuses to evict unrelated processes),
//      sends SIGTERM (2s grace) → SIGKILL (1s grace), then re-acquires.
//   4. If a third process raced us in after the eviction → loud error.
// Drop releases the flock and removes the lockfile.
//
// Different from `PidFileGuard` (DB-path-based). Reality-loop stdio does not
// open RocksDB, so we use a host-global lockfile instead of a per-DB one.
// ============================================================================

/// Compute the lockfile path for the reality-loop stdio singleton.
///
/// Returns `$HOME/.cache/cgreality/reality-loop-stdio.lock` (consistent with
/// the existing `~/.cache/cgreality/active_runtime_root` location used by
/// CGREALITY_RUNTIME_ROOT_HINT). Errors out loudly if HOME is unset rather
/// than silently falling back to /tmp (which would create cross-user
/// collisions on shared hosts).
#[cfg(unix)]
fn reality_loop_singleton_lock_path() -> Result<PathBuf> {
    let home = std::env::var("HOME").map_err(|e| {
        anyhow::anyhow!(
            "HOME env var is required to compute the reality-loop singleton lock path: {}",
            e
        )
    })?;
    Ok(PathBuf::from(home)
        .join(".cache")
        .join("cgreality")
        .join("reality-loop-stdio.lock"))
}

/// Read /proc/PID/cmdline as a space-joined string.
/// Returns None if /proc is unreadable for that PID (process dead or kernel
/// without procfs).
#[cfg(target_os = "linux")]
fn read_process_cmdline(pid: i32) -> Option<String> {
    std::fs::read_to_string(format!("/proc/{}/cmdline", pid))
        .ok()
        .map(|s| s.replace('\0', " ").trim().to_string())
}

/// True iff PID looks like a context-graph-mcp process running in
/// `--mode reality-loop`. False on any read failure or non-match — we err
/// on the side of "do not evict" when classification is uncertain.
#[cfg(target_os = "linux")]
fn is_reality_loop_mcp_process(pid: i32) -> bool {
    let Some(cmdline) = read_process_cmdline(pid) else {
        return false;
    };
    cmdline.contains("context-graph-mcp") && cmdline.contains("--mode reality-loop")
}

/// Evict an existing reality-loop MCP holder via SIGTERM (2s grace) then
/// SIGKILL (1s grace).
///
/// Returns true if the holder is dead (or never was alive), false if eviction
/// failed and the caller should propagate an error.
///
/// Refuses to evict:
///   - PID ≤ 1 (init or invalid)
///   - Our own PID (paranoia — flock should have succeeded if so)
///   - Any process whose cmdline does NOT match a reality-loop MCP (the
///     lockfile may have been touched by an unrelated tool; refuse to harm
///     foreign processes)
#[cfg(target_os = "linux")]
async fn evict_reality_loop_singleton_holder(pid: i32) -> bool {
    use tokio::time::{sleep, Duration};

    if pid <= 1 {
        warn!(
            "Refusing to evict reality-loop singleton holder PID {} (init or invalid)",
            pid
        );
        return false;
    }
    let our_pid = std::process::id() as i32;
    if pid == our_pid {
        error!(
            "Reality-loop singleton lockfile contains our own PID {} but flock is held — \
             refusing to self-evict; investigate fd inheritance or stale state",
            pid
        );
        return false;
    }

    // Liveness probe BEFORE classification: a dead process needs no eviction
    // and the kernel has already released its flock (so why are we here?).
    if unsafe { libc::kill(pid, 0) } != 0 {
        let errno = std::io::Error::last_os_error();
        if errno.raw_os_error() == Some(libc::ESRCH) {
            info!(
                "Reality-loop singleton holder PID {} is already dead (ESRCH); \
                 flock should be releasable",
                pid
            );
            return true;
        }
        warn!(
            "kill({}, 0) probe failed with non-ESRCH errno {} — treating as alive",
            pid, errno
        );
    }

    if !is_reality_loop_mcp_process(pid) {
        let cmdline = read_process_cmdline(pid).unwrap_or_default();
        error!(
            "Reality-loop singleton lockfile holder PID {} is NOT a reality-loop MCP \
             (cmdline: '{}'). Refusing to evict an unrelated process. \
             Resolve manually: 'kill {}' to terminate, or 'rm <lockfile>' if you are \
             certain the PID is stale.",
            pid, cmdline, pid
        );
        return false;
    }

    info!(
        "TAKEOVER: evicting older reality-loop MCP PID {} via SIGTERM (2s grace)",
        pid
    );
    if unsafe { libc::kill(pid, libc::SIGTERM) } != 0 {
        let errno = std::io::Error::last_os_error();
        if errno.raw_os_error() == Some(libc::ESRCH) {
            info!(
                "PID {} died between liveness probe and SIGTERM (ESRCH)",
                pid
            );
            return true;
        }
        error!("kill({}, SIGTERM) failed: {}", pid, errno);
        return false;
    }

    // 2-second SIGTERM grace, polled every 100ms
    for i in 0..20 {
        sleep(Duration::from_millis(100)).await;
        if unsafe { libc::kill(pid, 0) } != 0 {
            info!(
                "Old reality-loop MCP PID {} exited after SIGTERM ({}ms)",
                pid,
                (i + 1) * 100
            );
            // Kernel needs a tick to fully release the flock
            sleep(Duration::from_millis(200)).await;
            return true;
        }
    }

    warn!(
        "Reality-loop MCP PID {} ignored SIGTERM after 2s — escalating to SIGKILL",
        pid
    );
    if unsafe { libc::kill(pid, libc::SIGKILL) } != 0 {
        let errno = std::io::Error::last_os_error();
        if errno.raw_os_error() == Some(libc::ESRCH) {
            return true;
        }
        error!("kill({}, SIGKILL) failed: {}", pid, errno);
        return false;
    }
    for i in 0..10 {
        sleep(Duration::from_millis(100)).await;
        if unsafe { libc::kill(pid, 0) } != 0 {
            info!(
                "Old reality-loop MCP PID {} terminated by SIGKILL ({}ms)",
                pid,
                (i + 1) * 100
            );
            sleep(Duration::from_millis(200)).await;
            return true;
        }
        // Zombie check: kill(pid,0)==0 still succeeds for zombies (process-table
        // entry exists) but the kernel HAS released the flock (and all other
        // resources) at the moment of exit. Treat zombie as released.
        let status_path = format!("/proc/{}/status", pid);
        let is_zombie = std::fs::read_to_string(&status_path)
            .map(|s| s.contains("State:\tZ") || s.contains("State:\tX"))
            .unwrap_or(false);
        if is_zombie {
            info!(
                "Old reality-loop MCP PID {} is zombie after SIGKILL — flock released",
                pid
            );
            return true;
        }
    }

    error!(
        "FATAL: reality-loop MCP PID {} did not die after SIGKILL (1s grace) — \
         manual intervention required: kill -9 {}",
        pid, pid
    );
    false
}

/// Holder of the reality-loop stdio singleton flock.
///
/// While this struct is alive, no other reality-loop MCP can acquire the
/// lock. On Drop the kernel releases the flock and we remove the lockfile.
#[cfg(unix)]
struct RealityLoopSingletonGuard {
    path: PathBuf,
    _file: std::fs::File,
}

#[cfg(unix)]
impl RealityLoopSingletonGuard {
    /// Acquire the singleton lock with takeover semantics.
    ///
    /// Up to TWO acquire attempts: the first either succeeds or evicts the
    /// existing holder; the second re-acquires. If the second still
    /// EWOULDBLOCKs, a third process raced us — we exit loudly rather than
    /// loop forever.
    async fn acquire_with_takeover() -> Result<Self> {
        use std::io::{Read, Seek, Write};
        use std::os::unix::io::AsRawFd;

        let path = reality_loop_singleton_lock_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                anyhow::anyhow!(
                    "Cannot create singleton lock directory '{}': {}",
                    parent.display(),
                    e
                )
            })?;
        }

        let mut last_holder_pid: Option<i32> = None;
        for attempt in 1..=2 {
            let file = std::fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(&path)
                .map_err(|e| {
                    anyhow::anyhow!("Cannot open singleton lock '{}': {}", path.display(), e)
                })?;

            let fd = file.as_raw_fd();
            let result = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
            if result == 0 {
                // We hold the lock. Write our PID.
                let mut f = &file;
                if let Err(e) = f.seek(std::io::SeekFrom::Start(0)) {
                    warn!("singleton lock seek failed: {}", e);
                }
                if let Err(e) = f.set_len(0) {
                    warn!("singleton lock truncate failed: {}", e);
                }
                if let Err(e) = write!(f, "{}", std::process::id()) {
                    warn!("singleton lock PID write failed: {}", e);
                }
                if let Err(e) = f.flush() {
                    warn!("singleton lock flush failed: {}", e);
                }
                if attempt == 2 {
                    info!(
                        "Reality-loop singleton acquired AFTER takeover of PID {} (pid={}, path={})",
                        last_holder_pid.unwrap_or(-1),
                        std::process::id(),
                        path.display()
                    );
                } else {
                    info!(
                        "Reality-loop singleton acquired (pid={}, path={})",
                        std::process::id(),
                        path.display()
                    );
                }
                return Ok(RealityLoopSingletonGuard { path, _file: file });
            }

            let errno = std::io::Error::last_os_error();
            if errno.raw_os_error() != Some(libc::EWOULDBLOCK) {
                return Err(anyhow::anyhow!(
                    "flock({}) failed with non-EWOULDBLOCK error: {}",
                    path.display(),
                    errno
                ));
            }

            if attempt == 2 {
                return Err(anyhow::anyhow!(
                    "Reality-loop singleton lock '{}' is still held after eviction of PID {} — \
                     a third reality-loop MCP raced us. Refusing to loop. \
                     Investigate manually: 'lsof {}' and 'ps -ef | grep reality-loop'.",
                    path.display(),
                    last_holder_pid.unwrap_or(-1),
                    path.display()
                ));
            }

            // Read holder PID
            let mut contents = String::new();
            let mut f = &file;
            if let Err(e) = f.seek(std::io::SeekFrom::Start(0)) {
                error!("singleton lock seek-to-read failed while contended: {}", e);
                return Err(anyhow::anyhow!(
                    "Cannot read contended singleton lockfile '{}': seek failed: {}",
                    path.display(),
                    e
                ));
            }
            if let Err(e) = f.read_to_string(&mut contents) {
                error!("singleton lock read failed while contended: {}", e);
                return Err(anyhow::anyhow!(
                    "Cannot read contended singleton lockfile '{}': {}",
                    path.display(),
                    e
                ));
            }
            let holder_str = contents.trim().to_string();
            let holder_pid = match holder_str.parse::<i32>() {
                Ok(pid) => pid,
                Err(parse_err) => {
                    warn!(
                        "Singleton lockfile contained non-numeric content '{}' ({}) — \
                         removing and retrying",
                        holder_str, parse_err
                    );
                    drop(file);
                    if let Err(e) = std::fs::remove_file(&path) {
                        warn!("Failed to remove corrupt singleton lockfile: {}", e);
                    }
                    continue;
                }
            };
            last_holder_pid = Some(holder_pid);

            #[cfg(target_os = "linux")]
            {
                if !evict_reality_loop_singleton_holder(holder_pid).await {
                    return Err(anyhow::anyhow!(
                        "Cannot evict reality-loop singleton holder PID {} — see error log above. \
                         Lockfile: '{}'.",
                        holder_pid,
                        path.display()
                    ));
                }
            }
            #[cfg(not(target_os = "linux"))]
            {
                let _ = holder_pid;
                return Err(anyhow::anyhow!(
                    "Singleton lock contended on non-Linux platform; eviction not implemented. \
                     Manually kill PID {} and remove '{}'.",
                    holder_pid,
                    path.display()
                ));
            }

            drop(file);
            // Brief pause for kernel to release the flock after the evicted
            // process exits. The evict function already polled for exit, but
            // flock release is a separate kernel-tick away.
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
        }
        unreachable!("singleton acquire loop must return or error within 2 attempts")
    }
}

#[cfg(unix)]
impl Drop for RealityLoopSingletonGuard {
    fn drop(&mut self) {
        // The flock is auto-released when self._file closes. Best-effort:
        // remove the lockfile so the next startup sees a clean slate.
        if let Err(e) = std::fs::remove_file(&self.path) {
            warn!(
                "Failed to remove reality-loop singleton lockfile '{}' on drop: {}",
                self.path.display(),
                e
            );
        }
    }
}

#[cfg(not(unix))]
struct RealityLoopSingletonGuard;

#[cfg(not(unix))]
impl RealityLoopSingletonGuard {
    async fn acquire_with_takeover() -> Result<Self> {
        warn!(
            "Reality-loop singleton enforcement requires Unix flock; \
             multi-instance protection disabled on this platform"
        );
        Ok(RealityLoopSingletonGuard)
    }
}

/// Quick TCP connect check (no protocol verification).
/// Used only to detect if a port is in use before attempting to bind.
async fn is_port_in_use(port: u16) -> bool {
    use tokio::net::TcpStream;
    use tokio::time::{timeout, Duration};
    let addr = format!("127.0.0.1:{}", port);
    matches!(
        timeout(Duration::from_millis(200), TcpStream::connect(&addr)).await,
        Ok(Ok(_))
    )
}

/// Inner proxy: single connection attempt from stdio to daemon TCP.
///
/// Returns Ok(()) on clean stdin close (Claude Code exit).
/// Returns Err on TCP connection failure or daemon disconnect.
async fn run_stdio_to_tcp_proxy_inner(daemon_port: u16) -> Result<()> {
    use context_graph_mcp::server::transport::{read_line_bounded, MAX_LINE_BYTES};
    use tokio::io::{AsyncWriteExt, BufReader};
    use tokio::net::TcpStream;
    use tokio::time::Duration;

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
    // Step 7: 120s read timeout catches genuine daemon deadlocks
    let stdout_task = tokio::spawn(async move {
        let mut stdout = tokio::io::stdout();
        let mut line = String::new();
        loop {
            line.clear();
            // 120s timeout: generous enough for embedding warmup (~115s)
            // but catches genuine daemon deadlocks
            match tokio::time::timeout(
                Duration::from_secs(120),
                read_line_bounded(&mut reader, &mut line, MAX_LINE_BYTES),
            )
            .await
            {
                Ok(Ok(0)) => {
                    info!("Daemon closed connection");
                    break;
                }
                Ok(Ok(_)) => {
                    if let Err(e) = stdout.write_all(line.as_bytes()).await {
                        error!("Failed to write to stdout: {}", e);
                        break;
                    }
                    if let Err(e) = stdout.flush().await {
                        error!("Failed to flush stdout: {}", e);
                        break;
                    }
                }
                Ok(Err(e)) => {
                    error!("Failed to read from daemon (bounded read): {}", e);
                    break;
                }
                Err(_) => {
                    error!(
                        "Daemon read timeout (120s) — daemon may be stuck. \
                         This proxy will disconnect and trigger reconnection."
                    );
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
        match read_line_bounded(&mut stdin, &mut line, MAX_LINE_BYTES).await {
            Ok(0) => {
                info!("Stdin closed");
                break;
            }
            Ok(_) => {
                if let Err(e) = writer.write_all(line.as_bytes()).await {
                    error!("Failed to write to daemon: {}", e);
                    return Err(anyhow::anyhow!("TCP write to daemon failed: {}", e));
                }
                if let Err(e) = writer.flush().await {
                    error!("Failed to flush to daemon: {}", e);
                    return Err(anyhow::anyhow!("TCP flush to daemon failed: {}", e));
                }
            }
            Err(e) => {
                let err_str = e.to_string();
                if err_str.contains("Stdin closed") {
                    info!("Stdin closed");
                    break;
                }
                error!("Failed to read from stdin (bounded read): {}", e);
                return Err(anyhow::anyhow!("Stdin read failed: {}", e));
            }
        }
    }

    // Abort stdout task immediately — don't wait for 120s daemon read timeout
    stdout_task.abort();
    match stdout_task.await {
        Ok(()) => {}
        Err(e) if e.is_cancelled() => {
            // Expected — we just aborted it
        }
        Err(e) => {
            error!("Stdio stdout task panicked: {}", e);
        }
    }
    info!("Stdio proxy shutdown");
    Ok(())
}

/// Run as stdio-to-TCP proxy with automatic reconnection.
///
/// If the TCP connection to the daemon drops (daemon restart, network error),
/// retries up to 5 times with exponential backoff (200ms, 400ms, 800ms, 1.6s, 3.2s).
/// Each retry verifies daemon health before reconnecting.
///
/// Clean exit (stdin closed by Claude Code) does NOT trigger reconnection.
async fn run_stdio_to_tcp_proxy(daemon_port: u16) -> Result<()> {
    let max_reconnects: u32 = 5;
    let mut reconnect_count: u32 = 0;

    loop {
        match run_stdio_to_tcp_proxy_inner(daemon_port).await {
            Ok(()) => {
                // Clean shutdown: stdin closed means Claude Code is exiting.
                info!("Proxy shut down cleanly (stdin closed)");
                return Ok(());
            }
            Err(e) => {
                reconnect_count += 1;

                // Stdin-closed errors should not trigger reconnect
                let err_str = e.to_string();
                if err_str.contains("stdin") || err_str.contains("Stdin closed") {
                    info!("Proxy stdin closed — exiting");
                    return Ok(());
                }

                if reconnect_count > max_reconnects {
                    error!(
                        "Proxy connection failed {} times, giving up. Last error: {}",
                        max_reconnects, e
                    );
                    return Err(anyhow::anyhow!(
                        "Proxy lost connection to daemon {} times. \
                         The daemon may have crashed. Check: fuser {}/tcp",
                        max_reconnects,
                        daemon_port
                    ));
                }

                // Exponential backoff: 200ms, 400ms, 800ms, 1.6s, 3.2s
                let delay_ms = 200u64 * (1u64 << (reconnect_count - 1));
                warn!(
                    "Proxy connection lost (attempt {}/{}): {}. Reconnecting in {}ms...",
                    reconnect_count, max_reconnects, e, delay_ms
                );
                tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;

                // Verify daemon is still alive before reconnecting
                if !is_daemon_healthy(daemon_port).await {
                    error!(
                        "Daemon on port {} is not responding after connection loss. \
                         It may have crashed. Check: fuser {}/tcp",
                        daemon_port, daemon_port
                    );
                    return Err(anyhow::anyhow!(
                        "Daemon on port {} is not responding. It may have crashed.",
                        daemon_port
                    ));
                }

                info!("Daemon is healthy, reconnecting...");
            }
        }
    }
}

/// Start daemon server in background and return when ready.
///
/// Spawns a task that runs the TCP server and waits until it's accepting connections.
/// The PID guard is moved into the daemon task so it lives exactly as long as the daemon.
/// Signal handlers are registered inside the daemon task for graceful shutdown.
///
/// NOTE: On Unix, this is replaced by spawn_daemon_process() which runs the daemon
/// in a separate OS process (survives proxy death). Kept for non-Unix fallback.
#[cfg(not(unix))]
async fn start_daemon_server(
    config: Config,
    warm_first: bool,
    daemon_port: u16,
    pid_guard: Option<PidFileGuard>,
) -> Result<()> {
    use tokio::time::{sleep, Duration};

    info!("Starting daemon server on port {}...", daemon_port);

    // Create a modified config for the daemon
    let mut daemon_config = config;
    daemon_config.mcp.tcp_port = daemon_port;
    daemon_config.mcp.transport = "tcp".to_string();

    // Create the server
    let server = server::McpServer::new(daemon_config, warm_first).await?;

    // Spawn the daemon server — store JoinHandle so we detect crashes.
    // CRITICAL: pid_guard is moved into this task. It will be dropped
    // only when the daemon task exits (crash, signal, or normal shutdown).
    // This prevents the guard from being released when the proxy's main() returns.
    let daemon_handle = tokio::spawn(async move {
        // pid_guard lives here — dropped only when this task exits
        let _guard = pid_guard;

        // Register signal handlers within the daemon task.
        // These mirror the standalone mode's signal handling.
        let shutdown_signal = async {
            #[cfg(unix)]
            {
                let mut sigterm =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                        .expect("FATAL: failed to register SIGTERM handler in daemon task");

                tokio::select! {
                    _ = sigterm.recv() => {
                        info!("Daemon received SIGTERM — initiating graceful shutdown");
                    }
                    _ = tokio::signal::ctrl_c() => {
                        info!("Daemon received SIGINT (Ctrl+C) — initiating graceful shutdown");
                    }
                }
            }
            #[cfg(not(unix))]
            {
                tokio::signal::ctrl_c()
                    .await
                    .expect("FATAL: failed to register Ctrl+C handler");
                info!("Daemon received Ctrl+C — initiating graceful shutdown");
            }
        };

        // Run TCP server until either it exits or a signal arrives
        tokio::select! {
            result = server.run_tcp() => {
                match result {
                    Ok(()) => info!("Daemon TCP server exited normally"),
                    Err(e) => error!("CRITICAL: Daemon TCP server crashed: {}", e),
                }
            }
            _ = shutdown_signal => {
                info!("Daemon shutting down gracefully...");
                server.shutdown().await;
                info!("Daemon graceful shutdown complete");
            }
        }

        // _guard drops here — flock released, PID file removed
        Ok::<(), anyhow::Error>(())
    });

    // Wait for daemon to be ready (accept connections)
    for _ in 0..50 {
        // 5 seconds max
        if daemon_handle.is_finished() {
            error!("Daemon server task exited unexpectedly during startup");
            return Err(anyhow::anyhow!(
                "Daemon server exited before accepting connections"
            ));
        }
        sleep(Duration::from_millis(100)).await;
        if is_daemon_healthy(daemon_port).await {
            info!("Daemon server ready on port {}", daemon_port);
            return Ok(());
        }
    }

    Err(anyhow::anyhow!(
        "Daemon server failed to start within 5 seconds"
    ))
}

/// Spawn the daemon as a separate OS process that survives the proxy's death.
///
/// Uses `setsid()` to make the child a session leader, so it doesn't receive
/// SIGHUP/SIGTERM when the proxy's terminal or process group dies. The child
/// process runs with `--daemon-server-only` which triggers the headless daemon
/// handler in main().
///
/// Polls `is_daemon_healthy()` with 500ms intervals until the daemon is ready
/// or 120s timeout (generous for model warmup: 14 embedders on RTX 5090).
#[cfg(unix)]
async fn spawn_daemon_process(
    daemon_port: u16,
    config_path: Option<&Path>,
    warm_first: bool,
) -> Result<()> {
    use std::os::unix::process::CommandExt;
    use tokio::time::{sleep, Duration};

    let exe = std::env::current_exe()
        .map_err(|e| anyhow::anyhow!("Cannot determine current executable path: {}", e))?;

    let mut cmd = std::process::Command::new(&exe);
    cmd.arg("--daemon-server-only");
    cmd.arg("--daemon-port");
    cmd.arg(daemon_port.to_string());

    if let Some(path) = config_path {
        cmd.arg("--config");
        cmd.arg(path);
    }

    if !warm_first {
        cmd.arg("--no-warm");
    }

    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::inherit());

    // setsid() makes the child a session leader — it won't receive
    // SIGHUP/SIGTERM when the parent's terminal or process group dies.
    unsafe {
        cmd.pre_exec(|| {
            // INFRA-H1 FIX: Check setsid() return value — failure means
            // child stays in parent's process group and dies with terminal.
            if libc::setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }

    let child = cmd
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to spawn daemon process: {}", e))?;

    let child_pid = child.id();
    info!("Spawned daemon process PID {} (exe: {:?})", child_pid, exe);

    // Wait for daemon to become healthy (up to 120s for model warmup)
    let max_wait = Duration::from_secs(120);
    let poll_interval = Duration::from_millis(500);
    let start = std::time::Instant::now();

    loop {
        if start.elapsed() > max_wait {
            error!(
                "Daemon process PID {} did not become healthy within 120s",
                child_pid
            );
            unsafe {
                libc::kill(child_pid as i32, libc::SIGKILL);
            }
            return Err(anyhow::anyhow!(
                "Daemon process failed to start within 120s (PID {}). \
                 Check RUST_LOG=info for daemon stderr output.",
                child_pid
            ));
        }

        // Check if child is still alive
        let alive = unsafe { libc::kill(child_pid as i32, 0) } == 0;
        if !alive {
            return Err(anyhow::anyhow!(
                "Daemon process PID {} exited during startup. \
                 Check RUST_LOG=info for daemon stderr output.",
                child_pid
            ));
        }

        // Check health
        if is_daemon_healthy(daemon_port).await {
            info!(
                "Daemon process PID {} is healthy on port {} (started in {:.1}s)",
                child_pid,
                daemon_port,
                start.elapsed().as_secs_f64()
            );
            return Ok(());
        }

        sleep(poll_interval).await;
    }
}

/// Synchronous entry point: sets environment variables BEFORE any tokio threads
/// exist, then hands off to the async entry point.
///
/// INFRA-H2 FIX: `env::set_var` is unsound when other threads may call `getenv`
/// concurrently (POSIX UB, and Rust 2024 edition marks it `unsafe`).
/// `#[tokio::main]` spawns the runtime *before* the body executes, so any
/// `env::set_var` inside it races with tokio worker threads.  By constructing
/// the runtime manually we guarantee the `set_var` happens in a single-threaded
/// context.
fn main() -> Result<()> {
    // CRITICAL: Set env vars while still single-threaded (no tokio runtime yet).
    env::set_var("CONTEXT_GRAPH_MCP_QUIET", "1");

    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async_main())
}

/// Install kernel-level parent-death tracking (Linux `PR_SET_PDEATHSIG`).
///
/// When the process that forked/spawned us dies for ANY reason, the kernel
/// immediately delivers `SIGTERM` to us. This prevents the MCP server from
/// ever outliving its parent MCP client, which otherwise leaves a zombie
/// process holding the RocksDB flock.
///
/// The value is inherited across `fork` on Linux ≥ 3.4, and survives
/// `execve` only when we set it ourselves (which we do here). Calling this
/// at the top of `async_main` covers:
///   - normal parent exit (our stdio reader sees EOF anyway)
///   - parent SIGKILL / OOM / crash (stdio EOF may NOT arrive; pdeathsig fires)
///   - parent losing track of us and exiting unexpectedly
///
/// The no-op variant for non-Linux targets keeps the code portable.
#[cfg(target_os = "linux")]
fn install_parent_death_signal() {
    // SAFETY: `prctl` is an FFI call that only sets a kernel flag for the
    // current thread. Arguments are constants defined by the kernel ABI.
    // The only failure mode (ENOSYS on ancient kernels) returns -1 and we
    // log a warning but continue — the pdeathsig is defense in depth, not
    // the only safeguard (we also have SIGTERM handlers and stdio EOF).
    let rc = unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM, 0, 0, 0) };
    if rc != 0 {
        warn!(
            "prctl(PR_SET_PDEATHSIG) failed (errno={}) — parent-death tracking disabled",
            std::io::Error::last_os_error()
        );
    } else {
        debug!("Parent-death signal installed: SIGTERM on parent exit");
    }

    // Also check if our parent is already PID 1 (init) — that means we were
    // orphaned before we even started (e.g., launched from a script that
    // already died). In that case, exit immediately rather than linger.
    //
    // SAFETY: getppid is a pure syscall with no side effects.
    let ppid = unsafe { libc::getppid() };
    if ppid == 1 {
        warn!("Launched as orphan (PPID=1); exiting to avoid holding DB flock with no client");
        std::process::exit(0);
    }
}

#[cfg(not(target_os = "linux"))]
fn install_parent_death_signal() {
    // Other platforms: no-op. The existing SIGTERM handler and stdio EOF
    // detection are the only safety nets available.
}

/// Arm the doomsday shutdown watchdog.
///
/// Spawns a plain OS thread (not a tokio task) that sleeps [`DOOMSDAY_SECS`]
/// and then calls `libc::_exit(0)` unconditionally. This is the last-resort
/// guarantee that the MCP server cannot hang forever during shutdown:
///   - runs on an OS thread, so a wedged tokio runtime cannot block it
///   - uses `_exit` (not `exit`), bypassing Rust destructors that could hang
///   - the kernel releases all file locks and file descriptors on `_exit`,
///     so the RocksDB flock is freed immediately
///
/// Idempotent via a one-shot `AtomicBool` — multiple calls only arm once.
/// Used by every shutdown path (SIGTERM, SIGINT, stdio EOF).
fn arm_doomsday_watchdog(trigger: &'static str) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static ARMED: AtomicBool = AtomicBool::new(false);
    if ARMED.swap(true, Ordering::SeqCst) {
        return; // already armed by a prior signal
    }
    info!(
        "Doomsday watchdog armed ({}): will force-exit in {}s if graceful shutdown hangs",
        trigger, DOOMSDAY_SECS
    );
    // Code-simplifier fix #7 (per tasks/lessons.md lesson on `std::thread::spawn`):
    // spawn CAN fail under thread exhaustion (EAGAIN). A panicking watchdog is
    // its own worst enemy; log-and-continue and let the existing graceful
    // shutdown deadline (~10s) handle the exit path. We DO NOT reset ARMED — a
    // subsequent arm call (from another shutdown signal) may still succeed.
    let spawn_result = std::thread::Builder::new()
        .name("doomsday-watchdog".into())
        .spawn(|| {
            std::thread::sleep(std::time::Duration::from_secs(DOOMSDAY_SECS));
            eprintln!(
                "[DOOMSDAY] {}s graceful-shutdown deadline exceeded — force exiting via libc::_exit(0)",
                DOOMSDAY_SECS
            );
            // SAFETY: _exit is async-signal-safe and always succeeds. We are
            // past the point of caring about destructors; the OS reaps all
            // resources (flocks, fds, memory) for us.
            unsafe {
                libc::_exit(0);
            }
        });
    if let Err(e) = spawn_result {
        error!(
            "Failed to spawn doomsday watchdog ({}): {} — relying on graceful-shutdown deadline",
            trigger, e
        );
    }
}

/// How long we allow graceful shutdown to take before the doomsday watchdog
/// force-exits the process. Must exceed the existing 10s in-loop deadline
/// (so graceful shutdown gets priority) but not so long that a zombie
/// lingers meaningfully.
const DOOMSDAY_SECS: u64 = 20;

async fn run_reality_loop_stdio() -> Result<()> {
    use crate::handlers::tools::reality_loop;
    use crate::tools::tool_names;
    use serde_json::{json, Value};
    use tokio::io::{AsyncBufReadExt, BufReader};

    // SINGLETON ENFORCEMENT (2026-05-07 root-cause fix). Acquire a flock on a
    // host-wide reality-loop lockfile BEFORE any other state is touched. If
    // another reality-loop MCP holds the lock — the dominant case is "Claude
    // Code did `/mcp reconnect` and the previous subprocess never EOFed" — the
    // takeover protocol kills it (SIGTERM 2s grace, then SIGKILL) and reclaims
    // the lock. If a non-reality-loop process holds the lock, we exit loudly.
    // The guard is held for the lifetime of this function; Drop releases the
    // flock and removes the lockfile.
    //
    // Without this guard, every `/mcp reconnect` orphaned the prior MCP because
    // Claude Code's stdio transport does not call shutdown(SHUT_WR) on the
    // socketpair, so the orphan's `read_line` blocks forever (until the 1h
    // MAX_STDIO_IDLE_SECS watchdog fires). Two or three concurrent reality-loop
    // MCPs were routinely observed before this guard.
    let _singleton_guard = RealityLoopSingletonGuard::acquire_with_takeover()
        .await
        .map_err(|e| anyhow::anyhow!("FATAL: reality-loop singleton acquire failed: {}", e))?;

    // ZOMBIE-PREVENTION (root-cause fix for accumulated stale --mode reality-loop
    // processes): install the same defenses the default-mode stdio path uses.
    //
    //   1. PR_SET_PDEATHSIG: kernel delivers SIGTERM if Claude Code (our parent)
    //      dies for any reason — crash, kill -9, OOM, hot-reload. This makes a
    //      bare-orphaned MCP server impossible on Linux.
    //
    //   2. SIGTERM/SIGINT signal handlers: trigger the doomsday watchdog the
    //      moment a signal lands, so we cannot ignore a kill request.
    //
    //   3. Doomsday watchdog (armed on every shutdown trigger): a separate OS
    //      thread that calls libc::_exit(0) after DOOMSDAY_SECS regardless of
    //      whether tokio is wedged. The kernel reaps all fds + flocks on _exit,
    //      so even a hung in-flight tool call cannot leave us as a zombie.
    //
    // Without these, an `--mode reality-loop` server lingers indefinitely after
    // Claude Code disconnects (observed: 3 procs alive 12+ hours, holding stdin
    // sockets and PID file references).
    install_parent_death_signal();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin);
    let mut writer = tokio::io::BufWriter::new(stdout);
    let mut line = String::new();

    // Signal handlers run in parallel with the request loop; first to fire wins.
    #[cfg(unix)]
    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .map_err(|e| anyhow::anyhow!("failed to register SIGTERM handler: {e}"))?;

    // ZOMBIE-PREVENTION (H1, follow-up to PR_SET_PDEATHSIG): periodic idle
    // watchdog covering the cases the parent-death signal misses. stdin here is
    // a socketpair; it only EOFs when the peer calls shutdown(SHUT_WR). A
    // claude process that goes silent (SIGSTOP, debugger break, hot-reload that
    // holds the connection) would otherwise leave us blocked in read_line
    // forever. Two checks fire on every IDLE_TICK_SECS interval:
    //   1. getppid() == 1 ⇒ orphaned (parent reparented to init). PR_SET_PDEATHSIG
    //      catches this on Linux but is per-thread; this re-checks runtime-wide.
    //   2. last_byte.elapsed() > MAX_STDIO_IDLE_SECS ⇒ silent parent. Bound at 1h
    //      so /loop 15m or 30m intervals are nowhere near the threshold.
    // On any healthy tick we return `usize::MAX` as a sentinel so the loop
    // continues without treating the tick as a request.
    const IDLE_TICK_SECS: u64 = 30;
    const MAX_STDIO_IDLE_SECS: u64 = 3600;
    let mut idle_tick = tokio::time::interval(std::time::Duration::from_secs(IDLE_TICK_SECS));
    idle_tick.tick().await; // skip the immediate first tick
    let mut last_byte = std::time::Instant::now();

    loop {
        line.clear();
        let read = tokio::select! {
            biased; // poll signals first so a kill request is never starved
            _ = async {
                #[cfg(unix)]
                { sigterm.recv().await; }
                #[cfg(not(unix))]
                { std::future::pending::<()>().await; }
            } => {
                eprintln!("[reality-loop-stdio] SIGTERM received — initiating shutdown");
                arm_doomsday_watchdog("SIGTERM");
                0usize
            }
            _ = tokio::signal::ctrl_c() => {
                eprintln!("[reality-loop-stdio] SIGINT received — initiating shutdown");
                arm_doomsday_watchdog("SIGINT");
                0usize
            }
            _ = idle_tick.tick() => {
                #[cfg(unix)]
                let ppid = unsafe { libc::getppid() };
                #[cfg(not(unix))]
                let ppid: i32 = -1;
                if ppid == 1 {
                    eprintln!(
                        "[reality-loop-stdio] parent reparented to PID 1 (orphaned) — initiating shutdown"
                    );
                    arm_doomsday_watchdog("orphan_ppid_1");
                    0usize
                } else if last_byte.elapsed() >= std::time::Duration::from_secs(MAX_STDIO_IDLE_SECS) {
                    eprintln!(
                        "[reality-loop-stdio] stdio idle {}s exceeds {}s — initiating shutdown",
                        last_byte.elapsed().as_secs(), MAX_STDIO_IDLE_SECS
                    );
                    arm_doomsday_watchdog("stdio_idle_timeout");
                    0usize
                } else {
                    // Healthy idle tick: continue waiting without treating this
                    // as a request. usize::MAX is impossible from a real read
                    // (would mean 18 exabytes), so it's safe as a sentinel.
                    usize::MAX
                }
            }
            r = reader.read_line(&mut line) => r?,
        };
        if read == 0 {
            // EOF (Claude Code closed stdio) OR signal-triggered fall-through.
            // Arm the watchdog so any hung in-flight tool call cannot pin us.
            arm_doomsday_watchdog("stdio EOF");
            break;
        }
        if read == usize::MAX {
            // Healthy idle tick — continue waiting for the next byte/signal.
            continue;
        }
        last_byte = std::time::Instant::now();
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let request: Value = match serde_json::from_str(trimmed) {
            Ok(value) => value,
            Err(e) => {
                let response = json!({
                    "jsonrpc": "2.0",
                    "id": Value::Null,
                    "error": {"code": -32700, "message": format!("Parse error: {e}")}
                });
                write_jsonrpc_line(&mut writer, &response).await?;
                continue;
            }
        };
        let id = request.get("id").cloned().unwrap_or(Value::Null);
        let method = request.get("method").and_then(Value::as_str).unwrap_or("");
        let response = match method {
            "initialize" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "capabilities": {"tools": {}},
                    "serverInfo": {"name": "cgreality", "version": env!("CARGO_PKG_VERSION")}
                }
            }),
            "notifications/initialized" => continue,
            "tools/list" => json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {"tools": crate::tools::definitions::get_reality_loop_tool_definitions()}
            }),
            "tools/call" => {
                let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
                let name = params.get("name").and_then(Value::as_str).unwrap_or("");
                let args = params
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| json!({}));
                let result = match name {
                    tool_names::REALITY_LATEST_ROOT => reality_loop::analyze::reality_latest_root(args).await,
                    tool_names::REALITY_ATTEMPT_SUMMARY => reality_loop::analyze::reality_attempt_summary(args).await,
                    tool_names::REALITY_OFFICIAL_REPORT => reality_loop::analyze::reality_official_report(args).await,
                    tool_names::REALITY_PROBLEM_PACKET => reality_loop::analyze::reality_problem_packet(args).await,
                    tool_names::REALITY_SIGNAL => reality_loop::analyze::reality_signal(args).await,
                    tool_names::DYNAMICJEPA_REALITY_FOR_ATTEMPT => reality_loop::analyze::dynamicjepa_reality_for_attempt(args).await,
                    tool_names::REALITY_FAILURE => reality_loop::analyze::reality_failure(args).await,
                    tool_names::REALITY_TRIGGER_DECISION => reality_loop::analyze::reality_trigger_decision(args).await,
                    tool_names::REALITY_HARNESS_TRANSITIONS => reality_loop::analyze::reality_harness_transitions(args).await,
                    tool_names::REALITY_COMPARE_ATTEMPTS => reality_loop::analyze::reality_compare_attempts(args).await,
                    tool_names::REALITY_REPLAY_ARTIFACT => reality_loop::analyze::reality_replay_artifact(args).await,
                    tool_names::REALITY_AUDIT_TRAIL => reality_loop::analyze::reality_runtime_audit_trail(args).await,
                    tool_names::REALITY_QUERY_LEDGER => reality_loop::interact::reality_query_ledger(args).await,
                    tool_names::HARNESS_OPEN_WINDOW => reality_loop::alter::harness_open_window(args).await,
                    tool_names::HARNESS_APPLY_LINE_WINDOW_EDIT => reality_loop::alter::harness_apply_line_window_edit(args).await,
                    tool_names::HARNESS_RUN_COMMAND => reality_loop::alter::harness_run_command(args).await,
                    tool_names::HARNESS_GIT_DIFF => reality_loop::alter::harness_git_diff(args).await,
                    tool_names::HARNESS_GIT_STATUS => reality_loop::alter::harness_git_status(args).await,
                    tool_names::HARNESS_VERIFY_STATE => reality_loop::alter::harness_verify_state(args).await,
                    tool_names::OPTIMIZER_RECORD_DECISION => reality_loop::optimizer::optimizer_record_decision(args).await,
                    tool_names::OPTIMIZER_RECORD_RECOMMENDATION => reality_loop::optimizer::optimizer_record_recommendation(args).await,
                    tool_names::OPTIMIZER_RECORD_HARNESS_TRANSITION => reality_loop::optimizer::optimizer_record_harness_transition(args, true).await,
                    tool_names::OPTIMIZER_BANDIT_SELECT => reality_loop::bandit::optimizer_bandit_select(args).await,
                    tool_names::OPTIMIZER_BANDIT_RECORD_REWARD => reality_loop::bandit::optimizer_bandit_record_reward(args).await,
                    tool_names::OPTIMIZER_BANDIT_STATE => reality_loop::bandit::optimizer_bandit_state(args).await,
                    tool_names::OPTIMIZER_RECALL_RECOMMENDATIONS => reality_loop::recommendations::optimizer_recall_recommendations(args).await,
                    tool_names::OPTIMIZER_COMPUTE_INFLUENCE => reality_loop::influence::optimizer_compute_influence(args).await,
                    tool_names::OPTIMIZER_WITNESS_CHAIN_VERIFY => reality_loop::witness_chain::optimizer_witness_chain_verify(args).await,
                    tool_names::OPTIMIZER_WITNESS_CHAIN_DIFF => reality_loop::witness_chain::optimizer_witness_chain_diff(args).await,
                    tool_names::OPTIMIZER_WITNESS_CHAIN_REPAIR_LEGACY => reality_loop::witness_chain_repair::optimizer_witness_chain_repair_legacy(args).await,
                    tool_names::REALITY_SHIFT_LOG => reality_loop::shift_log::reality_shift_log(args).await,
                    tool_names::REALITY_SHIFT_COMPARE_TO_MY_VIEW => reality_loop::shift_log::reality_shift_compare_to_my_view(args).await,
                    // Phase 15: autoresearch engine
                    tool_names::EXPERIMENT_REGISTRY_LIST => reality_loop::autoresearch::experiment_registry_list(args).await,
                    tool_names::EXPERIMENT_REGISTRY_GET => reality_loop::autoresearch::experiment_registry_get(args).await,
                    tool_names::CHAMPION_STATE_GET => reality_loop::autoresearch::champion_state_get(args).await,
                    tool_names::ATTEMPTS_HISTORY_QUERY => reality_loop::autoresearch::attempts_history_query(args).await,
                    tool_names::ATTEMPTS_QUERY_REFLEXION => reality_loop::reflexion::attempts_query_reflexion(args).await,
                    tool_names::ATTEMPTS_CRITIQUE_SUMMARY => reality_loop::reflexion::attempts_critique_summary(args).await,
                    tool_names::ATTEMPTS_SUCCESS_STRATEGIES => reality_loop::reflexion::attempts_success_strategies(args).await,
                    tool_names::ATTEMPTS_SYNTHESIZE => reality_loop::reflexion::attempts_synthesize(args).await,
                    tool_names::EXPERIMENT_REGISTRY_PROPOSE => reality_loop::autoresearch::experiment_registry_propose(args).await,
                    tool_names::EXPERIMENT_REGISTRY_UPDATE_OUTCOME => reality_loop::autoresearch::experiment_registry_update_outcome(args).await,
                    tool_names::CHAMPION_STATE_PROMOTE => reality_loop::autoresearch::champion_state_promote(args).await,
                    _ => Err(reality_loop::CCRealityError::new(
                        "CCREALITY_LIGHTWEIGHT_TOOL_NOT_AVAILABLE",
                        "tool is not implemented by the lightweight cgreality stdio server",
                        "tools.call.name",
                        "use one of the reality_* / harness_* / optimizer_* tools or run the full default server for memory graph tools",
                        json!({"tool": name}),
                        None,
                    )),
                };
                match result {
                    Ok(value) => tool_success(id, value),
                    Err(err) => tool_error(id, err.into_value()),
                }
            }
            _ => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {"code": -32601, "message": format!("Method not found: {method}")}
            }),
        };
        write_jsonrpc_line(&mut writer, &response).await?;
    }
    Ok(())
}

fn tool_success(id: serde_json::Value, value: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "content": [{"type": "text", "text": serde_json::to_string(&value).unwrap_or_else(|_| "{}".to_string())}],
            "structuredContent": value,
            "isError": false
        }
    })
}

fn tool_error(id: serde_json::Value, value: serde_json::Value) -> serde_json::Value {
    tracing::error!(error = %value, "cgreality lightweight tool call failed");
    eprintln!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "event": "cgreality_tool_error",
            "error": value.clone()
        }))
        .unwrap_or_else(|_| {
            r#"{"event":"cgreality_tool_error","error":{"status":"error","error_code":"CCREALITY_ERROR_LOG_SERIALIZATION_FAILED"}}"#
                .to_string()
        })
    );
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "content": [{"type": "text", "text": serde_json::to_string(&value).unwrap_or_else(|_| "{}".to_string())}],
            "structuredContent": value,
            "isError": true
        }
    })
}

async fn write_jsonrpc_line<W: tokio::io::AsyncWrite + Unpin>(
    writer: &mut W,
    value: &serde_json::Value,
) -> Result<()> {
    use tokio::io::AsyncWriteExt;

    let line = serde_json::to_string(value)?;
    writer.write_all(line.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await?;
    Ok(())
}

async fn async_main() -> Result<()> {
    // Parse CLI arguments first (before logging init so --help works cleanly)
    let cli = CliArgs::parse()?;

    if cli.help {
        print_help();
        return Ok(());
    }
    if cli.version {
        eprintln!("context-graph-mcp {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    crate::tools::set_reality_loop_mode(cli.mode == "reality-loop");

    // Initialize logging - CRITICAL: Must write to stderr, not stdout!
    // MCP protocol requires stdout to be exclusively for JSON-RPC messages
    // Default to error-only to keep stderr clean for MCP clients
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("error"));

    fmt()
        .with_writer(io::stderr) // CRITICAL: stderr only!
        .with_env_filter(filter)
        .with_target(false) // Cleaner output for MCP
        .init();

    if cli.mode == "reality-loop"
        && cli.transport.as_deref().unwrap_or("stdio") == "stdio"
        && !cli.daemon
        && !cli.daemon_server_only
    {
        return run_reality_loop_stdio().await;
    }

    // ROOT-CAUSE FIX (zombie prevention): install kernel-level parent-death
    // tracking so if the MCP client dies for ANY reason (crash, kill -9,
    // uncaught exception, OOM), the kernel delivers SIGTERM to us and we
    // cannot outlive it. Combined with the existing SIGTERM handler below,
    // this guarantees no orphan MCP server ever holds the RocksDB flock.
    //
    // This is Linux-specific (PR_SET_PDEATHSIG) but the project already
    // targets x86_64-unknown-linux-gnu per rust-toolchain.toml.
    install_parent_death_signal();

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
    apply_overrides(&mut config, &cli)?;

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
    let daemon_port = determine_daemon_port(&cli)?;

    // SRV-M3 FIX: Resolve database path using the SAME function as McpServer::new().
    // Previously used `PathBuf::from(&config.storage.path)` which diverges from the
    // server's resolve_storage_path() when config path is empty or env var is set.
    // The guard prevents multiple processes from opening the same RocksDB,
    // which causes corruption if one is killed mid-compaction.
    let db_path = server::McpServer::resolve_storage_path(&config);
    let uses_rocksdb = config.storage.backend == "rocksdb";

    // ==================================================================
    // HEADLESS DAEMON MODE (--daemon-server-only)
    // This process IS the daemon, spawned by spawn_daemon_process().
    // Runs as a session leader (setsid) so it survives the proxy's death.
    // ==================================================================
    if cli.daemon_server_only {
        config.mcp.tcp_port = daemon_port;
        config.mcp.transport = "tcp".to_string();

        // Kill stale holders (both daemon and standalone)
        #[cfg(unix)]
        if uses_rocksdb {
            if !kill_stale_lock_holder(&db_path, daemon_port).await {
                debug!("kill_stale_lock_holder: no stale process found or no action taken");
            }
            if !kill_stale_standalone_holder(&db_path).await {
                debug!("kill_stale_standalone_holder: no stale process found or no action taken");
            }
            // Brief pause for kernel to release flock after kill
            tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
        }

        // Acquire PID file guard
        let _pid_guard = if uses_rocksdb {
            Some(PidFileGuard::acquire(&db_path)?)
        } else {
            None
        };

        // Create server
        let server = server::McpServer::new(config, warm_first).await?;

        // Start file watcher
        match server.start_file_watcher().await {
            Ok(true) => info!("File watcher started successfully"),
            Ok(false) => debug!("File watcher not started (disabled or models not ready)"),
            Err(e) => warn!("File watcher failed to start: {}", e),
        }

        info!(
            "Headless daemon server PID {} running on port {}",
            std::process::id(),
            daemon_port
        );

        // Run until signal
        let shutdown_signal = async {
            #[cfg(unix)]
            {
                let mut sigterm =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                        .expect("FATAL: failed to register SIGTERM handler in headless daemon");

                tokio::select! {
                    _ = sigterm.recv() => {
                        info!("Headless daemon received SIGTERM");
                    }
                    _ = tokio::signal::ctrl_c() => {
                        info!("Headless daemon received SIGINT");
                    }
                }
            }
            #[cfg(not(unix))]
            {
                tokio::signal::ctrl_c()
                    .await
                    .expect("FATAL: failed to register Ctrl+C handler");
                info!("Headless daemon received Ctrl+C");
            }
        };

        tokio::select! {
            result = server.run_tcp() => {
                if let Err(e) = result {
                    error!("Headless daemon TCP server crashed: {}", e);
                }
            }
            _ = shutdown_signal => {
                info!("Headless daemon shutting down...");
            }
        }

        // Shutdown with 10s force-exit deadline
        tokio::select! {
            _ = server.shutdown() => {
                info!("Headless daemon shutdown complete");
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(10)) => {
                error!(
                    "Headless daemon shutdown stuck (10s) — force exiting to release flock"
                );
                drop(_pid_guard);
                std::process::exit(1);
            }
        }

        info!("Headless daemon exiting");
        return Ok(());
    }

    if daemon_mode {
        // ==================================================================
        // DAEMON MODE: Share one MCP server across multiple Claude terminals
        // ==================================================================
        info!("Daemon mode enabled (port {})", daemon_port);

        let max_attempts = 5;
        let mut connected = false;
        for attempt in 1..=max_attempts {
            // ---- Step 1: Check for a healthy, running daemon ----
            if is_daemon_healthy(daemon_port).await {
                info!(
                    "Healthy daemon found on port {} (attempt {}), connecting as proxy",
                    daemon_port, attempt
                );
                run_stdio_to_tcp_proxy(daemon_port).await?;
                connected = true;
                break;
            }

            // ---- Step 2: Check for an unhealthy process hogging the port ----
            if is_port_in_use(daemon_port).await {
                warn!(
                    "Port {} is in use but daemon is unresponsive (attempt {}/{})",
                    daemon_port, attempt, max_attempts
                );
                #[cfg(unix)]
                if let Err(e) = kill_process_on_port(daemon_port).await {
                    warn!(
                        "Could not kill stuck process on port {}: {}",
                        daemon_port, e
                    );
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                continue;
            }

            // ---- Step 2.5: Kill stale lock holder if present ----
            // Port is free but a stale process may hold flock on mcp.pid,
            // blocking Step 3's PidFileGuard::acquire(). Kill it first.
            #[cfg(unix)]
            let _killed_stale = if uses_rocksdb {
                let killed_daemon = kill_stale_lock_holder(&db_path, daemon_port).await;
                let killed_standalone = kill_stale_standalone_holder(&db_path).await;
                if killed_daemon || killed_standalone {
                    // After SIGKILL, the kernel must reap the process and release
                    // the flock. On WSL2 this can take 500ms+. Wait with retries
                    // instead of racing PidFileGuard::acquire().
                    info!("Stale holder(s) killed, waiting for flock release...");
                    true
                } else {
                    false
                }
            } else {
                false
            };
            #[cfg(not(unix))]
            let killed_stale = false;

            // ---- Steps 3+4: Spawn daemon as separate OS process ----
            info!(
                "No daemon found, spawning daemon process (attempt {})...",
                attempt
            );
            if warm_first {
                warn!("Daemon mode with warm_first=true: models will load before serving requests");
            }

            #[cfg(unix)]
            {
                match spawn_daemon_process(daemon_port, cli.config_path.as_deref(), warm_first)
                    .await
                {
                    Ok(()) => {
                        info!(
                            "Daemon process started successfully on port {}",
                            daemon_port
                        );
                        run_stdio_to_tcp_proxy(daemon_port).await?;
                        connected = true;
                        break;
                    }
                    Err(e) => {
                        error!(
                            "Failed to spawn daemon (attempt {}/{}): {}",
                            attempt, max_attempts, e
                        );
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    }
                }
            }

            #[cfg(not(unix))]
            {
                // Non-Unix fallback: in-process daemon (original behavior)
                let pid_guard = if uses_rocksdb {
                    let mut guard_result = PidFileGuard::acquire(&db_path);
                    if guard_result.is_err() && killed_stale {
                        for retry in 1..=5 {
                            let delay_ms = 300 * retry;
                            info!(
                                "Waiting for flock release (retry {}/5, {}ms)...",
                                retry, delay_ms
                            );
                            tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                            guard_result = PidFileGuard::acquire(&db_path);
                            if guard_result.is_ok() {
                                break;
                            }
                        }
                    }
                    match guard_result {
                        Ok(guard) => Some(guard),
                        Err(e) => {
                            warn!(
                                "PID lock contention (attempt {}/{}): {}",
                                attempt, max_attempts, e
                            );
                            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                            continue;
                        }
                    }
                } else {
                    None
                };

                match start_daemon_server(config.clone(), warm_first, daemon_port, pid_guard).await
                {
                    Ok(()) => {
                        info!("Daemon started on port {}", daemon_port);
                        run_stdio_to_tcp_proxy(daemon_port).await?;
                        connected = true;
                        break;
                    }
                    Err(e) => {
                        error!(
                            "Failed to start daemon (attempt {}/{}): {}",
                            attempt, max_attempts, e
                        );
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                    }
                }
            }
        }

        if !connected {
            return Err(anyhow::anyhow!(
                "Failed to start or connect to daemon after {} attempts on port {}. \
                 Check logs above for details. Common causes:\n\
                 - Stale process holding RocksDB lock: kill $(cat {}/mcp.pid)\n\
                 - Port {} in use by another service: fuser {}/tcp\n\
                 - RocksDB corruption: rm -rf {}/LOCK",
                max_attempts,
                daemon_port,
                db_path.display(),
                daemon_port,
                daemon_port,
                db_path.display()
            ));
        }
    } else {
        // ==================================================================
        // STANDALONE MODE: Each terminal has its own MCP server (original behavior)
        // ==================================================================

        // Acquire PID file guard BEFORE opening RocksDB to prevent corruption
        // from multiple processes accessing the same database.
        // If a stale process (e.g., from a previous session with dead stdio)
        // holds the lock, kill it and retry.
        let _pid_guard = if uses_rocksdb {
            match PidFileGuard::acquire(&db_path) {
                Ok(guard) => Some(guard),
                Err(e) => {
                    #[cfg(unix)]
                    {
                        warn!("Lock contention: {} — attempting stale holder recovery", e);
                        if kill_stale_standalone_holder(&db_path).await {
                            // Retry with backoff for flock release
                            let mut guard_result = Err(e);
                            for retry in 1..=5 {
                                let delay_ms = 300 * retry;
                                info!(
                                    "Waiting for flock release (retry {}/5, {}ms)...",
                                    retry, delay_ms
                                );
                                tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms))
                                    .await;
                                match PidFileGuard::acquire(&db_path) {
                                    Ok(guard) => {
                                        guard_result = Ok(guard);
                                        break;
                                    }
                                    Err(e2) => guard_result = Err(e2),
                                }
                            }
                            match guard_result {
                                Ok(guard) => Some(guard),
                                Err(e) => {
                                    error!("FATAL: {}", e);
                                    return Err(e);
                                }
                            }
                        } else {
                            error!("FATAL: {}", e);
                            return Err(e);
                        }
                    }
                    #[cfg(not(unix))]
                    {
                        error!("FATAL: {}", e);
                        return Err(e);
                    }
                }
            }
        } else {
            None
        };

        // Create server with warmup configuration
        let server = server::McpServer::new(config, warm_first).await?;

        // Start file watcher if enabled in configuration
        match server.start_file_watcher().await {
            Ok(true) => info!("File watcher started successfully"),
            Ok(false) => debug!("File watcher not started (disabled or models not ready)"),
            Err(e) => warn!("File watcher failed to start: {}", e),
        }

        // Register signal handlers for graceful shutdown.
        // Without this, SIGTERM/SIGINT kills the process mid-operation,
        // interrupting RocksDB writes and HNSW persistence → corruption.
        let shutdown_signal = async {
            #[cfg(unix)]
            {
                let mut sigterm =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                        .expect("FATAL: failed to register SIGTERM handler");

                tokio::select! {
                    _ = sigterm.recv() => {
                        info!("Received SIGTERM — initiating graceful shutdown");
                        arm_doomsday_watchdog("SIGTERM");
                    }
                    _ = tokio::signal::ctrl_c() => {
                        info!("Received SIGINT (Ctrl+C) — initiating graceful shutdown");
                        arm_doomsday_watchdog("SIGINT");
                    }
                }
            }
            #[cfg(not(unix))]
            {
                tokio::signal::ctrl_c()
                    .await
                    .expect("FATAL: failed to register Ctrl+C handler");
                info!("Received Ctrl+C — initiating graceful shutdown");
                arm_doomsday_watchdog("Ctrl+C");
            }
        };

        // Run server with signal-aware shutdown.
        // When a signal arrives, we break out of the server loop and
        // fall through to shutdown() which awaits background tasks + flushes.
        match transport_mode {
            TransportMode::Stdio => {
                info!("MCP Server initialized, listening on stdio");
                tokio::select! {
                    result = server.run() => {
                        if let Err(e) = result {
                            error!("Server run() returned error: {}", e);
                        }
                        // stdin EOF (or read error) means the client is gone.
                        // Arm the doomsday watchdog so even a hung graceful
                        // shutdown cannot leave us as a zombie.
                        arm_doomsday_watchdog("stdio EOF");
                    }
                    _ = shutdown_signal => {
                        // Signal received — shutdown() called below (watchdog
                        // was armed inside shutdown_signal).
                    }
                }
            }
            TransportMode::Tcp => {
                info!("MCP Server initialized, starting TCP transport");
                tokio::select! {
                    result = server.run_tcp() => {
                        if let Err(e) = result {
                            error!("Server run_tcp() returned error: {}", e);
                        }
                    }
                    _ = shutdown_signal => {}
                }
            }
        }

        // Graceful shutdown with 10s force-exit deadline.
        // Catches stuck RocksDB compaction or HNSW flush that would hold
        // the flock forever, preventing new MCP servers from starting.
        tokio::select! {
            _ = server.shutdown() => {
                info!("Graceful shutdown complete");
            }
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(10)) => {
                error!(
                    "Shutdown stuck (10s deadline) — force exiting to release flock"
                );
                drop(_pid_guard);
                std::process::exit(1);
            }
        }

        // _pid_guard dropped here — releases flock + removes mcp.pid
    }

    info!("MCP Server shutdown complete");
    Ok(())
}
