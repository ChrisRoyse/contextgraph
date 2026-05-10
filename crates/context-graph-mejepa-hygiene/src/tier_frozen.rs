// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use crate::error::{OpsError, OpsResult};

const MAGIC: [u8; 4] = *b"CGTF";
const VERSION: u8 = 1;
const HEADER_LEN: usize = 22;

pub fn encode_frozen(values: &[f32]) -> OpsResult<Vec<u8>> {
    if values.is_empty() {
        return Err(OpsError::invalid(
            "values",
            "cannot encode an empty frozen vector",
        ));
    }
    let mut abs_sum = 0.0f32;
    let mut positives = 0u32;
    for (idx, value) in values.iter().enumerate() {
        if !value.is_finite() {
            return Err(OpsError::invalid(
                "values",
                format!("values[{idx}] is non-finite: {value}"),
            ));
        }
        if *value >= 0.0 {
            positives = positives.saturating_add(1);
        }
        abs_sum += value.abs();
    }
    let scale = abs_sum / values.len() as f32;
    if !scale.is_finite() || scale <= 0.0 {
        return Err(OpsError::invalid(
            "values",
            "frozen tier sign-only encoding rejects all-zero/degenerate vectors",
        ));
    }
    let payload_len = values.len().div_ceil(8);
    let mut out = Vec::with_capacity(HEADER_LEN + payload_len);
    out.extend_from_slice(&MAGIC);
    out.push(VERSION);
    out.push(1);
    out.extend_from_slice(&(values.len() as u64).to_le_bytes());
    out.extend_from_slice(&scale.to_le_bytes());
    out.extend_from_slice(&positives.to_le_bytes());
    let mut byte = 0u8;
    for (idx, value) in values.iter().enumerate() {
        if *value >= 0.0 {
            byte |= 1 << (idx % 8);
        }
        if idx % 8 == 7 {
            out.push(byte);
            byte = 0;
        }
    }
    if !values.len().is_multiple_of(8) {
        out.push(byte);
    }
    Ok(out)
}

pub fn decode_frozen(bytes: &[u8]) -> OpsResult<Vec<f32>> {
    if bytes.len() < HEADER_LEN {
        return Err(OpsError::invalid(
            "frozen_blob",
            format!(
                "CGTF header requires {HEADER_LEN} bytes, got {}",
                bytes.len()
            ),
        ));
    }
    if bytes[..4] != MAGIC {
        return Err(OpsError::invalid("frozen_blob.magic", "wrong CGTF magic"));
    }
    if bytes[4] != VERSION {
        return Err(OpsError::invalid(
            "frozen_blob.version",
            format!("unsupported CGTF version {}", bytes[4]),
        ));
    }
    if bytes[5] != 1 {
        return Err(OpsError::invalid(
            "frozen_blob.bits",
            "CGTF bit width must be 1",
        ));
    }
    let mut n_raw = [0u8; 8];
    n_raw.copy_from_slice(&bytes[6..14]);
    let n_u64 = u64::from_le_bytes(n_raw);
    let n = usize::try_from(n_u64).map_err(|_| {
        OpsError::invalid(
            "frozen_blob.n",
            format!("n {n_u64} does not fit in usize on this platform"),
        )
    })?;
    if n == 0 {
        return Err(OpsError::invalid("frozen_blob.n", "n must be > 0"));
    }
    let mut scale_raw = [0u8; 4];
    scale_raw.copy_from_slice(&bytes[14..18]);
    let scale = f32::from_le_bytes(scale_raw);
    if !scale.is_finite() || scale <= 0.0 {
        return Err(OpsError::invalid(
            "frozen_blob.scale",
            format!("scale must be finite and > 0, got {scale}"),
        ));
    }
    let mut positives_raw = [0u8; 4];
    positives_raw.copy_from_slice(&bytes[18..22]);
    let positives = u32::from_le_bytes(positives_raw);
    if positives as usize > n {
        return Err(OpsError::invalid(
            "frozen_blob.positive_count",
            "positive count exceeds n",
        ));
    }
    let payload = &bytes[HEADER_LEN..];
    let expected = n.div_ceil(8);
    if payload.len() != expected {
        return Err(OpsError::invalid(
            "frozen_blob.payload",
            format!(
                "payload length mismatch: got {}, expected {expected}",
                payload.len()
            ),
        ));
    }
    let mut out = Vec::with_capacity(n);
    let mut seen_pos = 0u32;
    for idx in 0..n {
        let sign = (payload[idx / 8] >> (idx % 8)) & 1;
        if sign == 1 {
            seen_pos += 1;
            out.push(scale);
        } else {
            out.push(-scale);
        }
    }
    if seen_pos != positives {
        return Err(OpsError::invalid(
            "frozen_blob.positive_count",
            format!("header says {positives}, payload has {seen_pos}"),
        ));
    }
    Ok(out)
}
