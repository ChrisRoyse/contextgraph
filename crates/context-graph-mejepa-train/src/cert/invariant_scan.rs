use crate::cert::chain::cf;
use crate::cert::TrainingCertificate;
use crate::error::TrainerError;
use rocksdb::DB;

pub fn scan_target_collapse_invariant(
    rocksdb: &DB,
    cf_name: &str,
    from: u64,
    to: u64,
) -> Result<Vec<u64>, TrainerError> {
    let cf = cf(rocksdb, cf_name)?;
    let mut violations = Vec::new();
    for step in from..=to {
        if let Some(bytes) = rocksdb.get_cf(cf, step.to_be_bytes())? {
            let cert: TrainingCertificate = serde_json::from_slice(&bytes)?;
            if cert.signal.delta_xi_components.target_collapse != 0.0 {
                violations.push(step);
            }
        }
    }
    Ok(violations)
}
