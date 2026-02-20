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
mod monitoring;
mod protocol;
mod server;
mod tools;
mod transport;
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
    /// Daemon TCP port (--daemon-port, default: 3100)
    daemon_port: u16,
    /// Whether --daemon-port was explicitly passed on CLI
    explicit_daemon_port: bool,
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
            daemon_port: 3100, // Default daemon port (aligned with .mcp.json)
            explicit_daemon_port: false,
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
                    cli.no_warm = false; // Explicitly override --no-warm if both passed
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
                            cli.explicit_daemon_port = true;
                        }
                    }
                }
                arg => {
                    eprintln!("WARNING: Unknown argument '{}' — ignoring. Use --help for usage.", arg);
                }
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
    --daemon-port <PORT> Daemon TCP port (default: 3100)
    --help, -h           Show this help message

ENVIRONMENT VARIABLES:
    CONTEXT_GRAPH_TRANSPORT     Transport mode (stdio|tcp|sse)
    CONTEXT_GRAPH_TCP_PORT      TCP port number
    CONTEXT_GRAPH_SSE_PORT      SSE port number (TASK-42)
    CONTEXT_GRAPH_BIND_ADDRESS  TCP/SSE bind address
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
fn determine_daemon_port(cli: &CliArgs) -> u16 {
    // CLI --daemon-port takes highest priority
    if cli.explicit_daemon_port {
        return cli.daemon_port;
    }

    // Environment variable is second priority
    if let Ok(port_env) = env::var("CONTEXT_GRAPH_DAEMON_PORT") {
        if let Ok(port) = port_env.parse::<u16>() {
            return port;
        }
    }

    // Default: 3100 (aligned with .mcp.json convention)
    3100
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
                    let _ = f.read_to_string(&mut contents);
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
                                    pid, db_path.display()
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
                                .map_err(|e| anyhow::anyhow!("Cannot reopen PID file '{}': {}", pid_path.display(), e))?;
                            let fd = file.as_raw_fd();
                            let result = unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) };
                            if result == 0 {
                                let mut f = &file;
                                let _ = f.seek(io::SeekFrom::Start(0));
                                let _ = f.set_len(0);
                                let _ = write!(f, "{}", std::process::id());
                                let _ = f.flush();
                                info!(
                                    "Reclaimed stale PID lock (was process {}): new pid={}",
                                    pid, std::process::id()
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
                    pid_path.display(), errno
                ));
            }

            // We hold the lock — write our PID
            let mut f = &file;
            let _ = f.seek(io::SeekFrom::Start(0));
            let _ = f.set_len(0);
            let _ = write!(f, "{}", std::process::id());
            let _ = f.flush();

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
            // On non-Unix: best-effort PID file (no flock)
            use std::io::Write;
            let mut f = &file;
            let _ = f.set_len(0);
            let _ = write!(f, "{}", std::process::id());
            let _ = f.flush();

            warn!("PID file guard: no flock on this OS, using advisory PID file only");

            Ok(PidFileGuard {
                path: pid_path,
            })
        }
    }
}

impl Drop for PidFileGuard {
    fn drop(&mut self) {
        // Release flock (implicit on file close) and remove PID file
        let _ = fs::remove_file(&self.path);
        debug!("PID file guard released: '{}'", self.path.display());
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
            warn!("Health check: port {} responded but not valid JSON-RPC", port);
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
        .map_err(|e| anyhow::anyhow!(
            "fuser command failed: {}. Is fuser installed? (apt install psmisc)", e
        ))?;

    let pids_str = String::from_utf8_lossy(&output.stdout);
    let our_pid = std::process::id() as i32;

    for token in pids_str.split_whitespace() {
        let pid_str: String = token.chars().filter(|c| c.is_ascii_digit()).collect();
        if let Ok(pid) = pid_str.parse::<i32>() {
            if pid == our_pid || pid <= 1 {
                continue;
            }

            warn!("Sending SIGTERM to stuck daemon process PID {}", pid);
            unsafe { libc::kill(pid, libc::SIGTERM); }
            tokio::time::sleep(Duration::from_millis(200)).await;

            if unsafe { libc::kill(pid, 0) } == 0 {
                warn!("PID {} did not respond to SIGTERM, sending SIGKILL", pid);
                unsafe { libc::kill(pid, libc::SIGKILL); }
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
            warn!("PID file contains non-numeric value '{}' — removing stale file", pid_str);
            let _ = fs::remove_file(&pid_path);
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
        info!("PID {} is now serving on port {} — no kill needed", pid, daemon_port);
        return false;
    }

    // Process alive, NOT serving on daemon port — stale.
    warn!(
        "STALE LOCK DETECTED: PID {} holds flock on '{}' but is NOT serving on port {}. \
         Sending SIGTERM for graceful shutdown.",
        pid, pid_path.display(), daemon_port
    );

    // SIGTERM first — gives RocksDB time to flush
    unsafe { libc::kill(pid, libc::SIGTERM); }

    // Wait up to 2s for graceful shutdown
    for i in 0..20 {
        sleep(Duration::from_millis(100)).await;
        if unsafe { libc::kill(pid, 0) } != 0 {
            info!("Stale PID {} terminated after SIGTERM ({}ms)", pid, (i + 1) * 100);
            sleep(Duration::from_millis(200)).await;
            return true;
        }
    }

    // Still alive after 2s — force kill
    warn!("PID {} did not exit after SIGTERM (2s), sending SIGKILL", pid);
    unsafe { libc::kill(pid, libc::SIGKILL); }

    // Wait up to 1s for kernel to reap the process
    for i in 0..10 {
        sleep(Duration::from_millis(100)).await;
        if unsafe { libc::kill(pid, 0) } != 0 {
            info!("Stale PID {} terminated after SIGKILL ({}ms)", pid, (i + 1) * 100);
            sleep(Duration::from_millis(200)).await;
            return true;
        }
        // Check for zombie state — flock IS released even though kill(pid,0)==0
        let status_path = format!("/proc/{}/status", pid);
        let is_zombie = fs::read_to_string(&status_path)
            .map(|s| s.contains("State:\tZ") || s.contains("State:\tX"))
            .unwrap_or(false);
        if is_zombie {
            info!("PID {} is zombie after SIGKILL — flock released, lock reclaimable", pid);
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
            warn!("PID file contains non-numeric value '{}' — removing", pid_str);
            let _ = fs::remove_file(&pid_path);
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
        let cmdline = fs::read_to_string(format!("/proc/{}/cmdline", pid))
            .unwrap_or_default()
            .replace('\0', " ");
        let is_standalone = !cmdline.contains("--daemon");
        if is_standalone {
            info!(
                "PID {} is a standalone MCP server (stdin={}) — treating as stale",
                pid, fd0_str
            );
            true
        } else {
            false
        }
    } else {
        // fd/0 points to /dev/null or is unreadable — likely stale
        !fd0_str.is_empty()
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
        pid, pid_path.display()
    );

    unsafe { libc::kill(pid, libc::SIGTERM); }

    for i in 0..20 {
        sleep(Duration::from_millis(100)).await;
        if unsafe { libc::kill(pid, 0) } != 0 {
            info!("Stale PID {} terminated after SIGTERM ({}ms)", pid, (i + 1) * 100);
            sleep(Duration::from_millis(200)).await;
            return true;
        }
    }

    warn!("PID {} did not exit after SIGTERM (2s), sending SIGKILL", pid);
    unsafe { libc::kill(pid, libc::SIGKILL); }

    for i in 0..10 {
        sleep(Duration::from_millis(100)).await;
        if unsafe { libc::kill(pid, 0) } != 0 {
            info!("Stale PID {} terminated after SIGKILL ({}ms)", pid, (i + 1) * 100);
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
                        max_reconnects, daemon_port
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
                let mut sigterm = tokio::signal::unix::signal(
                    tokio::signal::unix::SignalKind::terminate(),
                )
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

#[tokio::main]
async fn main() -> Result<()> {
    // CRITICAL: MCP servers must be silent - set this BEFORE any config loading.
    // L2 NOTE: env::set_var is technically UB in multi-threaded context per POSIX.
    // This is the first statement before any async tasks spawn. When upgrading to
    // Rust 2024 edition, this must move to a pre-runtime init or use unsafe{}.
    // For now on edition 2021, this is safe in practice (no concurrent readers yet).
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

    // Resolve database path for PID file guard.
    // The guard prevents multiple processes from opening the same RocksDB,
    // which causes corruption if one is killed mid-compaction.
    let db_path = PathBuf::from(&config.storage.path);
    let uses_rocksdb = config.storage.backend == "rocksdb";

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
                    warn!("Could not kill stuck process on port {}: {}", daemon_port, e);
                }
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                continue;
            }

            // ---- Step 2.5: Kill stale lock holder if present ----
            // Port is free but a stale process may hold flock on mcp.pid,
            // blocking Step 3's PidFileGuard::acquire(). Kill it first.
            #[cfg(unix)]
            let killed_stale = if uses_rocksdb {
                if kill_stale_lock_holder(&db_path, daemon_port).await {
                    // After SIGKILL, the kernel must reap the process and release
                    // the flock. On WSL2 this can take 500ms+. Wait with retries
                    // instead of racing PidFileGuard::acquire().
                    info!("Stale lock holder killed, waiting for flock release...");
                    true
                } else {
                    false
                }
            } else {
                false
            };
            #[cfg(not(unix))]
            let killed_stale = false;

            // ---- Step 3: Port is free — try to acquire PID lock and start daemon ----
            let pid_guard = if uses_rocksdb {
                let mut guard_result = PidFileGuard::acquire(&db_path);
                // If we just killed a stale holder, the flock may not be released
                // yet (kernel reap delay, especially on WSL2). Retry with backoff.
                if guard_result.is_err() && killed_stale {
                    for retry in 1..=5 {
                        let delay_ms = 300 * retry;
                        info!("Waiting for flock release (retry {}/5, {}ms)...", retry, delay_ms);
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

            // ---- Step 4: Start daemon server (guard ownership transfers) ----
            info!("No daemon found, starting new daemon server (attempt {})...", attempt);
            if warm_first {
                warn!("Daemon mode with warm_first=true: models will load before serving requests");
            }

            match start_daemon_server(config.clone(), warm_first, daemon_port, pid_guard).await {
                Ok(()) => {
                    info!("Daemon started successfully on port {}", daemon_port);
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

        if !connected {
            return Err(anyhow::anyhow!(
                "Failed to start or connect to daemon after {} attempts on port {}. \
                 Check logs above for details. Common causes:\n\
                 - Stale process holding RocksDB lock: kill $(cat {}/mcp.pid)\n\
                 - Port {} in use by another service: fuser {}/tcp\n\
                 - RocksDB corruption: rm -rf {}/LOCK",
                max_attempts, daemon_port,
                db_path.display(), daemon_port, daemon_port, db_path.display()
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
                                info!("Waiting for flock release (retry {}/5, {}ms)...", retry, delay_ms);
                                tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
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
                let mut sigterm = tokio::signal::unix::signal(
                    tokio::signal::unix::SignalKind::terminate(),
                )
                .expect("FATAL: failed to register SIGTERM handler");

                tokio::select! {
                    _ = sigterm.recv() => {
                        info!("Received SIGTERM — initiating graceful shutdown");
                    }
                    _ = tokio::signal::ctrl_c() => {
                        info!("Received SIGINT (Ctrl+C) — initiating graceful shutdown");
                    }
                }
            }
            #[cfg(not(unix))]
            {
                tokio::signal::ctrl_c()
                    .await
                    .expect("FATAL: failed to register Ctrl+C handler");
                info!("Received Ctrl+C — initiating graceful shutdown");
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
                    }
                    _ = shutdown_signal => {
                        // Signal received — shutdown() called below
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
            TransportMode::Sse => {
                info!("MCP Server initialized, starting SSE transport");
                tokio::select! {
                    result = server.run_sse() => {
                        if let Err(e) = result {
                            error!("Server run_sse() returned error: {}", e);
                        }
                    }
                    _ = shutdown_signal => {}
                }
            }
        }

        // Graceful shutdown: await background tasks, persist HNSW, flush RocksDB
        server.shutdown().await;

        // _pid_guard dropped here — releases flock + removes mcp.pid
    }

    info!("MCP Server shutdown complete");
    Ok(())
}
