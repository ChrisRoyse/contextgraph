use crate::{InstrumentError, InstrumentResult};

pub(crate) fn validate_text_field(
    field: &'static str,
    value: &str,
    remediation: &'static str,
) -> InstrumentResult<()> {
    if value.trim().is_empty() {
        return Err(InstrumentError::invalid(
            field,
            format!("{field} is empty or whitespace-only"),
            remediation,
        ));
    }
    if value.chars().any(|ch| ch == '\0') {
        return Err(InstrumentError::invalid(
            field,
            format!("{field} contains a NUL byte"),
            remediation,
        ));
    }
    Ok(())
}

pub(crate) fn validate_single_line(
    field: &'static str,
    value: &str,
    remediation: &'static str,
) -> InstrumentResult<()> {
    validate_text_field(field, value, remediation)?;
    if value.chars().any(char::is_control) {
        return Err(InstrumentError::invalid(
            field,
            format!("{field} contains a control character"),
            remediation,
        ));
    }
    Ok(())
}

pub(crate) fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash
}

pub(crate) fn hash_bin(value: &str, bins: usize) -> usize {
    (fnv1a64(value.as_bytes()) as usize) % bins
}

pub(crate) fn add_hashed_token_features(out: &mut [f32], text: &str, weight: f32) {
    for token in text.split(|ch: char| !ch.is_alphanumeric() && ch != '_') {
        if token.is_empty() {
            continue;
        }
        let hash = fnv1a64(token.as_bytes());
        let idx = (hash as usize) % out.len();
        let sign = if hash & (1 << 63) == 0 { 1.0 } else { -1.0 };
        out[idx] += sign * weight;
    }
}

pub(crate) fn add_hashed_pair(out: &mut [f32], key: &str, value: f32, offset: usize, span: usize) {
    if span == 0 {
        return;
    }
    let idx = offset + hash_bin(key, span);
    if idx < out.len() {
        out[idx] += value;
    }
}

pub(crate) fn bounded_ratio(value: f32, denom: f32) -> f32 {
    if denom <= 0.0 {
        0.0
    } else {
        (value / denom).clamp(0.0, 1.0)
    }
}

pub(crate) fn normalize_l2(out: &mut [f32]) {
    let norm = out
        .iter()
        .map(|v| (*v as f64) * (*v as f64))
        .sum::<f64>()
        .sqrt();
    if norm > 0.0 {
        for value in out {
            *value = (*value as f64 / norm) as f32;
        }
    }
}

pub(crate) fn validate_finite_output(field: &'static str, out: &[f32]) -> InstrumentResult<()> {
    for (idx, value) in out.iter().enumerate() {
        if !value.is_finite() {
            return Err(InstrumentError::invariant(
                field,
                format!("output[{idx}] is non-finite: {value}"),
                "inspect instrument aggregation math and reject invalid source inputs before encoding",
            ));
        }
    }
    Ok(())
}
