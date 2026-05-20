use crate::error::{TrainerError, TrainerErrorCode};
use candle_core::{DType, Device, Tensor, Var};
use candle_nn::{linear, Linear, Module, VarBuilder, VarMap};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub const NATIVE_ACTIVE_CONSTELLATION_SCHEMA_VERSION: u16 = 1;
pub const NATIVE_ACTIVE_CONSTELLATION_FEATURE_NAMES: [&str; 5] = [
    "abs_mean",
    "entropy_proxy",
    "norm_l2",
    "std",
    "zero_fraction",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeEmbedderSpec {
    pub embedder_id: String,
    pub dim: usize,
}

#[derive(Debug)]
pub struct NativeSlotTensor {
    pub spec: NativeEmbedderSpec,
    pub vectors: Tensor,
    pub coverage_mask: Tensor,
    pub vector_sha256_by_row: Vec<Option<String>>,
}

#[derive(Debug)]
pub struct NativeActiveConstellationBatch {
    pub schema_version: u16,
    pub row_keys: Vec<String>,
    pub labels: Tensor,
    pub labels_bool: Vec<bool>,
    pub slots: Vec<NativeSlotTensor>,
    pub pairwise_features: Tensor,
    pub pairwise_mask: Tensor,
    pub pairwise_pairs: Vec<(String, String)>,
    pub pairwise_feature_names: Vec<String>,
    pub coverage_counts: Vec<usize>,
    pub source_provenance: Vec<Value>,
    pub artifact_hashes: BTreeMap<String, String>,
    pub vector_hashes_verified: usize,
    pub flat_vector_concat_used: bool,
}

impl NativeActiveConstellationBatch {
    pub fn batch_size(&self) -> usize {
        self.row_keys.len()
    }

    pub fn embedder_specs(&self) -> Vec<NativeEmbedderSpec> {
        self.slots.iter().map(|slot| slot.spec.clone()).collect()
    }

    pub fn pairwise_feature_dim(&self) -> usize {
        self.pairwise_pairs.len() * self.pairwise_feature_names.len()
    }

    pub fn summary(&self) -> Value {
        let mut rows = Vec::with_capacity(self.row_keys.len());
        for (idx, row_key) in self.row_keys.iter().enumerate() {
            let present = self
                .slots
                .iter()
                .filter_map(|slot| {
                    slot.vector_sha256_by_row[idx].as_ref().map(|sha| {
                        json!({
                            "embedder_id": slot.spec.embedder_id,
                            "dim": slot.spec.dim,
                            "vector_sha256": sha,
                        })
                    })
                })
                .collect::<Vec<_>>();
            rows.push(json!({
                "row_key": row_key,
                "oracle_all_passed": self.labels_bool[idx],
                "coverage_count": self.coverage_counts[idx],
                "present_embedder_vectors": present,
                "source": self.source_provenance[idx],
            }));
        }
        json!({
            "schema_version": self.schema_version,
            "batch_size": self.batch_size(),
            "embedder_specs": self.embedder_specs(),
            "pairwise_pairs": self.pairwise_pairs,
            "pairwise_feature_names": self.pairwise_feature_names,
            "pairwise_feature_dim": self.pairwise_feature_dim(),
            "coverage_counts": self.coverage_counts,
            "coverage_min": self.coverage_counts.iter().copied().min().unwrap_or(0),
            "coverage_max": self.coverage_counts.iter().copied().max().unwrap_or(0),
            "ragged_coverage": self.coverage_counts.iter().copied().collect::<BTreeSet<_>>().len() > 1,
            "vector_hashes_verified": self.vector_hashes_verified,
            "flat_vector_concat_used": self.flat_vector_concat_used,
            "raw_vector_comparison_policy": "slot vectors stay in per-embedder tensors; cross-embedder evidence is represented only by scalar pairwise relation features plus masks",
            "artifact_hashes": self.artifact_hashes,
            "rows": rows,
        })
    }
}

#[derive(Debug, Clone)]
struct TrainingRowMeta {
    row_key: String,
    oracle_all_passed: bool,
    present_embedder_count: usize,
    source: Value,
}

#[derive(Debug, Clone)]
struct SlotVectorRow {
    embedder_id: String,
    dim: usize,
    vector: Vec<f32>,
    vector_sha256: String,
}

#[derive(Debug, Clone)]
struct PairwiseObservation {
    features: [f32; 5],
}

pub struct NativeActiveConstellationAdapter {
    varmap: VarMap,
    slot_blocks: Vec<NativeSlotAdapterBlock>,
    pairwise_proj: Linear,
    output_proj: Linear,
    hidden_dim: usize,
    pairwise_feature_dim: usize,
    dtype: DType,
    device: Device,
}

#[derive(Debug)]
struct NativeSlotAdapterBlock {
    spec: NativeEmbedderSpec,
    input_proj: Linear,
}

impl NativeActiveConstellationAdapter {
    pub fn new(
        specs: &[NativeEmbedderSpec],
        pairwise_feature_dim: usize,
        hidden_dim: usize,
        device: Device,
    ) -> Result<Self, TrainerError> {
        if specs.is_empty() {
            return Err(config_error(
                "native constellation adapter requires at least one embedder spec",
            ));
        }
        if pairwise_feature_dim == 0 {
            return Err(config_error(
                "native constellation adapter requires explicit pairwise relation features",
            ));
        }
        if hidden_dim == 0 {
            return Err(config_error(
                "native constellation adapter hidden_dim must be > 0",
            ));
        }
        let dtype = DType::BF16;
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, dtype, &device);
        let mut slot_blocks = Vec::with_capacity(specs.len());
        for spec in specs {
            if spec.dim == 0 {
                return Err(config_error(format!(
                    "embedder {} has zero dimension",
                    spec.embedder_id
                )));
            }
            slot_blocks.push(NativeSlotAdapterBlock {
                spec: spec.clone(),
                input_proj: linear(
                    spec.dim,
                    hidden_dim,
                    vb.pp(format!("slot_{}", spec.embedder_id)),
                )?,
            });
        }
        let pairwise_proj = linear(pairwise_feature_dim, hidden_dim, vb.pp("pairwise_proj"))?;
        let output_proj = linear(hidden_dim, 1, vb.pp("oracle_output"))?;
        Ok(Self {
            varmap,
            slot_blocks,
            pairwise_proj,
            output_proj,
            hidden_dim,
            pairwise_feature_dim,
            dtype,
            device,
        })
    }

    pub fn forward(&self, batch: &NativeActiveConstellationBatch) -> Result<Tensor, TrainerError> {
        if batch.batch_size() == 0 {
            return Err(config_error("native constellation batch is empty"));
        }
        if batch.pairwise_feature_dim() != self.pairwise_feature_dim {
            return Err(config_error(format!(
                "pairwise feature dim mismatch: batch={} model={}",
                batch.pairwise_feature_dim(),
                self.pairwise_feature_dim
            )));
        }
        let batch_size = batch.batch_size();
        let mut slot_sum = Tensor::zeros((batch_size, self.hidden_dim), self.dtype, &self.device)?;
        for block in &self.slot_blocks {
            let slot = batch
                .slots
                .iter()
                .find(|slot| slot.spec.embedder_id == block.spec.embedder_id)
                .ok_or_else(|| {
                    config_error(format!("batch missing embedder {}", block.spec.embedder_id))
                })?;
            if slot.spec.dim != block.spec.dim {
                return Err(config_error(format!(
                    "embedder {} dimension mismatch batch={} model={}",
                    block.spec.embedder_id, slot.spec.dim, block.spec.dim
                )));
            }
            let encoded = block
                .input_proj
                .forward(&slot.vectors.to_dtype(self.dtype)?)?;
            let masked = encoded.broadcast_mul(&slot.coverage_mask.to_dtype(self.dtype)?)?;
            slot_sum = (&slot_sum + &masked)?;
        }
        let coverage = Tensor::from_slice(
            &batch
                .coverage_counts
                .iter()
                .map(|count| (*count).max(1) as f32)
                .collect::<Vec<_>>(),
            (batch_size, 1),
            &self.device,
        )?
        .to_dtype(self.dtype)?;
        let slot_mean = slot_sum.broadcast_div(&coverage)?;
        let pairwise_hidden = self
            .pairwise_proj
            .forward(&batch.pairwise_features.to_dtype(self.dtype)?)?;
        let hidden = (&slot_mean + &pairwise_hidden)?.affine(0.5, 0.0)?;
        Ok(self.output_proj.forward(&hidden)?.to_dtype(DType::F32)?)
    }

    pub fn oracle_mse_loss(
        &self,
        batch: &NativeActiveConstellationBatch,
    ) -> Result<(Tensor, f32), TrainerError> {
        let logits = self.forward(batch)?;
        let labels = batch.labels.to_dtype(DType::F32)?;
        let loss = (&logits - &labels)?.sqr()?.mean_all()?;
        let scalar = loss.to_scalar::<f32>()?;
        if !scalar.is_finite() {
            return Err(TrainerError::new(
                TrainerErrorCode::MejepaTrainLossNan,
                "native active-embedder oracle MSE loss was non-finite",
            ));
        }
        Ok((loss, scalar))
    }

    pub fn trainable_parameters(&self) -> Vec<Var> {
        self.varmap.all_vars()
    }

    pub fn weight_sha256(&self) -> Result<String, TrainerError> {
        let data =
            self.varmap.data().lock().map_err(|_| {
                config_error("native constellation adapter VarMap mutex was poisoned")
            })?;
        let mut names = data.keys().cloned().collect::<Vec<_>>();
        names.sort();
        let mut hasher = Sha256::new();
        for name in names {
            let var = data
                .get(&name)
                .ok_or_else(|| config_error("native adapter VarMap changed during hash"))?;
            let tensor = var.as_tensor();
            hasher.update(name.as_bytes());
            hasher.update([0]);
            hasher.update(format!("{:?}", tensor.dtype()).as_bytes());
            hasher.update([0]);
            for dim in tensor.dims() {
                hasher.update(dim.to_le_bytes());
            }
            hasher.update([0]);
            let values = tensor
                .to_dtype(DType::F32)?
                .flatten_all()?
                .to_vec1::<f32>()?;
            for value in values {
                hasher.update(value.to_le_bytes());
            }
        }
        Ok(hex::encode(hasher.finalize()))
    }

    pub fn save_checkpoint(&self, path: &Path) -> Result<(), TrainerError> {
        self.varmap.save(path)?;
        Ok(())
    }

    pub fn load_checkpoint(&mut self, path: &Path) -> Result<(), TrainerError> {
        self.varmap.load(path)?;
        Ok(())
    }
}

pub fn build_native_active_constellation_batch(
    artifact_root: &Path,
    batch_size: usize,
    device: &Device,
) -> Result<NativeActiveConstellationBatch, TrainerError> {
    validate_aiwonder_artifact_root(artifact_root)?;
    if batch_size < 4 {
        return Err(config_error("native constellation batch_size must be >= 4"));
    }
    let training_rows_path = artifact_root.join("constellation_training_rows.jsonl");
    let slot_vectors_path = artifact_root.join("constellation_slot_vectors.jsonl");
    let pairwise_path = artifact_root.join("constellation_pairwise_observations.jsonl");
    let certificate_path = artifact_root.join("constellation_training_certificate.json");
    for path in [
        &training_rows_path,
        &slot_vectors_path,
        &pairwise_path,
        &certificate_path,
    ] {
        if !path.is_file() {
            return Err(config_error(format!(
                "native constellation artifact missing: {}",
                path.display()
            )));
        }
    }
    let certificate: Value = serde_json::from_slice(&fs::read(&certificate_path)?)?;
    if certificate["passes"] != true
        || certificate["objective_policy"]["flat_vector_concat_used"] != false
    {
        return Err(config_error(format!(
            "native constellation certificate invalid: {}",
            certificate_path.display()
        )));
    }

    let rows = read_training_rows(&training_rows_path)?;
    let selected = select_training_rows(&rows, batch_size)?;
    let row_keys = selected
        .iter()
        .map(|row| row.row_key.clone())
        .collect::<Vec<_>>();
    let selected_key_set = row_keys.iter().cloned().collect::<BTreeSet<_>>();
    let labels_bool = selected
        .iter()
        .map(|row| row.oracle_all_passed)
        .collect::<Vec<_>>();
    let source_provenance = selected
        .iter()
        .map(|row| row.source.clone())
        .collect::<Vec<_>>();
    let label_values = labels_bool
        .iter()
        .map(|value| if *value { 1.0f32 } else { 0.0 })
        .collect::<Vec<_>>();
    let labels = Tensor::from_slice(&label_values, (row_keys.len(), 1), device)?;

    let slot_rows = read_slot_vectors(&slot_vectors_path, &selected_key_set)?;
    let specs = embedder_specs(&slot_rows)?;
    let pairwise = read_pairwise_observations(&pairwise_path, &selected_key_set)?;
    let pairwise_pairs = pairwise_pairs(&specs);
    let slots = build_slot_tensors(&row_keys, &specs, &slot_rows, device)?;
    let coverage_counts = slots
        .iter()
        .fold(vec![0usize; row_keys.len()], |mut counts, slot| {
            for (idx, sha) in slot.vector_sha256_by_row.iter().enumerate() {
                if sha.is_some() {
                    counts[idx] += 1;
                }
            }
            counts
        });
    if coverage_counts.contains(&0) {
        return Err(config_error(
            "native constellation selected a row with zero embedder coverage",
        ));
    }
    let pairwise_feature_names = NATIVE_ACTIVE_CONSTELLATION_FEATURE_NAMES
        .iter()
        .map(|name| (*name).to_string())
        .collect::<Vec<_>>();
    let (pairwise_features, pairwise_mask) = build_pairwise_tensors(
        &row_keys,
        &pairwise_pairs,
        &pairwise_feature_names,
        &pairwise,
        device,
    )?;
    let vector_hashes_verified = slot_rows.len();
    let artifact_hashes = BTreeMap::from([
        (
            "constellation_training_rows".to_string(),
            sha256_file(&training_rows_path)?,
        ),
        (
            "constellation_slot_vectors".to_string(),
            sha256_file(&slot_vectors_path)?,
        ),
        (
            "constellation_pairwise_observations".to_string(),
            sha256_file(&pairwise_path)?,
        ),
        (
            "constellation_training_certificate".to_string(),
            sha256_file(&certificate_path)?,
        ),
    ]);
    Ok(NativeActiveConstellationBatch {
        schema_version: NATIVE_ACTIVE_CONSTELLATION_SCHEMA_VERSION,
        row_keys,
        labels,
        labels_bool,
        slots,
        pairwise_features,
        pairwise_mask,
        pairwise_pairs,
        pairwise_feature_names,
        coverage_counts,
        source_provenance,
        artifact_hashes,
        vector_hashes_verified,
        flat_vector_concat_used: false,
    })
}

fn read_training_rows(path: &Path) -> Result<Vec<TrainingRowMeta>, TrainerError> {
    let file = fs::File::open(path)?;
    let mut rows = Vec::new();
    for (idx, line) in BufReader::new(file).lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(&line).map_err(|err| {
            config_error(format!("parse {} line {}: {err}", path.display(), idx + 1))
        })?;
        if value["partition"] != "train" {
            continue;
        }
        let row_key = required_str(&value, "row_key")?;
        let oracle_all_passed = value["labels"]["oracle_all_passed"]
            .as_bool()
            .ok_or_else(|| config_error(format!("row {row_key} missing oracle_all_passed")))?;
        let present_embedder_count = value["slot_policy"]["present_embedder_count"]
            .as_u64()
            .ok_or_else(|| {
                config_error(format!(
                    "row {row_key} missing slot_policy.present_embedder_count"
                ))
            })? as usize;
        rows.push(TrainingRowMeta {
            row_key,
            oracle_all_passed,
            present_embedder_count,
            source: value["source"].clone(),
        });
    }
    if rows.is_empty() {
        return Err(config_error(format!(
            "no train rows in native constellation artifact {}",
            path.display()
        )));
    }
    Ok(rows)
}

fn select_training_rows(
    rows: &[TrainingRowMeta],
    batch_size: usize,
) -> Result<Vec<TrainingRowMeta>, TrainerError> {
    let mut selected = Vec::<TrainingRowMeta>::new();
    let mut seen = BTreeSet::<String>::new();
    let add = |row: &TrainingRowMeta,
               selected: &mut Vec<TrainingRowMeta>,
               seen: &mut BTreeSet<String>| {
        if selected.len() < batch_size && seen.insert(row.row_key.clone()) {
            selected.push(row.clone());
        }
    };
    if let Some(max) = rows.iter().max_by_key(|row| row.present_embedder_count) {
        add(max, &mut selected, &mut seen);
    }
    if let Some(min) = rows.iter().min_by_key(|row| row.present_embedder_count) {
        add(min, &mut selected, &mut seen);
    }
    for label in [true, false] {
        if let Some(row) = rows.iter().find(|row| row.oracle_all_passed == label) {
            add(row, &mut selected, &mut seen);
        }
    }
    let mut by_coverage = rows.iter().collect::<Vec<_>>();
    by_coverage.sort_by_key(|row| {
        (
            row.present_embedder_count,
            !row.oracle_all_passed,
            row.row_key.clone(),
        )
    });
    for row in by_coverage {
        add(row, &mut selected, &mut seen);
        if selected.len() == batch_size {
            break;
        }
    }
    if selected.len() != batch_size {
        return Err(config_error(format!(
            "native constellation batch selected {} rows, expected {batch_size}",
            selected.len()
        )));
    }
    let labels = selected
        .iter()
        .map(|row| row.oracle_all_passed)
        .collect::<BTreeSet<_>>();
    if labels.len() < 2 {
        return Err(config_error(
            "native constellation batch has degenerate oracle labels",
        ));
    }
    let coverage = selected
        .iter()
        .map(|row| row.present_embedder_count)
        .collect::<BTreeSet<_>>();
    if coverage.len() < 2 {
        return Err(config_error(
            "native constellation batch did not exercise ragged coverage",
        ));
    }
    Ok(selected)
}

fn read_slot_vectors(
    path: &Path,
    selected_keys: &BTreeSet<String>,
) -> Result<BTreeMap<(String, String), SlotVectorRow>, TrainerError> {
    let file = fs::File::open(path)?;
    let mut rows = BTreeMap::new();
    for (idx, line) in BufReader::new(file).lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(&line).map_err(|err| {
            config_error(format!("parse {} line {}: {err}", path.display(), idx + 1))
        })?;
        let row_key = required_str(&value, "row_key")?;
        if !selected_keys.contains(&row_key) {
            continue;
        }
        if value["flat_vector_concat_used"] != false {
            return Err(config_error(format!(
                "slot vector row {row_key} did not declare flat_vector_concat_used=false"
            )));
        }
        let embedder_id = required_str(&value, "embedder_id")?;
        let dim = value["dim"]
            .as_u64()
            .ok_or_else(|| config_error(format!("{row_key}:{embedder_id} missing dim")))?
            as usize;
        let vector_values = value["vector"]
            .as_array()
            .ok_or_else(|| config_error(format!("{row_key}:{embedder_id} missing vector array")))?;
        if vector_values.len() != dim {
            return Err(config_error(format!(
                "{row_key}:{embedder_id} vector length {} != dim {dim}",
                vector_values.len()
            )));
        }
        let mut vector = Vec::with_capacity(dim);
        for value in vector_values {
            let value = value.as_f64().ok_or_else(|| {
                config_error(format!(
                    "{row_key}:{embedder_id} vector contains non-number"
                ))
            })? as f32;
            if !value.is_finite() {
                return Err(TrainerError::new(
                    TrainerErrorCode::MejepaTrainLossNan,
                    format!("{row_key}:{embedder_id} vector contains non-finite value"),
                ));
            }
            vector.push(value);
        }
        let vector_sha256 = required_str(&value, "vector_sha256")?;
        let expected = vector_sha256
            .strip_prefix("sha256:")
            .unwrap_or(&vector_sha256);
        let observed = sha256_f32s(&vector);
        if observed != expected {
            return Err(TrainerError::new(
                TrainerErrorCode::MejepaTrainEmbedderDigestMismatch,
                format!(
                    "{row_key}:{embedder_id} vector sha mismatch expected {expected} got {observed}"
                ),
            ));
        }
        rows.insert(
            (row_key.clone(), embedder_id.clone()),
            SlotVectorRow {
                embedder_id,
                dim,
                vector,
                vector_sha256,
            },
        );
    }
    if rows.is_empty() {
        return Err(config_error(
            "native constellation slot-vector readback was empty",
        ));
    }
    Ok(rows)
}

fn embedder_specs(
    rows: &BTreeMap<(String, String), SlotVectorRow>,
) -> Result<Vec<NativeEmbedderSpec>, TrainerError> {
    let mut dims = BTreeMap::<String, usize>::new();
    for row in rows.values() {
        match dims.get(&row.embedder_id) {
            Some(existing) if *existing != row.dim => {
                return Err(config_error(format!(
                    "embedder {} has conflicting dims {} and {}",
                    row.embedder_id, existing, row.dim
                )));
            }
            Some(_) => {}
            None => {
                dims.insert(row.embedder_id.clone(), row.dim);
            }
        }
    }
    Ok(dims
        .into_iter()
        .map(|(embedder_id, dim)| NativeEmbedderSpec { embedder_id, dim })
        .collect())
}

fn read_pairwise_observations(
    path: &Path,
    selected_keys: &BTreeSet<String>,
) -> Result<BTreeMap<(String, String, String), PairwiseObservation>, TrainerError> {
    let file = fs::File::open(path)?;
    let mut rows = BTreeMap::new();
    for (idx, line) in BufReader::new(file).lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(&line).map_err(|err| {
            config_error(format!("parse {} line {}: {err}", path.display(), idx + 1))
        })?;
        let row_key = required_str(&value, "row_key")?;
        if !selected_keys.contains(&row_key) {
            continue;
        }
        if value["flat_vector_concat_used"] != false {
            return Err(config_error(format!(
                "pairwise row {row_key} did not declare flat_vector_concat_used=false"
            )));
        }
        let left = required_str(&value, "left_embedder_id")?;
        let right = required_str(&value, "right_embedder_id")?;
        let scalar = &value["scalar_feature_abs_delta"];
        let mut features = [0.0f32; 5];
        for (idx, name) in NATIVE_ACTIVE_CONSTELLATION_FEATURE_NAMES.iter().enumerate() {
            let value = scalar[*name].as_f64().ok_or_else(|| {
                config_error(format!("pairwise {row_key}:{left}:{right} missing {name}"))
            })? as f32;
            if !value.is_finite() {
                return Err(TrainerError::new(
                    TrainerErrorCode::MejepaTrainLossNan,
                    format!("pairwise {row_key}:{left}:{right} {name} non-finite"),
                ));
            }
            features[idx] = value;
        }
        rows.insert(
            (row_key.clone(), left.clone(), right.clone()),
            PairwiseObservation { features },
        );
    }
    if rows.is_empty() {
        return Err(config_error(
            "native constellation pairwise observation readback was empty",
        ));
    }
    Ok(rows)
}

fn build_slot_tensors(
    row_keys: &[String],
    specs: &[NativeEmbedderSpec],
    rows: &BTreeMap<(String, String), SlotVectorRow>,
    device: &Device,
) -> Result<Vec<NativeSlotTensor>, TrainerError> {
    let mut tensors = Vec::with_capacity(specs.len());
    for spec in specs {
        let mut values = vec![0.0f32; row_keys.len() * spec.dim];
        let mut mask = Vec::with_capacity(row_keys.len());
        let mut sha_by_row = Vec::with_capacity(row_keys.len());
        for (row_idx, row_key) in row_keys.iter().enumerate() {
            if let Some(row) = rows.get(&(row_key.clone(), spec.embedder_id.clone())) {
                let start = row_idx * spec.dim;
                values[start..start + spec.dim].copy_from_slice(&row.vector);
                mask.push(1.0f32);
                sha_by_row.push(Some(row.vector_sha256.clone()));
            } else {
                mask.push(0.0f32);
                sha_by_row.push(None);
            }
        }
        tensors.push(NativeSlotTensor {
            spec: spec.clone(),
            vectors: Tensor::from_slice(&values, (row_keys.len(), spec.dim), device)?,
            coverage_mask: Tensor::from_slice(&mask, (row_keys.len(), 1), device)?,
            vector_sha256_by_row: sha_by_row,
        });
    }
    Ok(tensors)
}

fn pairwise_pairs(specs: &[NativeEmbedderSpec]) -> Vec<(String, String)> {
    let ids = specs
        .iter()
        .map(|spec| spec.embedder_id.clone())
        .collect::<Vec<_>>();
    let mut pairs = Vec::new();
    for left in 0..ids.len() {
        for right in left + 1..ids.len() {
            pairs.push((ids[left].clone(), ids[right].clone()));
        }
    }
    pairs
}

fn build_pairwise_tensors(
    row_keys: &[String],
    pairs: &[(String, String)],
    feature_names: &[String],
    observations: &BTreeMap<(String, String, String), PairwiseObservation>,
    device: &Device,
) -> Result<(Tensor, Tensor), TrainerError> {
    let pair_feature_count = pairs.len() * feature_names.len();
    let mut values = vec![0.0f32; row_keys.len() * pair_feature_count];
    let mut mask = vec![0.0f32; row_keys.len() * pairs.len()];
    for (row_idx, row_key) in row_keys.iter().enumerate() {
        for (pair_idx, (left, right)) in pairs.iter().enumerate() {
            if let Some(obs) = observations.get(&(row_key.clone(), left.clone(), right.clone())) {
                mask[row_idx * pairs.len() + pair_idx] = 1.0;
                let start = row_idx * pair_feature_count + pair_idx * feature_names.len();
                values[start..start + feature_names.len()].copy_from_slice(&obs.features);
            }
        }
    }
    Ok((
        Tensor::from_slice(&values, (row_keys.len(), pair_feature_count), device)?,
        Tensor::from_slice(&mask, (row_keys.len(), pairs.len()), device)?,
    ))
}

fn validate_aiwonder_artifact_root(path: &Path) -> Result<(), TrainerError> {
    let text = path.to_string_lossy();
    if text.contains("/mnt/d") {
        return Err(config_error(format!(
            "native constellation artifacts must not use /mnt/d: {}",
            path.display()
        )));
    }
    if !path.starts_with("/zfs/archive/contextgraph") && !path.starts_with("/zfs/hot/contextgraph")
    {
        return Err(config_error(format!(
            "native constellation artifacts must live on aiwonder /zfs roots: {}",
            path.display()
        )));
    }
    Ok(())
}

fn required_str(value: &Value, field: &str) -> Result<String, TrainerError> {
    value[field]
        .as_str()
        .map(ToString::to_string)
        .ok_or_else(|| config_error(format!("missing string field {field}")))
}

fn sha256_file(path: &Path) -> Result<String, TrainerError> {
    Ok(sha256_hex(&fs::read(path)?))
}

fn sha256_f32s(values: &[f32]) -> String {
    let mut hasher = Sha256::new();
    for value in values {
        hasher.update(value.to_le_bytes());
    }
    hex::encode(hasher.finalize())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn config_error(message: impl Into<String>) -> TrainerError {
    TrainerError::new(TrainerErrorCode::MejepaTrainConfigInvalid, message)
}
