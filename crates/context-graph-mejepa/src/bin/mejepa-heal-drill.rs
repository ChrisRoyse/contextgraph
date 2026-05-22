use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use context_graph_mejepa::heal::{run_heal_drill, HealDrillArgs, InjectDrift, READBACK_ROOT};

#[derive(Parser, Debug, Clone)]
#[command(name = "mejepa-heal-drill", about = "Phase 5 DoD verification harness")]
struct Cli {
    #[arg(long, value_enum)]
    inject_drift: InjectDrift,
    #[arg(long, default_value = READBACK_ROOT)]
    output_readback: PathBuf,
    #[arg(long, default_value_t = 5000)]
    max_observations: u64,
    #[arg(long, default_value_t = 42)]
    seed: u64,
    #[arg(long, default_value_t = 60)]
    rtx_5090_budget_min: u64,
}

fn main() -> ExitCode {
    let _ = tracing_subscriber::fmt::try_init();
    let cli = Cli::parse();
    match run_heal_drill(HealDrillArgs {
        inject_drift: cli.inject_drift,
        output_readback: cli.output_readback,
        max_observations: cli.max_observations,
        seed: cli.seed,
        rtx_5090_budget_min: cli.rtx_5090_budget_min,
    }) {
        Ok(summary) => {
            println!("{}", serde_json::to_string_pretty(&summary).unwrap());
            ExitCode::from(summary.exit_code as u8)
        }
        Err(err) => {
            err.log_context(file!());
            eprintln!("{}: {err}", err.code());
            ExitCode::from(1)
        }
    }
}
