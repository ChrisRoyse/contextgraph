#[path = "../../../tests/e2e/heal_ticker_telemetry.rs"]
mod heal_ticker_telemetry;

fn main() -> anyhow::Result<()> {
    let fsv_root = heal_ticker_telemetry::default_fsv_root();
    let report_path = heal_ticker_telemetry::default_report_path(&fsv_root);
    let report = heal_ticker_telemetry::run_heal_ticker_telemetry_fsv(&fsv_root)?;
    heal_ticker_telemetry::write_report_with_readback(&report, &report_path)?;
    println!("all_passed: {}", report.all_passed);
    println!("readback_equal: {}", report.readback_equal);
    println!("artifact_path: {}", report_path.display());
    Ok(())
}
