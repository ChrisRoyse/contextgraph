use crate::error::TrainerError;
use crate::online_head_state_support::validate_id;
use sha2::{Digest, Sha256};

pub fn online_head_key(panel_signature_hash: &str) -> Result<String, TrainerError> {
    validate_id("panel_signature_hash", panel_signature_hash)?;
    Ok(format!("online_head:{panel_signature_hash}"))
}

pub fn repeat_metric_key(
    replay_cell_id: &str,
    label_signature_hash: &str,
    skill_signature_hash: Option<&str>,
    ability_signature_hash: Option<&str>,
    membership_signature_hash: Option<&str>,
) -> Result<String, TrainerError> {
    validate_id("replay_cell_id", replay_cell_id)?;
    validate_id("label_signature_hash", label_signature_hash)?;
    for (field, value) in [
        ("skill_signature_hash", skill_signature_hash),
        ("ability_signature_hash", ability_signature_hash),
        ("membership_signature_hash", membership_signature_hash),
    ] {
        if let Some(value) = value {
            validate_id(field, value)?;
        }
    }
    let mut hasher = Sha256::new();
    for value in [
        Some(replay_cell_id),
        Some(label_signature_hash),
        skill_signature_hash,
        ability_signature_hash,
        membership_signature_hash,
    ] {
        if let Some(value) = value {
            hasher.update(value.as_bytes());
        }
        hasher.update([0]);
    }
    Ok(format!(
        "mistake_repeat:{}",
        &hex::encode(hasher.finalize())[..24]
    ))
}

pub fn unrelated_control_panel_signature_hash(
    panel_signature_hash: &str,
    replay_cell_id: &str,
) -> Result<String, TrainerError> {
    validate_id("panel_signature_hash", panel_signature_hash)?;
    validate_id("replay_cell_id", replay_cell_id)?;
    let mut hasher = Sha256::new();
    hasher.update(b"online-head-unrelated-control");
    hasher.update([0]);
    hasher.update(panel_signature_hash.as_bytes());
    hasher.update([0]);
    hasher.update(replay_cell_id.as_bytes());
    Ok(format!(
        "control_panel:{}",
        &hex::encode(hasher.finalize())[..24]
    ))
}
