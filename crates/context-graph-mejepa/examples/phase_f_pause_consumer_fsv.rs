#[path = "../../../tests/e2e/pause_consumer.rs"]
mod pause_consumer;

use anyhow::Result;

fn main() -> Result<()> {
    let fsv_root = pause_consumer::default_fsv_root();
    let report_path = pause_consumer::default_report_path(&fsv_root);
    let report = pause_consumer::run_pause_consumer_fsv(&fsv_root)?;
    pause_consumer::write_report_with_readback(&report, &report_path)?;
    println!("all_passed: {}", report.all_passed);
    println!("readback_equal: {}", report.readback_equal);
    println!("artifact_path: {}", report_path.display());
    println!(
        "live_prediction_rows_after: {}",
        report
            .cf_counts_after
            .get(context_graph_mejepa::CF_MEJEPA_LIVE_PREDICTIONS)
            .copied()
            .unwrap_or(0)
    );
    Ok(())
}
