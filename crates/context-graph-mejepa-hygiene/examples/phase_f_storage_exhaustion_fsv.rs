#[path = "../../../tests/e2e/storage_exhaustion.rs"]
mod storage_exhaustion;

use anyhow::Result;

fn main() -> Result<()> {
    let fsv_root = storage_exhaustion::default_fsv_root();
    let report_path = storage_exhaustion::default_report_path(&fsv_root);
    let report = storage_exhaustion::run_storage_exhaustion_fsv(&fsv_root)?;
    storage_exhaustion::write_report_with_readback(&report, &report_path)?;
    println!("all_passed: {}", report.all_passed);
    println!("readback_equal: {}", report.readback_equal);
    println!("artifact_path: {}", report_path.display());
    println!(
        "shift_rows_after: {}",
        report
            .cf_counts_after
            .get(context_graph_mejepa_cf::CF_MEJEPA_SHIFT_WATERMARK)
            .map(|count| count.rows)
            .unwrap_or_default()
    );
    Ok(())
}
