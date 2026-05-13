#[path = "../../../tests/e2e/soak.rs"]
mod soak;

fn main() -> anyhow::Result<()> {
    let fsv_root = soak::default_fsv_root();
    let report_path = soak::default_report_path(&fsv_root);
    let report = soak::run_soak_fsv(&fsv_root)?;
    soak::write_report_with_readback(&report, &report_path)?;
    println!("all_passed: {}", report.all_passed);
    println!("readback_equal: {}", report.readback_equal);
    println!("artifact_path: {}", report_path.display());
    println!("cycle_count: {}", report.cycle_summaries.len());
    println!(
        "max_prediction_p99_ms: {}",
        report
            .cycle_summaries
            .iter()
            .map(|summary| summary.prediction_p99_ms)
            .max()
            .ok_or_else(|| anyhow::anyhow!("MEJEPA_SOAK_EMPTY_CYCLES"))?
    );
    Ok(())
}
