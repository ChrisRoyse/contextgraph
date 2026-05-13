#[path = "../../../tests/e2e/harness.rs"]
mod harness;

use std::path::PathBuf;
use std::process::ExitCode;

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/phase-f-e2e-happy-path-fsv";

fn main() -> ExitCode {
    let root = PathBuf::from(FSV_ROOT);
    match harness::run_e2e_happy_path_fsv(&root).and_then(|report| {
        let evidence_path = root.join("e2e_happy_path_fsv.json");
        harness::write_report_with_readback(&report, &evidence_path)?;
        println!("{}", serde_json::to_string_pretty(&report)?);
        Ok(())
    }) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err:#}");
            ExitCode::from(1)
        }
    }
}
