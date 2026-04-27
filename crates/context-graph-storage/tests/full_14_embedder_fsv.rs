//! Full 14-embedder state verification.
//!
//! These tests intentionally print before/after state and read from the
//! physical source of truth after executing writes. They are designed for manual
//! FSV runs with `-- --nocapture`.

use context_graph_core::clustering::MAX_WEIGHTED_AGREEMENT;
use context_graph_core::embeddings::category::{category_for, max_weighted_agreement};
use context_graph_core::embeddings::config::{get_quantization, QuantizationConfig};
use context_graph_core::teleological::embedder::{Embedder, EmbedderDims};
use context_graph_core::teleological::groups::{group_indices, GroupAlignments};
use context_graph_core::teleological::synergy_matrix::SynergyMatrix;
use context_graph_core::teleological::types::NUM_EMBEDDERS;
use context_graph_core::training::{
    compute_cross_correlations, compute_group_alignments, topic_profile_or_fallback,
    NUM_CROSS_CORRELATIONS,
};
use context_graph_core::types::fingerprint::{
    SemanticFingerprint, SparseVector, E10_DIM, E11_DIM, E12_TOKEN_DIM, E13_SPLADE_VOCAB, E14_DIM,
    E1_DIM, E2_DIM, E3_DIM, E4_DIM, E5_DIM, E6_SPARSE_VOCAB, E7_DIM, E8_DIM, E9_DIM,
};
use context_graph_embeddings::types::ModelId;
use context_graph_embeddings::{
    QuantizationMetadata, QuantizationMethod, QuantizedEmbedding, StoredQuantizedFingerprint,
};
use context_graph_storage::teleological::{
    deserialize_topic_profile, topic_profile_key, QuantizedFingerprintStorage,
    RocksDbTeleologicalStore, CF_TOPIC_PROFILES, QUANTIZED_EMBEDDER_CFS,
};
use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use tempfile::TempDir;
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
struct SlotSpec {
    idx: u8,
    embedder: Embedder,
    model: ModelId,
    method: QuantizationMethod,
    storage_dim: usize,
    data_len: usize,
}

fn section(title: &str) {
    println!("\n================================================================================");
    println!("14-EMBEDDER FSV: {}", title);
    println!("================================================================================");
}

fn slot_specs() -> Vec<SlotSpec> {
    ModelId::production()
        .iter()
        .copied()
        .enumerate()
        .map(|(idx, model)| {
            let method = QuantizationMethod::for_model_id(model);
            let storage_dim = model.dimension();
            let data_len = match method {
                QuantizationMethod::PQ8 => 8,
                QuantizationMethod::Float8E4M3 => storage_dim,
                QuantizationMethod::Binary => (storage_dim + 7) / 8,
                QuantizationMethod::SparseNative => 32,
                QuantizationMethod::TokenPruning => 64,
            };

            SlotSpec {
                idx: idx as u8,
                embedder: Embedder::from_index(idx).expect("slot in range"),
                model,
                method,
                storage_dim,
                data_len,
            }
        })
        .collect()
}

fn method_from_core_config(config: QuantizationConfig) -> QuantizationMethod {
    match config {
        QuantizationConfig::PQ8 { .. } => QuantizationMethod::PQ8,
        QuantizationConfig::Float8 => QuantizationMethod::Float8E4M3,
        QuantizationConfig::Binary => QuantizationMethod::Binary,
        QuantizationConfig::TokenPruning => QuantizationMethod::TokenPruning,
        QuantizationConfig::Inverted => QuantizationMethod::SparseNative,
        QuantizationConfig::None => {
            panic!("production embedders must not use uncompressed storage")
        }
    }
}

fn payload(spec: SlotSpec) -> Vec<u8> {
    (0..spec.data_len)
        .map(|j| {
            (spec.idx)
                .wrapping_mul(19)
                .wrapping_add((j % 251) as u8)
                .wrapping_add(3)
        })
        .collect()
}

fn metadata(spec: SlotSpec) -> QuantizationMetadata {
    match spec.method {
        QuantizationMethod::PQ8 => QuantizationMetadata::PQ8 {
            codebook_id: spec.idx as u32,
            num_subvectors: 8,
        },
        QuantizationMethod::Float8E4M3 => QuantizationMetadata::Float8 {
            scale: 1.0 + spec.idx as f32 * 0.01,
            bias: -(spec.idx as f32) * 0.001,
        },
        QuantizationMethod::Binary => QuantizationMetadata::Binary { threshold: 0.0 },
        QuantizationMethod::SparseNative => QuantizationMetadata::Sparse {
            vocab_size: spec.storage_dim,
            nnz: spec.data_len / 2,
        },
        QuantizationMethod::TokenPruning => QuantizationMetadata::TokenPruning {
            original_tokens: 128,
            kept_tokens: 64,
            threshold: 0.5,
        },
    }
}

fn build_embeddings() -> HashMap<u8, QuantizedEmbedding> {
    slot_specs()
        .into_iter()
        .map(|spec| {
            (
                spec.idx,
                QuantizedEmbedding {
                    method: spec.method,
                    original_dim: spec.storage_dim,
                    data: payload(spec),
                    metadata: metadata(spec),
                },
            )
        })
        .collect()
}

fn dense_unit(dim: usize) -> Vec<f32> {
    let mut v = vec![0.0; dim];
    if let Some(first) = v.first_mut() {
        *first = 1.0;
    }
    v
}

fn complete_semantic_fingerprint() -> SemanticFingerprint {
    SemanticFingerprint {
        e1_semantic: dense_unit(E1_DIM),
        e2_temporal_recent: dense_unit(E2_DIM),
        e3_temporal_periodic: dense_unit(E3_DIM),
        e4_temporal_positional: dense_unit(E4_DIM),
        e5_causal_as_cause: dense_unit(E5_DIM),
        e5_causal_as_effect: Vec::new(),
        e5_causal: Vec::new(),
        e6_sparse: SparseVector {
            indices: vec![1],
            values: vec![1.0],
        },
        e7_code: dense_unit(E7_DIM),
        e8_graph_as_source: dense_unit(E8_DIM),
        e8_graph_as_target: Vec::new(),
        e8_graph: Vec::new(),
        e9_hdc: dense_unit(E9_DIM),
        e10_multimodal_paraphrase: dense_unit(E10_DIM),
        e10_multimodal_as_context: Vec::new(),
        e11_entity: dense_unit(E11_DIM),
        e12_late_interaction: vec![dense_unit(E12_TOKEN_DIM), vec![0.0; E12_TOKEN_DIM]],
        e13_splade: SparseVector {
            indices: vec![2],
            values: vec![1.0],
        },
        e14_bge_m3_dense: dense_unit(E14_DIM),
    }
}

fn empty_semantic_fingerprint() -> SemanticFingerprint {
    SemanticFingerprint {
        e1_semantic: Vec::new(),
        e2_temporal_recent: Vec::new(),
        e3_temporal_periodic: Vec::new(),
        e4_temporal_positional: Vec::new(),
        e5_causal_as_cause: Vec::new(),
        e5_causal_as_effect: Vec::new(),
        e5_causal: Vec::new(),
        e6_sparse: SparseVector {
            indices: Vec::new(),
            values: Vec::new(),
        },
        e7_code: Vec::new(),
        e8_graph_as_source: Vec::new(),
        e8_graph_as_target: Vec::new(),
        e8_graph: Vec::new(),
        e9_hdc: Vec::new(),
        e10_multimodal_paraphrase: Vec::new(),
        e10_multimodal_as_context: Vec::new(),
        e11_entity: Vec::new(),
        e12_late_interaction: Vec::new(),
        e13_splade: SparseVector {
            indices: Vec::new(),
            values: Vec::new(),
        },
        e14_bge_m3_dense: Vec::new(),
    }
}

fn assert_close(actual: f32, expected: f32, context: &str) {
    let diff = (actual - expected).abs();
    assert!(
        diff <= 1e-6,
        "{}: expected {}, got {}, diff {}",
        context,
        expected,
        actual,
        diff
    );
}

fn pair_index(i: usize, j: usize) -> usize {
    assert!(i < j);
    let mut idx = 0;
    for a in 0..NUM_EMBEDDERS {
        for b in (a + 1)..NUM_EMBEDDERS {
            if a == i && b == j {
                return idx;
            }
            idx += 1;
        }
    }
    panic!("pair out of range: ({}, {})", i, j);
}

#[test]
fn fsv_all_14_model_slot_contracts_are_aligned() {
    section("slot/model/embedder/quantization contract");

    let specs = slot_specs();
    println!(
        "BEFORE: ModelId::production().len() = {}",
        ModelId::production().len()
    );
    println!("BEFORE: Embedder::COUNT = {}", Embedder::COUNT);

    assert_eq!(specs.len(), 14);
    assert_eq!(ModelId::production().len(), NUM_EMBEDDERS);
    assert_eq!(Embedder::COUNT, NUM_EMBEDDERS);

    for spec in specs {
        let core_method = method_from_core_config(get_quantization(spec.embedder));
        let embedder_dim = match spec.embedder.expected_dims() {
            EmbedderDims::Dense(dim) => dim,
            EmbedderDims::Sparse { vocab_size } => vocab_size,
            EmbedderDims::TokenLevel { per_token } => per_token,
        };
        println!(
            "AFTER: slot {:02} {} uses model {:?}, storage_dim={}, embedder_dim={}, method={:?}, cf={}",
            spec.idx,
            spec.embedder.name(),
            spec.model,
            spec.storage_dim,
            embedder_dim,
            spec.method,
            QUANTIZED_EMBEDDER_CFS[spec.idx as usize]
        );

        assert_eq!(
            core_method, spec.method,
            "core config/storage method mismatch at slot {}",
            spec.idx
        );
        if spec.embedder == Embedder::Hdc {
            assert_eq!(embedder_dim, E9_DIM, "HDC projected dense dim");
            assert_eq!(spec.storage_dim, 10000, "HDC raw binary storage dim");
        } else {
            assert_eq!(
                embedder_dim, spec.storage_dim,
                "Embedder expected dim must match storage dim at slot {}",
                spec.idx
            );
        }
    }

    assert_eq!(QUANTIZED_EMBEDDER_CFS.len(), 14);
    println!("EVIDENCE: all 14 active production slots align with embedder enum, CF list, dimensions, and quantization methods");
}

#[test]
fn fsv_all_14_embedders_persist_to_physical_rocksdb_cfs() {
    section("physical RocksDB storage for emb_0 through emb_13");

    let tmp = TempDir::new().expect("temp dir");
    let db_path = tmp.path().to_path_buf();
    let id = Uuid::new_v4();
    let topic_profile = [
        0.01, 0.02, 0.03, 0.04, 0.05, 0.06, 0.07, 0.08, 0.09, 0.10, 0.11, 0.12, 0.13, 0.14,
    ];
    let expected = build_embeddings();

    println!("BEFORE: id = {}", id);
    for spec in slot_specs() {
        let emb = expected.get(&spec.idx).expect("expected slot");
        println!(
            "BEFORE: slot {:02} {} cf={} method={:?} dim={} data_len={} prefix={:?}",
            spec.idx,
            spec.embedder.name(),
            QUANTIZED_EMBEDDER_CFS[spec.idx as usize],
            emb.method,
            emb.original_dim,
            emb.data.len(),
            &emb.data[..emb.data.len().min(6)]
        );
    }

    {
        let store = RocksDbTeleologicalStore::open(&db_path).expect("open write store");
        let fp = StoredQuantizedFingerprint::new(id, expected.clone(), topic_profile, [0xCD; 32]);
        assert!(
            fp.validate_quantization_methods(),
            "synthetic slots must match production methods"
        );
        store
            .store_quantized_fingerprint(&fp)
            .expect("store all 14 slots and topic profile");
        println!(
            "EXECUTE: store_quantized_fingerprint wrote id={} into RocksDB",
            id
        );
    }

    let reopened = RocksDbTeleologicalStore::open(&db_path).expect("open read store");
    println!("AFTER: reopened RocksDB at {}", db_path.display());

    for spec in slot_specs() {
        let key = *id.as_bytes();
        let cf_name = QUANTIZED_EMBEDDER_CFS[spec.idx as usize];
        let cf = reopened
            .db()
            .cf_handle(cf_name)
            .expect("embedder CF must exist");
        let raw = reopened
            .db()
            .get_cf(cf, key)
            .expect("raw CF read")
            .expect("raw CF row must exist");
        let loaded = reopened
            .load_embedder(id, spec.idx)
            .expect("load_embedder after reopen");
        let exp = expected.get(&spec.idx).expect("expected slot");

        println!(
            "AFTER: slot {:02} raw_cf_bytes={} loaded_method={:?} loaded_dim={} loaded_data_len={} prefix={:?}",
            spec.idx,
            raw.len(),
            loaded.method,
            loaded.original_dim,
            loaded.data.len(),
            &loaded.data[..loaded.data.len().min(6)]
        );

        assert!(
            !raw.is_empty(),
            "raw CF value must exist at slot {}",
            spec.idx
        );
        assert_eq!(
            loaded.method, exp.method,
            "method mismatch at slot {}",
            spec.idx
        );
        assert_eq!(
            loaded.original_dim, exp.original_dim,
            "dim mismatch at slot {}",
            spec.idx
        );
        assert_eq!(
            loaded.data, exp.data,
            "data bytes mismatch at slot {}",
            spec.idx
        );
    }

    let topic_cf = reopened
        .db()
        .cf_handle(CF_TOPIC_PROFILES)
        .expect("topic profile CF must exist");
    let topic_raw = reopened
        .db()
        .get_cf(topic_cf, topic_profile_key(&id))
        .expect("topic profile raw read")
        .expect("topic profile row must exist");
    let persisted_topic = deserialize_topic_profile(&topic_raw).expect("decode topic profile");
    println!(
        "AFTER: topic_profiles raw bytes={} persisted_topic={:?}",
        topic_raw.len(),
        persisted_topic
    );
    assert_eq!(topic_raw.len(), 56);
    assert_eq!(persisted_topic, topic_profile);

    let loaded_full = reopened
        .load_quantized_fingerprint(id)
        .expect("load full fingerprint");
    println!(
        "AFTER: loaded_full.embeddings.len()={}, loaded_full.topic_profile={:?}",
        loaded_full.embeddings.len(),
        loaded_full.topic_profile
    );
    assert_eq!(loaded_full.embeddings.len(), 14);
    assert_eq!(loaded_full.topic_profile, topic_profile);

    println!("EDGE BEFORE: construct fingerprint with one missing slot, len=13");
    let missing_result = std::panic::catch_unwind(|| {
        let mut missing = build_embeddings();
        missing.remove(&5);
        let _ = StoredQuantizedFingerprint::new(Uuid::new_v4(), missing, [0.0; 14], [0; 32]);
    });
    println!(
        "EDGE AFTER: missing slot panic observed = {}",
        missing_result.is_err()
    );
    assert!(missing_result.is_err());

    println!("EDGE BEFORE: attempt invalid load_embedder(id, 14)");
    let invalid_result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let _ = reopened.load_embedder(id, 14);
    }));
    println!(
        "EDGE AFTER: invalid index panic observed = {}",
        invalid_result.is_err()
    );
    assert!(invalid_result.is_err());

    println!("EDGE BEFORE: delete_quantized_fingerprint(id={})", id);
    reopened
        .delete_quantized_fingerprint(id)
        .expect("delete fingerprint");
    for spec in slot_specs() {
        let after_delete = reopened.load_embedder(id, spec.idx);
        println!(
            "EDGE AFTER: slot {:02} load after delete is_err={}",
            spec.idx,
            after_delete.is_err()
        );
        assert!(
            after_delete.is_err(),
            "slot {} should be absent after delete",
            spec.idx
        );
    }
    let topic_after_delete = reopened
        .db()
        .get_cf(topic_cf, topic_profile_key(&id))
        .expect("topic read after delete");
    println!(
        "EDGE AFTER: topic profile row exists after delete = {}",
        topic_after_delete.is_some()
    );
    assert!(topic_after_delete.is_none());
}

#[test]
fn fsv_all_91_cross_correlations_match_manual_formula() {
    section("91 pairwise cross-correlations");

    let synergy = SynergyMatrix::with_base_synergies();
    let mut profile = [0.0f32; NUM_EMBEDDERS];
    for (i, value) in profile.iter_mut().enumerate() {
        *value = (i as f32 + 1.0) / 20.0;
    }

    println!("BEFORE: topic_profile = {:?}", profile);
    let actual = compute_cross_correlations(&profile, &synergy);
    println!("AFTER: correlation_count = {}", actual.len());
    assert_eq!(actual.len(), NUM_CROSS_CORRELATIONS);
    assert_eq!(actual.len(), 91);

    let mut idx = 0;
    for i in 0..NUM_EMBEDDERS {
        for j in (i + 1)..NUM_EMBEDDERS {
            let weight = synergy.get_synergy(i, j);
            let expected = profile[i] * profile[j] * weight;
            println!(
                "AFTER: pair #{:02} E{}xE{} profile_i={} profile_j={} synergy={} expected={} actual={}",
                idx,
                i + 1,
                j + 1,
                profile[i],
                profile[j],
                weight,
                expected,
                actual[idx]
            );
            assert_close(actual[idx], expected, "manual cross-correlation");
            idx += 1;
        }
    }
    assert_eq!(idx, 91);

    let zeros = [0.0f32; NUM_EMBEDDERS];
    println!("EDGE BEFORE: zero profile = {:?}", zeros);
    let zero_corr = compute_cross_correlations(&zeros, &synergy);
    println!(
        "EDGE AFTER: zero correlation sum = {}",
        zero_corr.iter().sum::<f32>()
    );
    assert!(zero_corr.iter().all(|v| *v == 0.0));

    let mut clamped = [0.5f32; NUM_EMBEDDERS];
    clamped[0] = 2.0;
    clamped[1] = -1.0;
    println!("EDGE BEFORE: clamped profile = {:?}", clamped);
    let clamped_corr = compute_cross_correlations(&clamped, &synergy);
    let p01 = pair_index(0, 1);
    let p02 = pair_index(0, 2);
    println!(
        "EDGE AFTER: pair(0,1)={}, pair(0,2)={}",
        clamped_corr[p01], clamped_corr[p02]
    );
    assert_close(clamped_corr[p01], 0.0, "negative input clamps to zero");
    assert_close(
        clamped_corr[p02],
        1.0 * 0.5 * synergy.get_synergy(0, 2),
        "above-one input clamps to one",
    );

    println!("EDGE BEFORE: invalid synergy index get_synergy(14, 0)");
    let invalid = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let _ = synergy.get_synergy(14, 0);
    }));
    println!(
        "EDGE AFTER: invalid synergy panic observed = {}",
        invalid.is_err()
    );
    assert!(invalid.is_err());
}

#[test]
fn fsv_group_weight_and_topic_profile_calculations() {
    section("group alignments, weighted agreement, fallback topic profiles");

    let mut profile = [0.0f32; NUM_EMBEDDERS];
    for (i, value) in profile.iter_mut().enumerate() {
        *value = i as f32 + 1.0;
    }
    println!("BEFORE: group input profile = {:?}", profile);

    let groups = compute_group_alignments(&profile);
    let expected = [
        group_indices::FACTUAL
            .iter()
            .map(|i| profile[*i])
            .sum::<f32>()
            / group_indices::FACTUAL.len() as f32,
        group_indices::TEMPORAL
            .iter()
            .map(|i| profile[*i])
            .sum::<f32>()
            / group_indices::TEMPORAL.len() as f32,
        group_indices::CAUSAL
            .iter()
            .map(|i| profile[*i])
            .sum::<f32>()
            / group_indices::CAUSAL.len() as f32,
        group_indices::RELATIONAL
            .iter()
            .map(|i| profile[*i])
            .sum::<f32>()
            / group_indices::RELATIONAL.len() as f32,
        group_indices::QUALITATIVE
            .iter()
            .map(|i| profile[*i])
            .sum::<f32>()
            / group_indices::QUALITATIVE.len() as f32,
        group_indices::IMPLEMENTATION
            .iter()
            .map(|i| profile[*i])
            .sum::<f32>()
            / group_indices::IMPLEMENTATION.len() as f32,
    ];
    println!("AFTER: groups = {:?}, expected = {:?}", groups, expected);
    assert_eq!(groups, expected);

    let ga = GroupAlignments::from_array(groups);
    let manual_average = groups.iter().sum::<f32>() / groups.len() as f32;
    let manual_variance = groups
        .iter()
        .map(|v| (*v - manual_average).powi(2))
        .sum::<f32>()
        / groups.len() as f32;
    let manual_std_dev = manual_variance.sqrt();
    let manual_coherence = 1.0 / (1.0 + manual_std_dev);
    println!(
        "AFTER: group average={} std_dev={} coherence={} dominant={:?} weakest={:?}",
        ga.average(),
        ga.std_dev(),
        ga.coherence(),
        ga.dominant_group(),
        ga.weakest_group()
    );
    assert_close(ga.average(), manual_average, "group average");
    assert_close(ga.std_dev(), manual_std_dev, "group std_dev");
    assert_close(ga.coherence(), manual_coherence, "group coherence");

    let mut weight_sum = 0.0f32;
    let mut category_counts = HashMap::new();
    for embedder in Embedder::all() {
        let category = category_for(embedder);
        let weight = category.topic_weight();
        *category_counts
            .entry(category.to_string())
            .or_insert(0usize) += 1;
        weight_sum += weight;
        println!(
            "AFTER: {} category={} topic_weight={}",
            embedder.name(),
            category,
            weight
        );
    }
    println!(
        "AFTER: category_counts={:?}, weight_sum={}, max_weighted_agreement()={}, MAX_WEIGHTED_AGREEMENT={}",
        category_counts,
        weight_sum,
        max_weighted_agreement(),
        MAX_WEIGHTED_AGREEMENT
    );
    assert_close(weight_sum, 9.5, "manual topic weight sum");
    assert_close(
        max_weighted_agreement(),
        9.5,
        "category max weighted agreement",
    );
    assert_close(
        MAX_WEIGHTED_AGREEMENT,
        9.5,
        "clustering max weighted agreement",
    );

    let complete_fp = complete_semantic_fingerprint();
    println!("BEFORE: complete fingerprint has unit signal in all 14 slots");
    let fallback = topic_profile_or_fallback(None, &complete_fp);
    println!("AFTER: fallback profile = {:?}", fallback);
    for (i, value) in fallback.iter().enumerate() {
        assert_close(*value, 1.0, &format!("fallback slot {}", i));
    }

    let stored = [0.25f32; NUM_EMBEDDERS];
    println!("EDGE BEFORE: stored profile override = {:?}", stored);
    let override_profile = topic_profile_or_fallback(Some(stored), &empty_semantic_fingerprint());
    println!("EDGE AFTER: override profile = {:?}", override_profile);
    assert_eq!(override_profile, stored);

    let empty = empty_semantic_fingerprint();
    println!("EDGE BEFORE: empty fingerprint");
    let empty_profile = topic_profile_or_fallback(None, &empty);
    println!("EDGE AFTER: empty fallback profile = {:?}", empty_profile);
    assert!(empty_profile.iter().all(|v| *v == 0.0));

    let mut invalid = complete_semantic_fingerprint();
    invalid.e1_semantic[0] = f32::NAN;
    println!("EDGE BEFORE: E1 contains NaN");
    let invalid_profile = topic_profile_or_fallback(None, &invalid);
    println!(
        "EDGE AFTER: invalid fallback profile[0] = {}",
        invalid_profile[0]
    );
    assert_eq!(invalid_profile[0], 0.0);

    assert_eq!(E6_SPARSE_VOCAB, E13_SPLADE_VOCAB);
}
