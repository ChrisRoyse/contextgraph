use crate::types::SurpriseSeverity;

pub fn surprise_weight_from_score(severity_score: f32) -> u8 {
    if severity_score >= SurpriseSeverity::Catastrophic.severity_score() {
        4
    } else if severity_score >= SurpriseSeverity::High.severity_score() {
        2
    } else {
        1
    }
}

pub fn surprise_weight_from_reason(reason: &str) -> u8 {
    let lower = reason.to_ascii_lowercase();
    if lower.contains("catastrophic") {
        4
    } else if lower.contains("agent_surprise") || lower.contains("surprise") {
        2
    } else {
        1
    }
}
