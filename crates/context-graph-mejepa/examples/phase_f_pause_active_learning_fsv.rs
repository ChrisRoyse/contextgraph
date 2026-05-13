#[path = "../../../tests/e2e/pause_active_learning.rs"]
mod pause_active_learning;

use anyhow::Result;

fn main() -> Result<()> {
    let fsv_root = pause_active_learning::default_fsv_root();
    let report_path = pause_active_learning::default_report_path(&fsv_root);
    let report = pause_active_learning::run_pause_active_learning_fsv(&fsv_root)?;
    pause_active_learning::write_report_with_readback(&report, &report_path)?;
    println!("all_passed: {}", report.all_passed);
    println!("readback_equal: {}", report.readback_equal);
    println!("artifact_path: {}", report_path.display());
    println!("queue_entries_after: {}", report.queue_entries_after);
    Ok(())
}
