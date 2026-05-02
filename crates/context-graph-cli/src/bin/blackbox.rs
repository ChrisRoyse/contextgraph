use clap::Parser;
use context_graph_cli::commands::blackbox::{handle_blackbox_command, BlackboxCli};
use tracing_subscriber::{fmt, EnvFilter};

#[tokio::main]
async fn main() {
    let cli = BlackboxCli::parse();
    let filter = match cli.verbose {
        0 => EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn")),
        1 => EnvFilter::new("info"),
        2 => EnvFilter::new("debug"),
        _ => EnvFilter::new("trace"),
    };

    fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_level(true)
        .with_writer(std::io::stderr)
        .init();

    let exit_code = handle_blackbox_command(cli.command).await;
    std::process::exit(exit_code);
}
