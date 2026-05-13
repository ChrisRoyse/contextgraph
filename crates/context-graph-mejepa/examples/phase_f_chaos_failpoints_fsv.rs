#[path = "../../../tests/e2e/chaos_failpoints.rs"]
mod chaos_failpoints;

fn main() -> anyhow::Result<()> {
    let fsv_root = chaos_failpoints::default_fsv_root();
    let report_path = chaos_failpoints::default_report_path(&fsv_root);
    let report = chaos_failpoints::run_chaos_failpoints_fsv(&fsv_root)?;
    chaos_failpoints::write_report_with_readback(&report, &report_path)?;
    println!("all_passed: {}", report.all_passed);
    println!("readback_equal: {}", report.readback_equal);
    println!("artifact_path: {}", report_path.display());
    println!(
        "live_predictions_after: {}",
        report
            .cf_counts_after
            .get(context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("MEJEPA_CHAOS_LIVE_COUNT_MISSING"))?
    );
    println!(
        "feedback_rows_after: {}",
        report
            .cf_counts_after
            .get(context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("MEJEPA_CHAOS_FEEDBACK_COUNT_MISSING"))?
    );
    Ok(())
}
