#[path = "../../../tests/e2e/catastrophic_event.rs"]
mod catastrophic_event;

use anyhow::Result;

fn main() -> Result<()> {
    let fsv_root = catastrophic_event::default_fsv_root();
    let report_path = catastrophic_event::default_report_path(&fsv_root);
    let report = catastrophic_event::run_catastrophic_event_fsv(&fsv_root)?;
    catastrophic_event::write_report_with_readback(&report, &report_path)?;
    println!("all_passed: {}", report.all_passed);
    println!("readback_equal: {}", report.readback_equal);
    println!("artifact_path: {}", report_path.display());
    println!(
        "heal_reports_after: {}",
        report
            .cf_counts_after
            .get(context_graph_mejepa_cf::CF_MEJEPA_HEAL_REPORTS)
            .copied()
            .unwrap_or_default()
    );
    Ok(())
}
