// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use serde::{Deserialize, Serialize};

use crate::error::{OpsError, OpsResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Frozen,
    Cold,
    Cool,
    Warm,
    Hot,
}

impl Tier {
    pub fn is_lossy(self) -> bool {
        !matches!(self, Self::Hot)
    }
}

pub fn tier_for_score(score: f32) -> OpsResult<Tier> {
    if !score.is_finite() || !(0.0..=1.0).contains(&score) {
        return Err(OpsError::invalid(
            "access_frequency.score",
            format!("expected finite score in [0,1], got {score}"),
        ));
    }
    Ok(if score >= 0.8 {
        Tier::Hot
    } else if score >= 0.6 {
        Tier::Warm
    } else if score >= 0.4 {
        Tier::Cool
    } else if score >= 0.2 {
        Tier::Cold
    } else {
        Tier::Frozen
    })
}

pub fn decayed_score(score: f32, last_read_unix: i64, now_unix: i64) -> OpsResult<f32> {
    if now_unix < last_read_unix {
        return Err(OpsError::invalid(
            "access_frequency.last_read_unix",
            format!("now_unix {now_unix} is before last_read_unix {last_read_unix}"),
        ));
    }
    if !score.is_finite() || !(0.0..=1.0).contains(&score) {
        return Err(OpsError::invalid(
            "access_frequency.score",
            format!("expected finite score in [0,1], got {score}"),
        ));
    }
    let elapsed_days = (now_unix - last_read_unix) as f32 / 86_400.0;
    Ok((score * 0.99_f32.powf(elapsed_days)).clamp(0.0, 1.0))
}

pub fn score_after_read(score: f32) -> OpsResult<f32> {
    if !score.is_finite() || !(0.0..=1.0).contains(&score) {
        return Err(OpsError::invalid(
            "access_frequency.score",
            format!("expected finite score in [0,1], got {score}"),
        ));
    }
    Ok((score + 0.1).min(1.0))
}

pub fn encode_for_tier(values: &[f32], tier: Tier) -> OpsResult<Vec<u8>> {
    match tier {
        Tier::Hot => raw_f32_bytes(values),
        Tier::Warm => encode_cgtc(values, context_graph_tier_compression::BitWidth::Seven),
        Tier::Cool => encode_cgtc(values, context_graph_tier_compression::BitWidth::Five),
        Tier::Cold => encode_cgtc(values, context_graph_tier_compression::BitWidth::Three),
        Tier::Frozen => crate::tier_frozen::encode_frozen(values),
    }
}

pub fn decode_from_tier(bytes: &[u8], tier: Tier) -> OpsResult<Vec<f32>> {
    match tier {
        Tier::Hot => raw_f32_values(bytes),
        Tier::Warm | Tier::Cool | Tier::Cold => {
            let blob = context_graph_tier_compression::deserialize(bytes)?;
            let expected = compressed_bits_for_tier(tier).ok_or_else(|| {
                OpsError::invalid("tier", format!("tier {tier:?} is not a CGTC tier"))
            })?;
            if blob.bits != expected {
                return Err(OpsError::invalid(
                    "tier_blob.bits",
                    format!(
                        "metadata tier {tier:?} expects {:?}, blob contains {:?}",
                        expected, blob.bits
                    ),
                ));
            }
            Ok(context_graph_tier_compression::decode(&blob)?)
        }
        Tier::Frozen => crate::tier_frozen::decode_frozen(bytes),
    }
}

fn compressed_bits_for_tier(tier: Tier) -> Option<context_graph_tier_compression::BitWidth> {
    match tier {
        Tier::Warm => Some(context_graph_tier_compression::BitWidth::Seven),
        Tier::Cool => Some(context_graph_tier_compression::BitWidth::Five),
        Tier::Cold => Some(context_graph_tier_compression::BitWidth::Three),
        Tier::Hot | Tier::Frozen => None,
    }
}

fn raw_f32_bytes(values: &[f32]) -> OpsResult<Vec<u8>> {
    if values.is_empty() {
        return Err(OpsError::invalid("values", "cannot encode an empty vector"));
    }
    let mut out = Vec::with_capacity(values.len() * 4);
    for (idx, value) in values.iter().enumerate() {
        if !value.is_finite() {
            return Err(OpsError::invalid(
                "values",
                format!("values[{idx}] is non-finite: {value}"),
            ));
        }
        out.extend_from_slice(&value.to_le_bytes());
    }
    Ok(out)
}

fn raw_f32_values(bytes: &[u8]) -> OpsResult<Vec<f32>> {
    if bytes.is_empty() || !bytes.len().is_multiple_of(4) {
        return Err(OpsError::invalid(
            "hot_blob",
            format!(
                "hot tier blob length must be non-zero multiple of 4, got {}",
                bytes.len()
            ),
        ));
    }
    let mut out = Vec::with_capacity(bytes.len() / 4);
    for chunk in bytes.chunks_exact(4) {
        let mut raw = [0u8; 4];
        raw.copy_from_slice(chunk);
        let value = f32::from_le_bytes(raw);
        if !value.is_finite() {
            return Err(OpsError::invalid(
                "hot_blob",
                "hot tier blob contains NaN/Inf",
            ));
        }
        out.push(value);
    }
    Ok(out)
}

fn encode_cgtc(
    values: &[f32],
    bits: context_graph_tier_compression::BitWidth,
) -> OpsResult<Vec<u8>> {
    let blob = context_graph_tier_compression::encode(values, bits)?;
    Ok(context_graph_tier_compression::serialize(&blob)?)
}
