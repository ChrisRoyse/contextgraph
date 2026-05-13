#[path = "../../../tests/e2e/restart_safety.rs"]
mod restart_safety;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let fsv_root = restart_safety::default_fsv_root();
    let report_path = restart_safety::default_report_path(&fsv_root);
    let report = restart_safety::run_restart_safety_fsv(&fsv_root).await?;
    restart_safety::write_report_with_readback(&report, &report_path)?;
    println!("all_passed: {}", report.all_passed);
    println!("readback_equal: {}", report.readback_equal);
    println!("artifact_path: {}", report_path.display());
    println!(
        "final_watermark_offset: {}",
        report.source_of_truth.final_watermark_offset
    );
    Ok(())
}
