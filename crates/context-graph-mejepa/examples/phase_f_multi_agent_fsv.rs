#[path = "../../../tests/e2e/multi_agent.rs"]
mod multi_agent;

fn main() -> anyhow::Result<()> {
    let fsv_root = multi_agent::default_fsv_root();
    let report_path = multi_agent::default_report_path(&fsv_root);
    let report = multi_agent::run_multi_agent_fsv(&fsv_root)?;
    multi_agent::write_report_with_readback(&report, &report_path)?;
    println!("all_passed: {}", report.all_passed);
    println!("readback_equal: {}", report.readback_equal);
    println!("artifact_path: {}", report_path.display());
    println!(
        "live_predictions_after: {}",
        report
            .cf_counts_after
            .get(context_graph_mejepa_cf::CF_MEJEPA_LIVE_PREDICTIONS)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("MEJEPA_MULTI_AGENT_LIVE_COUNT_MISSING"))?
    );
    println!(
        "feedback_rows_after: {}",
        report
            .cf_counts_after
            .get(context_graph_mejepa_cf::CF_MEJEPA_AGENT_FEEDBACK)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("MEJEPA_MULTI_AGENT_FEEDBACK_COUNT_MISSING"))?
    );
    Ok(())
}
