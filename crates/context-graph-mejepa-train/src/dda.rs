use crate::{TrainerError, TrainerErrorCode};
use context_graph_mejepa::types::{ChunkId, DdaSignals, PanelId};
use context_graph_mejepa_cf::CF_MEJEPA_DDA_SIGNALS;
use rocksdb::{IteratorMode, WriteOptions, DB};
use serde_json::json;
use sha2::{Digest, Sha256};

pub const DDA_PAIRWISE_PROJECTION_DIM: usize = 128;
pub const DDA_PAIRWISE_PROJECTION_SCHEMA: &str = "signed-hash-projection-v1";

pub type DdaResult<T> = Result<T, TrainerError>;

#[derive(Debug, Clone, PartialEq)]
pub struct DdaVectorInput {
    pub embedder_id: String,
    pub vector: Vec<f32>,
    pub centroid: Vec<f32>,
}

impl DdaVectorInput {
    pub fn validate(&self, index: usize) -> DdaResult<()> {
        if self.embedder_id.trim().is_empty() {
            return Err(invalid_dda_input(
                "dda.inputs.embedder_id",
                format!("embedder_id at index {index} is empty"),
                json!({ "index": index }),
            ));
        }
        if self.vector.is_empty() {
            return Err(invalid_dda_input(
                "dda.inputs.vector",
                format!("vector for {} is empty", self.embedder_id),
                json!({ "embedder_id": self.embedder_id }),
            ));
        }
        if self.vector.len() != self.centroid.len() {
            return Err(invalid_dda_input(
                "dda.inputs.centroid",
                format!(
                    "vector/centroid dimension mismatch for {}: {} vs {}",
                    self.embedder_id,
                    self.vector.len(),
                    self.centroid.len()
                ),
                json!({
                    "embedder_id": self.embedder_id,
                    "vector_dim": self.vector.len(),
                    "centroid_dim": self.centroid.len()
                }),
            ));
        }
        if !self.vector.iter().all(|value| value.is_finite()) {
            return Err(invalid_dda_input(
                "dda.inputs.vector",
                format!("vector for {} contains NaN or Inf", self.embedder_id),
                json!({ "embedder_id": self.embedder_id }),
            ));
        }
        if !self.centroid.iter().all(|value| value.is_finite()) {
            return Err(invalid_dda_input(
                "dda.inputs.centroid",
                format!("centroid for {} contains NaN or Inf", self.embedder_id),
                json!({ "embedder_id": self.embedder_id }),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DdaPairwiseBaseline {
    pub expected_cosine_upper: Vec<f32>,
    pub sigma_upper: Vec<f32>,
    pub pairwise_mi_upper: Vec<f32>,
}

impl DdaPairwiseBaseline {
    pub fn explicit_unit_baseline_for_count(embedder_count: usize) -> Self {
        let len = upper_triangle_len(embedder_count);
        Self {
            expected_cosine_upper: vec![0.0; len],
            sigma_upper: vec![1.0; len],
            pairwise_mi_upper: vec![0.0; len],
        }
    }

    pub fn validate(&self, embedder_count: usize) -> DdaResult<()> {
        let expected = upper_triangle_len(embedder_count);
        if self.expected_cosine_upper.len() != expected {
            return Err(invalid_dda_input(
                "dda.baseline.expected_cosine_upper",
                format!(
                    "expected_cosine_upper length {}, expected {}",
                    self.expected_cosine_upper.len(),
                    expected
                ),
                json!({ "embedder_count": embedder_count }),
            ));
        }
        if self.sigma_upper.len() != expected {
            return Err(invalid_dda_input(
                "dda.baseline.sigma_upper",
                format!(
                    "sigma_upper length {}, expected {}",
                    self.sigma_upper.len(),
                    expected
                ),
                json!({ "embedder_count": embedder_count }),
            ));
        }
        if self.pairwise_mi_upper.len() != expected {
            return Err(invalid_dda_input(
                "dda.baseline.pairwise_mi_upper",
                format!(
                    "pairwise_mi_upper length {}, expected {}",
                    self.pairwise_mi_upper.len(),
                    expected
                ),
                json!({ "embedder_count": embedder_count }),
            ));
        }
        for (idx, value) in self.expected_cosine_upper.iter().enumerate() {
            if !value.is_finite() || !(-1.0..=1.0).contains(value) {
                return Err(invalid_dda_input(
                    "dda.baseline.expected_cosine_upper",
                    format!("expected cosine at {idx} must be finite in [-1, 1]; got {value}"),
                    json!({ "index": idx }),
                ));
            }
        }
        for (idx, value) in self.sigma_upper.iter().enumerate() {
            if !value.is_finite() || *value <= 0.0 {
                return Err(invalid_dda_input(
                    "dda.baseline.sigma_upper",
                    format!("sigma at {idx} must be finite and positive; got {value}"),
                    json!({ "index": idx }),
                ));
            }
        }
        for (idx, value) in self.pairwise_mi_upper.iter().enumerate() {
            if !value.is_finite() || *value < 0.0 {
                return Err(invalid_dda_input(
                    "dda.baseline.pairwise_mi_upper",
                    format!("MI at {idx} must be finite and non-negative; got {value}"),
                    json!({ "index": idx }),
                ));
            }
        }
        Ok(())
    }
}

pub fn compute_dda_signals(
    inputs: &[DdaVectorInput],
    baseline: &DdaPairwiseBaseline,
) -> DdaResult<DdaSignals> {
    validate_inputs(inputs)?;
    baseline.validate(inputs.len())?;

    let per_embedder_cosine = inputs
        .iter()
        .map(|input| cosine_same_dim(&input.vector, &input.centroid, &input.embedder_id))
        .collect::<DdaResult<Vec<_>>>()?;

    let projected = inputs
        .iter()
        .map(project_to_pairwise_basis)
        .collect::<DdaResult<Vec<_>>>()?;
    let mut pairwise_cosine_upper = Vec::with_capacity(upper_triangle_len(inputs.len()));
    let mut blind_spot_z_scores = Vec::with_capacity(upper_triangle_len(inputs.len()));
    let mut idx = 0usize;
    for i in 0..projected.len() {
        for j in (i + 1)..projected.len() {
            let cosine = cosine_same_dim(
                &projected[i],
                &projected[j],
                &format!(
                    "pairwise:{}:{}",
                    inputs[i].embedder_id, inputs[j].embedder_id
                ),
            )?;
            pairwise_cosine_upper.push(cosine);
            blind_spot_z_scores
                .push((cosine - baseline.expected_cosine_upper[idx]) / baseline.sigma_upper[idx]);
            idx += 1;
        }
    }

    DdaSignals::try_new(DdaSignals {
        per_embedder_cosine,
        pairwise_cosine_upper,
        pairwise_mi_upper: baseline.pairwise_mi_upper.clone(),
        blind_spot_z_scores,
    })
    .map_err(|err| {
        TrainerError::new(
            TrainerErrorCode::MejepaTrainConfigInvalid,
            format!("computed DDA signals violated ME-JEPA invariants: {err}"),
        )
        .with_context(json!({
            "projection_schema": DDA_PAIRWISE_PROJECTION_SCHEMA,
            "projection_dim": DDA_PAIRWISE_PROJECTION_DIM,
            "remediation": "inspect DDA input dimensions, finite checks, and pairwise baseline lengths"
        }))
    })
}

pub fn persist_dda_signals(
    db: &DB,
    panel_id: &PanelId,
    chunk_id: &ChunkId,
    signals: &DdaSignals,
) -> DdaResult<()> {
    signals.validate().map_err(|err| {
        invalid_dda_input(
            "dda.signals",
            format!("DdaSignals validation failed before write: {err}"),
            json!({ "chunk_id": chunk_id.0 }),
        )
    })?;
    chunk_id.validate("chunk_id").map_err(|err| {
        invalid_dda_input(
            "chunk_id",
            format!("chunk id validation failed before DDA write: {err}"),
            json!({ "chunk_id": chunk_id.0 }),
        )
    })?;
    let cf = db.cf_handle(CF_MEJEPA_DDA_SIGNALS).ok_or_else(|| {
        TrainerError::new(
            TrainerErrorCode::MejepaTrainCertChainBroken,
            format!("missing RocksDB column family {CF_MEJEPA_DDA_SIGNALS}"),
        )
        .with_context(json!({
            "cf": CF_MEJEPA_DDA_SIGNALS,
            "remediation": "open the inference RocksDB with context_graph_mejepa_cf::INFER_CFS after Phase D"
        }))
    })?;
    let key = encode_dda_signal_key(panel_id, chunk_id)?;
    let value = serde_json::to_vec(signals)?;
    let mut write_opts = WriteOptions::default();
    write_opts.set_sync(true);
    db.put_cf_opt(cf, &key, &value, &write_opts)?;
    db.flush_cf(cf)?;
    let readback = read_dda_signals(db, panel_id, chunk_id)?.ok_or_else(|| {
        TrainerError::new(
            TrainerErrorCode::MejepaTrainCertChainBroken,
            "DDA signals readback returned None immediately after write",
        )
        .with_context(json!({ "chunk_id": chunk_id.0 }))
    })?;
    if &readback != signals {
        return Err(TrainerError::new(
            TrainerErrorCode::MejepaTrainCertChainBroken,
            "DDA signals readback did not match written value",
        )
        .with_context(json!({
            "chunk_id": chunk_id.0,
            "written": signals,
            "readback": readback
        })));
    }
    Ok(())
}

pub fn read_dda_signals(
    db: &DB,
    panel_id: &PanelId,
    chunk_id: &ChunkId,
) -> DdaResult<Option<DdaSignals>> {
    let cf = db.cf_handle(CF_MEJEPA_DDA_SIGNALS).ok_or_else(|| {
        TrainerError::new(
            TrainerErrorCode::MejepaTrainCertChainBroken,
            format!("missing RocksDB column family {CF_MEJEPA_DDA_SIGNALS}"),
        )
    })?;
    let key = encode_dda_signal_key(panel_id, chunk_id)?;
    let Some(bytes) = db.get_cf(cf, key)? else {
        return Ok(None);
    };
    let signals: DdaSignals = serde_json::from_slice(&bytes)?;
    signals.validate().map_err(|err| {
        TrainerError::new(
            TrainerErrorCode::MejepaTrainCertChainBroken,
            format!("persisted DDA signals failed validation on readback: {err}"),
        )
        .with_context(json!({
            "chunk_id": chunk_id.0,
            "remediation": "inspect the DDA writer version and RocksDB value bytes"
        }))
    })?;
    Ok(Some(signals))
}

pub fn count_persisted_dda_signals(db: &DB) -> DdaResult<usize> {
    let cf = db.cf_handle(CF_MEJEPA_DDA_SIGNALS).ok_or_else(|| {
        TrainerError::new(
            TrainerErrorCode::MejepaTrainCertChainBroken,
            format!("missing RocksDB column family {CF_MEJEPA_DDA_SIGNALS}"),
        )
    })?;
    let mut count = 0usize;
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        item?;
        count += 1;
    }
    Ok(count)
}

pub fn encode_dda_signal_key(panel_id: &PanelId, chunk_id: &ChunkId) -> DdaResult<Vec<u8>> {
    bincode::serialize(&(panel_id, chunk_id)).map_err(|err| {
        TrainerError::new(
            TrainerErrorCode::MejepaTrainCertChainBroken,
            format!("DDA key bincode serialization failed: {err}"),
        )
        .with_context(json!({
            "chunk_id": chunk_id.0,
            "remediation": "keep DDA key encoding stable as (PanelId, ChunkId)"
        }))
    })
}

fn validate_inputs(inputs: &[DdaVectorInput]) -> DdaResult<()> {
    if inputs.is_empty() {
        return Err(invalid_dda_input(
            "dda.inputs",
            "DDA signal computation requires at least one embedder vector",
            json!({ "input_count": 0 }),
        ));
    }
    let mut previous = None::<&str>;
    for (idx, input) in inputs.iter().enumerate() {
        input.validate(idx)?;
        if let Some(prev) = previous {
            if input.embedder_id.as_str() <= prev {
                return Err(invalid_dda_input(
                    "dda.inputs.embedder_id",
                    format!(
                        "embedder ids must be strictly increasing and unique; {} came after {}",
                        input.embedder_id, prev
                    ),
                    json!({ "index": idx, "embedder_id": input.embedder_id, "previous": prev }),
                ));
            }
        }
        previous = Some(&input.embedder_id);
    }
    Ok(())
}

fn project_to_pairwise_basis(input: &DdaVectorInput) -> DdaResult<Vec<f32>> {
    let mut out = vec![0.0f32; DDA_PAIRWISE_PROJECTION_DIM];
    for (idx, value) in input.vector.iter().enumerate() {
        let mut hasher = Sha256::new();
        hasher.update(input.embedder_id.as_bytes());
        hasher.update((idx as u64).to_le_bytes());
        let digest = hasher.finalize();
        let bucket = u64::from_le_bytes([
            digest[0], digest[1], digest[2], digest[3], digest[4], digest[5], digest[6], digest[7],
        ]) as usize
            % DDA_PAIRWISE_PROJECTION_DIM;
        let sign = if digest[8] & 1 == 0 { 1.0 } else { -1.0 };
        out[bucket] += sign * *value;
    }
    let norm = l2_norm(&out);
    if norm == 0.0 {
        return Err(invalid_dda_input(
            "dda.inputs.vector",
            format!(
                "signed hash projection for {} produced a zero vector",
                input.embedder_id
            ),
            json!({
                "embedder_id": input.embedder_id,
                "projection_schema": DDA_PAIRWISE_PROJECTION_SCHEMA
            }),
        ));
    }
    for value in &mut out {
        *value /= norm;
    }
    Ok(out)
}

fn cosine_same_dim(a: &[f32], b: &[f32], context: &str) -> DdaResult<f32> {
    if a.len() != b.len() {
        return Err(invalid_dda_input(
            "dda.cosine",
            format!(
                "cosine dimension mismatch for {context}: {} vs {}",
                a.len(),
                b.len()
            ),
            json!({ "context": context, "left_dim": a.len(), "right_dim": b.len() }),
        ));
    }
    let norm_a = l2_norm(a);
    let norm_b = l2_norm(b);
    if norm_a == 0.0 || norm_b == 0.0 {
        return Err(invalid_dda_input(
            "dda.cosine",
            format!("cosine received zero vector for {context}"),
            json!({ "context": context, "left_norm": norm_a, "right_norm": norm_b }),
        ));
    }
    let dot = a.iter().zip(b).map(|(x, y)| *x * *y).sum::<f32>();
    Ok((dot / (norm_a * norm_b)).clamp(-1.0, 1.0))
}

fn l2_norm(values: &[f32]) -> f32 {
    values.iter().map(|value| value * value).sum::<f32>().sqrt()
}

fn upper_triangle_len(n: usize) -> usize {
    n.saturating_mul(n.saturating_sub(1)) / 2
}

fn invalid_dda_input(
    field: &str,
    message: impl Into<String>,
    context: serde_json::Value,
) -> TrainerError {
    TrainerError::new(TrainerErrorCode::MejepaTrainConfigInvalid, message).with_context(json!({
        "field": field,
        "context": context,
        "remediation": "fix the DDA trigger input before computing or persisting signals"
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(id: &str, vector: Vec<f32>, centroid: Vec<f32>) -> DdaVectorInput {
        DdaVectorInput {
            embedder_id: id.to_string(),
            vector,
            centroid,
        }
    }

    #[test]
    fn computes_per_embedder_and_pairwise_signals() {
        let inputs = vec![
            input("E1", vec![1.0, 0.0, 0.0], vec![1.0, 0.0, 0.0]),
            input("E2", vec![0.0, 1.0, 0.0], vec![0.0, 1.0, 0.0]),
            input("E3", vec![1.0, 1.0, 0.0], vec![1.0, 0.0, 0.0]),
        ];
        let baseline = DdaPairwiseBaseline::explicit_unit_baseline_for_count(inputs.len());
        let signals = compute_dda_signals(&inputs, &baseline).unwrap();
        assert_eq!(signals.per_embedder_cosine.len(), 3);
        assert_eq!(signals.pairwise_cosine_upper.len(), 3);
        assert_eq!(signals.pairwise_mi_upper.len(), 3);
        assert_eq!(signals.blind_spot_z_scores, signals.pairwise_cosine_upper);
        assert!((signals.per_embedder_cosine[0] - 1.0).abs() < 1e-6);
        assert!((signals.per_embedder_cosine[2] - 0.70710677).abs() < 1e-5);
    }

    #[test]
    fn rejects_unsorted_or_duplicate_embedder_ids() {
        let inputs = vec![
            input("E2", vec![1.0], vec![1.0]),
            input("E1", vec![1.0], vec![1.0]),
        ];
        let baseline = DdaPairwiseBaseline::explicit_unit_baseline_for_count(inputs.len());
        let err = compute_dda_signals(&inputs, &baseline).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_TRAIN_CONFIG_INVALID");
    }

    #[test]
    fn rejects_zero_vector_cosine() {
        let inputs = vec![input("E1", vec![0.0, 0.0], vec![1.0, 0.0])];
        let baseline = DdaPairwiseBaseline::explicit_unit_baseline_for_count(inputs.len());
        let err = compute_dda_signals(&inputs, &baseline).unwrap_err();
        assert_eq!(err.code(), "MEJEPA_TRAIN_CONFIG_INVALID");
    }
}
