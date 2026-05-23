use clap::{Parser, Subcommand};
use context_graph_mejepa_train::cli;

#[derive(Parser)]
#[command(
    name = "mejepa-train",
    version,
    about = "ME-JEPA Phase 3 Training Loop CLI"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Train(Box<cli::TrainArgs>),
    VerifyChain(cli::VerifyChainArgs),
    VerifyCheckpoint(cli::VerifyCheckpointArgs),
    EvalHoldout(cli::EvalHoldoutArgs),
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    match cli.command {
        Command::Train(args) => cli::run_train(*args),
        Command::VerifyChain(args) => cli::run_verify_chain(args),
        Command::VerifyCheckpoint(args) => cli::run_verify_checkpoint(args),
        Command::EvalHoldout(args) => cli::run_eval_holdout(args),
    }
}
