#[path = "../../../tests/e2e/adversarial_corpus.rs"]
mod adversarial_corpus;

use anyhow::Result;

fn main() -> Result<()> {
    let fsv_root = adversarial_corpus::default_fsv_root();
    let report_path = adversarial_corpus::default_report_path(&fsv_root);
    let report = adversarial_corpus::run_adversarial_corpus_fsv(&fsv_root)?;
    adversarial_corpus::write_report_with_readback(&report, &report_path)?;
    println!("all_passed: {}", report.all_passed);
    println!("readback_equal: {}", report.readback_equal);
    println!("artifact_path: {}", report_path.display());
    println!("non_pass_rate: {:.3}", report.aggregate.non_pass_rate);
    println!(
        "high_confidence_false_passes: {}",
        report.aggregate.high_confidence_false_passes
    );
    Ok(())
}
