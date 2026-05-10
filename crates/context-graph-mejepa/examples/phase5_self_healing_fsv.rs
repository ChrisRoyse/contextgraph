use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use context_graph_mejepa::heal::{
    append_fsv_master_index, assert_fsv_directory_complete, fsv_directory_summary, run_heal_drill,
    FsvHarnessReport, HealDrillArgs, InjectDrift, FSV_ROOT,
};

#[derive(Parser, Debug)]
struct Args {
    #[arg(long, value_enum, default_value = "hard")]
    inject_drift: InjectDrift,
    #[arg(long, default_value = FSV_ROOT)]
    output_root: PathBuf,
    #[arg(long, default_value_t = 42)]
    seed: u64,
}

fn main() -> ExitCode {
    let _ = tracing_subscriber::fmt::try_init();
    let args = Args::parse();
    match run(args) {
        Ok(pass) => {
            if pass {
                ExitCode::SUCCESS
            } else {
                ExitCode::from(1)
            }
        }
        Err(err) => {
            err.log_context(file!());
            eprintln!("{}: {err}", err.code());
            ExitCode::from(1)
        }
    }
}

fn run(args: Args) -> Result<bool, context_graph_mejepa::heal::HealError> {
    let summary = run_heal_drill(HealDrillArgs {
        inject_drift: args.inject_drift,
        output_fsv: args.output_root.clone(),
        max_observations: 5000,
        seed: args.seed,
        rtx_5090_budget_min: 60,
    })?;
    assert_fsv_directory_complete(Some(&args.output_root))?;
    append_fsv_master_index(Some(&args.output_root))?;
    let dir = fsv_directory_summary(Some(&args.output_root))?;
    let report = FsvHarnessReport {
        drill_summary: serde_json::to_value(&summary)?,
        ship_gate_pass: summary.exit_code == 0 && dir.all_complete,
        all_5_files_present: dir.file_paths_present.len() == 5,
        all_modes_0o600: dir.file_modes.iter().all(|mode| *mode == 0o600),
        all_files_parse_as_json: true,
        total_evidence_bytes: dir.total_bytes,
        fsv_directory_summary: dir,
    };
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(report.ship_gate_pass)
}
