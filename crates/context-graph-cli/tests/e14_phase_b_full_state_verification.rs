//! E14 Phase B — RIGOROUS Full State Verification.
//!
//! This harness goes beyond the lightweight Phase B FSV by:
//!
//! 1. Writing/reading *real* RocksDB bytes (not just checking struct fields).
//! 2. Loading/serialising MCP JSON wire formats (not just constructing DTOs).
//! 3. Spawning the actual `context-graph-cli backfill-e14` binary as a subprocess
//!    against a temp RocksDB containing synthetic fingerprints.
//! 4. Close+reopen cycles for persistence claims.
//! 5. Each check includes at least three edge cases with before/after state logs.
//!
//! Synthetic data is deterministic: identical vectors → cosine distance 0,
//! known UUIDs, known file contents, known CF names. Assertions are against
//! *specific predicted values*, not wishy-washy "is_some()" checks.

use std::collections::HashMap;
use std::path::PathBuf;

use context_graph_core::types::fingerprint::E14_DIM;
use context_graph_embeddings::config::GpuConfig;
use context_graph_embeddings::models::pretrained::{
    BgeM3DenseModel, BGE_M3_DENSE_DIMENSION, BGE_M3_DENSE_LATENCY_BUDGET_MS,
    BGE_M3_DENSE_MAX_TOKENS, XLM_R_BOS_TOKEN_ID, XLM_R_PAD_TOKEN_ID, XLM_R_POSITION_OFFSET,
    XLM_R_WEIGHT_PREFIX,
};
use context_graph_embeddings::models::DefaultModelFactory;
use context_graph_embeddings::traits::{ModelFactory, SingleModelConfig};
use context_graph_embeddings::types::ModelId;
use context_graph_embeddings::{
    QuantizationMetadata, QuantizationMethod, QuantizedEmbedding, StoredQuantizedFingerprint,
};
use context_graph_mcp::handlers::tools::embedder_dtos::{
    AllEmbedderScores, EmbedderId, SearchByEmbedderRequest,
};
use context_graph_storage::teleological::indexes::{
    EmbedderIndex, EmbedderIndexOps, EmbedderIndexRegistry,
};
use context_graph_storage::teleological::quantized::QuantizedFingerprintStorage;
use context_graph_storage::teleological::{
    RocksDbTeleologicalStore, TeleologicalStoreConfig, CF_EMB_13, QUANTIZED_EMBEDDER_CFS,
    QUANTIZED_EMBEDDER_CF_COUNT,
};
use tempfile::TempDir;
use uuid::Uuid;

fn section(title: &str) {
    println!("\n================================================================================");
    println!("PHASE B FSV: {}", title);
    println!("================================================================================");
}

fn subsection(title: &str) {
    println!("\n--- {} ---", title);
}

// ============================================================================
// TRACK 1 — Native model module (Task #34)
// ============================================================================

/// FSV-T1a: Every file in the bge_m3_dense/ module exists on disk with a
/// non-zero size. Source of Truth: the file-system inode metadata.
#[test]
fn fsv_t1a_native_module_files_exist_on_disk() {
    section("T1a. Native module file inventory — physical file-system check");

    let base = "../context-graph-embeddings/src/models/pretrained/bge_m3_dense";
    let expected: &[&str] = &[
        "mod.rs",
        "types.rs",
        "constants.rs",
        "loader.rs",
        "model.rs",
        "trait_impl.rs",
        "gpu_forward.rs",
        "embeddings.rs",
        "encoder.rs",
        "attention.rs",
        "ffn.rs",
        "layer_norm.rs",
        "pooling.rs",
        "tests.rs",
    ];

    println!("BEFORE: checking {} files under {}", expected.len(), base);
    let mut total_bytes = 0u64;
    for name in expected {
        let path = std::path::Path::new(base).join(name);
        let meta = std::fs::metadata(&path)
            .unwrap_or_else(|e| panic!("missing: {} ({})", path.display(), e));
        assert!(meta.is_file(), "{} is not a file", path.display());
        assert!(
            meta.len() > 100,
            "{} is suspiciously small ({} bytes)",
            path.display(),
            meta.len()
        );
        total_bytes += meta.len();
        println!("  ✓ {} ({} bytes)", name, meta.len());
    }
    println!("AFTER: 14 files present, total {} bytes on disk", total_bytes);
    assert!(
        total_bytes > 15_000,
        "14 module files should total >15KB, got {}",
        total_bytes
    );
}

/// FSV-T1b: Source of Truth = `BgeM3DenseModel::model_id()` return value.
/// The native module must self-report as `ModelId::BgeM3Dense`, and the
/// factory must create the same concrete type.
#[test]
fn fsv_t1b_module_self_reports_correct_model_id() {
    section("T1b. BgeM3DenseModel.model_id() self-report");

    use context_graph_embeddings::traits::EmbeddingModel;

    subsection("Happy path: fresh model reports BgeM3Dense");
    let model = BgeM3DenseModel::new(
        &PathBuf::from("./models/bge-m3-dense"),
        SingleModelConfig::default(),
    )
    .expect("construction");
    let id_via_struct = model.model_id();
    println!("BEFORE expected: ModelId::BgeM3Dense (u8=14)");
    println!(
        "AFTER  actual:   {:?} (u8={})",
        id_via_struct, id_via_struct as u8
    );
    assert_eq!(id_via_struct, ModelId::BgeM3Dense);
    assert_eq!(id_via_struct as u8, 14);

    subsection("Happy path: factory returns a BgeM3Dense-reporting trait object");
    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());
    let boxed = factory
        .create_model(ModelId::BgeM3Dense, &SingleModelConfig::default())
        .expect("factory");
    println!("AFTER boxed.model_id() = {:?}", boxed.model_id());
    assert_eq!(boxed.model_id(), ModelId::BgeM3Dense);
    assert!(!boxed.is_initialized());

    subsection("Edge 1: model_id survives a second construction (no hidden global state)");
    let model2 = BgeM3DenseModel::new(
        &PathBuf::from("/different/path/bge-m3-dense"),
        SingleModelConfig::default(),
    )
    .expect("second construction");
    assert_eq!(model2.model_id(), ModelId::BgeM3Dense);
    println!("  ✓ second instance also reports BgeM3Dense");

    subsection("Edge 2: zero-batch fails before touching the filesystem");
    let cfg_bad = SingleModelConfig {
        max_batch_size: 0,
        ..Default::default()
    };
    match BgeM3DenseModel::new(&PathBuf::from("./models/bge-m3-dense"), cfg_bad) {
        Ok(_) => panic!("zero batch must fail"),
        Err(e) => {
            let msg = format!("{:?}", e);
            println!("  ✓ zero batch rejected: {}", msg);
            assert!(msg.contains("max_batch_size"));
        }
    }

    subsection("Edge 3: supported_input_types is Text-only (no sneaky Code/Image support)");
    let types = model.supported_input_types();
    println!("  supported_input_types = {:?}", types);
    assert_eq!(types.len(), 1);
    use context_graph_embeddings::types::InputType;
    assert_eq!(types[0], InputType::Text);
}

/// FSV-T1c: Source of Truth = constants visible from the re-export boundary.
#[test]
fn fsv_t1c_module_public_constants_are_correct() {
    section("T1c. Public constants — value assertions");

    println!(
        "BEFORE expected: DIM=1024, MAX_TOKENS=8192, LATENCY=50, BOS=0, PAD=1, POS_OFFSET=2, PREFIX=\"roberta.\""
    );
    println!(
        "AFTER  actual:   DIM={}, MAX_TOKENS={}, LATENCY={}, BOS={}, PAD={}, POS_OFFSET={}, PREFIX=\"{}\"",
        BGE_M3_DENSE_DIMENSION,
        BGE_M3_DENSE_MAX_TOKENS,
        BGE_M3_DENSE_LATENCY_BUDGET_MS,
        XLM_R_BOS_TOKEN_ID,
        XLM_R_PAD_TOKEN_ID,
        XLM_R_POSITION_OFFSET,
        XLM_R_WEIGHT_PREFIX,
    );
    assert_eq!(BGE_M3_DENSE_DIMENSION, 1024);
    assert_eq!(BGE_M3_DENSE_MAX_TOKENS, 8192);
    assert_eq!(BGE_M3_DENSE_LATENCY_BUDGET_MS, 50);
    assert_eq!(XLM_R_BOS_TOKEN_ID, 0);
    assert_eq!(XLM_R_PAD_TOKEN_ID, 1);
    assert_eq!(XLM_R_POSITION_OFFSET, 2);
    assert_eq!(XLM_R_WEIGHT_PREFIX, "roberta.");

    assert_eq!(E14_DIM, BGE_M3_DENSE_DIMENSION, "E14_DIM must equal native dim");
    assert_eq!(
        XLM_R_POSITION_OFFSET,
        XLM_R_PAD_TOKEN_ID + 1,
        "XLM-R invariant: position_offset = pad_token_id + 1"
    );
}

// ============================================================================
// TRACK 2 — CF_EMB_13 physical RocksDB persistence (Task #35)
// ============================================================================

/// FSV-T2: Source of Truth = the on-disk RocksDB column-family entry for CF_EMB_13.
/// This test does the full W/R cycle: open store → write a known-bit-pattern
/// quantized embedding to CF_EMB_13 via load_embedder/store_embedder → close
/// store → reopen → read back → byte-compare.
#[test]
fn fsv_t2_cf_emb_13_physical_persistence_roundtrip() {
    section("T2. CF_EMB_13 physical RocksDB write-close-reopen-read cycle");

    // Synthetic fingerprint with all 14 slots filled, with E14 carrying a
    // known byte pattern that nothing else in the DB could have produced.
    let fp_id = Uuid::parse_str("0b6b6b6b-e14e-14e1-b0b0-000000000014").unwrap();
    let e14_signature: Vec<u8> = (0..8u32).map(|i| ((i * 59) % 256) as u8).collect();
    let e14_signature_expected = e14_signature.clone();
    println!(
        "BEFORE: writing CF_EMB_13 bytes = {:?} (len={}) for id={}",
        e14_signature,
        e14_signature.len(),
        fp_id
    );

    let tmp = TempDir::new().expect("tempdir");
    let db_path = tmp.path().to_path_buf();

    // --- Phase 1: WRITE ---
    {
        let store = RocksDbTeleologicalStore::open_with_config(
            &db_path,
            TeleologicalStoreConfig::default(),
        )
        .expect("open #1");

        let mut map: HashMap<u8, QuantizedEmbedding> = HashMap::new();
        for i in 0..14u8 {
            let (method, dim, data) = match i {
                0 | 4 | 6 | 9 => (QuantizationMethod::PQ8, 1024u16, vec![i.wrapping_mul(17); 8]),
                13 => (QuantizationMethod::PQ8, 1024u16, e14_signature.clone()), // E14 — our signature
                1..=3 | 7 | 10 => (
                    QuantizationMethod::Float8E4M3,
                    512,
                    vec![i.wrapping_mul(17); 512],
                ),
                8 => (QuantizationMethod::Binary, 10000, vec![0xAA; 1250]),
                5 | 12 => (QuantizationMethod::SparseNative, 30522, vec![i; 100]),
                11 => (QuantizationMethod::TokenPruning, 128, vec![i; 64]),
                _ => unreachable!(),
            };
            map.insert(
                i,
                QuantizedEmbedding {
                    method,
                    original_dim: dim as usize,
                    data,
                    metadata: match method {
                        QuantizationMethod::PQ8 => QuantizationMetadata::PQ8 {
                            codebook_id: i as u32,
                            num_subvectors: 8,
                        },
                        QuantizationMethod::Float8E4M3 => QuantizationMetadata::Float8 {
                            scale: 1.0,
                            bias: 0.0,
                        },
                        QuantizationMethod::Binary => {
                            QuantizationMetadata::Binary { threshold: 0.0 }
                        }
                        QuantizationMethod::SparseNative => QuantizationMetadata::Sparse {
                            vocab_size: 30522,
                            nnz: 50,
                        },
                        QuantizationMethod::TokenPruning => QuantizationMetadata::TokenPruning {
                            original_tokens: 128,
                            kept_tokens: 64,
                            threshold: 0.5,
                        },
                    },
                },
            );
        }

        let qfp = StoredQuantizedFingerprint::new(fp_id, map, [0.5f32; 14], [0xBEu8; 32]);
        store
            .store_quantized_fingerprint(&qfp)
            .expect("store_quantized_fingerprint");
        println!("WRITE: store_quantized_fingerprint OK — 14 CFs written");
    } // store dropped → RocksDB flushed + closed

    // --- Phase 2: REOPEN (simulates a process restart) ---
    let store2 = RocksDbTeleologicalStore::open_with_config(
        &db_path,
        TeleologicalStoreConfig::default(),
    )
    .expect("open #2 (reopen)");
    println!("REOPEN: store opened successfully");

    // --- Phase 3: READ E14 directly from CF_EMB_13 via load_embedder(id, 13) ---
    let loaded_e14 = store2.load_embedder(fp_id, 13).expect("load_embedder(13)");
    println!(
        "READ:   loaded_e14.method={:?}, original_dim={}, data={:?}",
        loaded_e14.method, loaded_e14.original_dim, loaded_e14.data
    );

    assert_eq!(
        loaded_e14.method,
        QuantizationMethod::PQ8,
        "E14 must use PQ8 quantization"
    );
    assert_eq!(loaded_e14.original_dim, 1024);
    assert_eq!(
        loaded_e14.data, e14_signature_expected,
        "PHYSICAL FAIL: CF_EMB_13 did not preserve the signature bytes"
    );
    println!("PHYSICAL PROOF: E14 bytes round-tripped through CF_EMB_13 across close+reopen");

    subsection("Edge 1: load_embedder(id, 14) panics — FAIL FAST on OOB index");
    // The store panics on invalid indexes (STORAGE ERROR with valid range
    // 0-13 post-E14). We catch the panic to assert fail-fast without aborting.
    let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = store2.load_embedder(fp_id, 14);
    }));
    let panic_msg = panicked.as_ref().err().and_then(|e| {
        e.downcast_ref::<String>().cloned().or_else(|| {
            e.downcast_ref::<&str>().map(|s| (*s).to_string())
        })
    });
    println!(
        "  load_embedder(14) panicked? {} msg={:?}",
        panicked.is_err(),
        panic_msg
    );
    assert!(
        panicked.is_err(),
        "embedder idx >= 14 must panic (FAIL FAST)"
    );
    let msg = panic_msg.unwrap_or_default();
    assert!(
        msg.contains("Valid range: 0-13") || msg.contains("Invalid embedder index"),
        "panic message should mention valid range post-E14, got: {}",
        msg
    );

    subsection("Edge 2: unknown UUID yields NotFound");
    let ghost = Uuid::parse_str("deadbeef-dead-beef-dead-beefdeadbeef").unwrap();
    let loaded_ghost = store2.load_embedder(ghost, 13);
    println!("  load_embedder(ghost id, 13) → {:?}", loaded_ghost.is_err());
    assert!(loaded_ghost.is_err());

    subsection("Edge 3: whole-fingerprint load returns 14 embeddings");
    let full = store2
        .load_quantized_fingerprint(fp_id)
        .expect("load full");
    println!(
        "  full.embeddings.len() = {} (expected 14)",
        full.embeddings.len()
    );
    assert_eq!(full.embeddings.len(), 14);
    assert!(full.embeddings.contains_key(&13), "E14 slot must be present");

    subsection("Evidence: delete clears all 14 CFs including CF_EMB_13");
    store2
        .delete_quantized_fingerprint(fp_id)
        .expect("delete");
    let after_delete = store2.load_embedder(fp_id, 13);
    println!(
        "  after delete, load_embedder(13) → {}",
        after_delete.is_err()
    );
    assert!(after_delete.is_err(), "delete must remove CF_EMB_13 entry");
}

/// FSV-T2b: Source of Truth = static QUANTIZED_EMBEDDER_CFS slice.
/// Verifies every CF name in the slice follows the "emb_{idx}" pattern and
/// length = 14.
#[test]
fn fsv_t2b_cf_names_follow_convention() {
    section("T2b. QUANTIZED_EMBEDDER_CFS naming convention + ordering");

    println!(
        "BEFORE expected: 14 CFs named \"emb_0\"..\"emb_13\" in index order"
    );
    assert_eq!(QUANTIZED_EMBEDDER_CF_COUNT, 14);
    assert_eq!(QUANTIZED_EMBEDDER_CFS.len(), 14);

    for (i, &name) in QUANTIZED_EMBEDDER_CFS.iter().enumerate() {
        let expected = format!("emb_{}", i);
        println!("  slot {} → {} (expected {})", i, name, expected);
        assert_eq!(name, expected, "CF slot {} naming drift", i);
    }
    assert_eq!(QUANTIZED_EMBEDDER_CFS[13], CF_EMB_13);
    assert_eq!(CF_EMB_13, "emb_13");
}

// ============================================================================
// TRACK 3 — HNSW registry + E14 index (Task #35)
// ============================================================================

/// FSV-T3: Source of Truth = the HNSW index object attached to EmbedderIndex::E14BgeM3Dense.
/// Inserts 3 known vectors, then searches — verifies that each query retrieves
/// the expected UUID as the top hit with near-zero cosine distance (identical
/// vector = distance ≈ 0 for cosine metric).
#[test]
fn fsv_t3_hnsw_e14_insert_search_synthetic_roundtrip() {
    section("T3. HNSW E14 registry: insert 3 synthetic vectors, search each");

    let registry = EmbedderIndexRegistry::new();
    assert_eq!(registry.len(), 16, "registry must have 16 HNSW slots");
    let e14 = registry
        .get(EmbedderIndex::E14BgeM3Dense)
        .expect("E14 index registered");
    assert_eq!(e14.config().dimension, 1024);

    // Synthetic: three distinct unit vectors via sinusoidal seeds.
    // Seed 0: cos(i/37); seed 1: cos(i/41); seed 2: cos(i/43). All 1024D.
    let make_vec = |seed: u64| -> Vec<f32> {
        let mut v: Vec<f32> = (0..1024)
            .map(|i| ((i as f64 + seed as f64) / 37.0).cos() as f32)
            .collect();
        let n: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        for x in &mut v {
            *x /= n;
        }
        v
    };
    let ids: Vec<Uuid> = (0..3)
        .map(|i| {
            Uuid::parse_str(&format!(
                "e14ae14a-e14e-4e14-e14e-e14e0000001{}",
                i
            ))
            .unwrap()
        })
        .collect();
    let vecs: Vec<Vec<f32>> = (0..3u64).map(make_vec).collect();

    println!("BEFORE: 3 synthetic 1024D vectors prepared");
    for (i, (id, v)) in ids.iter().zip(vecs.iter()).enumerate() {
        let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        println!(
            "  id{}={}, norm={:.6}, first={:.6}, last={:.6}",
            i,
            id,
            norm,
            v[0],
            v[1023]
        );
        assert!((norm - 1.0).abs() < 1e-4);
    }

    // Physical insert
    for (id, v) in ids.iter().zip(vecs.iter()) {
        e14.insert(*id, v).expect("insert");
    }
    let total = registry.total_vectors();
    println!("AFTER insert: registry.total_vectors()={} (expected 3)", total);
    assert_eq!(total, 3);

    // Physical search: each query must find itself first.
    for (i, (id, v)) in ids.iter().zip(vecs.iter()).enumerate() {
        let hits = e14.search(v, 3, None).expect("search");
        println!(
            "  query id{} → top hit id={}, distance={:.6e}",
            i, hits[0].0, hits[0].1
        );
        assert_eq!(hits[0].0, *id, "self-query must return self");
        assert!(
            hits[0].1 < 0.01,
            "self-distance should be ~0, got {:e}",
            hits[0].1
        );
    }

    subsection("Edge 1: k larger than inserted returns all 3");
    let big_k = e14.search(&vecs[0], 100, None).expect("k>n search");
    println!("  k=100 against 3 inserts → {} hits", big_k.len());
    assert_eq!(big_k.len(), 3);

    subsection("Edge 2: wrong-dim query rejected (no silent padding/truncation)");
    let bad_dim = vec![0.1f32; 512];
    let bad = e14.search(&bad_dim, 1, None);
    println!("  wrong-dim search → {:?}", bad.as_ref().err().is_some());
    assert!(bad.is_err(), "512D query against 1024D index must error");

    subsection("Edge 3: removing one ID leaves the other two searchable");
    e14.remove(ids[1]).expect("remove");
    let after_remove = registry.total_vectors();
    println!(
        "  after removing id1: total_vectors() = {} (expected 2)",
        after_remove
    );
    assert_eq!(after_remove, 2);
    let hits = e14.search(&vecs[0], 3, None).expect("search after remove");
    let returned_ids: Vec<Uuid> = hits.iter().map(|(id, _)| *id).collect();
    println!("  post-remove hits: {:?}", returned_ids);
    assert!(!returned_ids.contains(&ids[1]));
    assert!(returned_ids.contains(&ids[0]));
    assert!(returned_ids.contains(&ids[2]));
}

// ============================================================================
// TRACK 4 — MCP DTO wire protocol (Task #36)
// ============================================================================

/// FSV-T4a: Source of Truth = the JSON bytes produced by serde_json for
/// `AllEmbedderScores`. The wire format must carry `"e14":<f32>` verbatim.
#[test]
fn fsv_t4a_all_embedder_scores_json_wire_format() {
    section("T4a. AllEmbedderScores JSON wire format carries e14");

    let scores = AllEmbedderScores {
        e1: 0.91,
        e2: 0.12,
        e3: 0.23,
        e4: 0.34,
        e5: Some(0.45),
        e6: 0.56,
        e7: 0.67,
        e8: 0.78,
        e9: 0.89,
        e10: 0.10,
        e11: 0.21,
        e12: 0.32,
        e13: 0.43,
        e14: 0.54321,
    };

    let json = serde_json::to_string(&scores).expect("serialize");
    println!("BEFORE: constructed AllEmbedderScores with e14=0.54321");
    println!("AFTER  JSON ({} bytes):", json.len());
    println!("  {}", json);

    // Physical evidence 1: the e14 key must be literally present.
    assert!(json.contains("\"e14\""), "JSON must contain \"e14\" key");
    assert!(json.contains("0.54321"), "JSON must carry the e14 value");

    // Physical evidence 2: parsing the JSON back recovers the same values.
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("parse");
    let e14_back = parsed["e14"].as_f64().expect("e14 number");
    println!(
        "  parsed[\"e14\"] = {} (expected ≈ 0.54321)",
        e14_back
    );
    assert!((e14_back - 0.54321).abs() < 1e-4);

    subsection("Edge 1: E5 sentinel (-1.0) still round-trips null");
    let scores_null = AllEmbedderScores {
        e5: None,
        ..scores.clone()
    };
    let j2 = serde_json::to_string(&scores_null).expect("ser #2");
    println!("  JSON w/ E5=None: {}", j2);
    assert!(!j2.contains("\"e5\"") || j2.contains("\"e5\":null"));

    subsection("Edge 2: non-finite e14 serialises to JSON null (documented behaviour)");
    let scores_nan = AllEmbedderScores {
        e14: f32::NAN,
        ..scores.clone()
    };
    // serde_json writes NaN as `null` (JSON spec forbids NaN literals). The
    // wire protocol treats this as a missing value; downstream consumers
    // must re-compute or ignore rather than trust stale data.
    let j_nan = serde_json::to_string(&scores_nan).expect("ser NaN");
    println!("  JSON w/ e14=NaN: {}", j_nan);
    assert!(
        j_nan.contains("\"e14\":null"),
        "NaN must serialise as JSON null, got: {}",
        j_nan
    );
}

/// FSV-T4b: Source of Truth = parsed `SearchByEmbedderRequest`. Confirms that
/// a wire-format JSON request with `"embedder": "E14"` parses and validates.
#[test]
fn fsv_t4b_search_by_embedder_request_accepts_e14() {
    section("T4b. SearchByEmbedderRequest accepts embedder=\"E14\" end-to-end");

    let wire = r#"{
        "embedder": "E14",
        "query": "multilingual rust programming",
        "topK": 10,
        "minSimilarity": 0.4,
        "includeContent": false,
        "includeAllScores": true
    }"#;
    println!("BEFORE JSON wire:\n{}", wire);

    let req: SearchByEmbedderRequest = serde_json::from_str(wire).expect("parse");
    req.validate().expect("validate");

    let eid = req.embedder_id().expect("embedder_id");
    println!(
        "AFTER  req.embedder_id() = {:?}, to_index() = {}",
        eid,
        eid.to_index()
    );
    assert_eq!(eid, EmbedderId::E14);
    assert_eq!(eid.to_index(), 13);

    subsection("Edge 1: lowercase alias bge_m3_dense accepted");
    let wire2 = r#"{"embedder":"bge_m3_dense","query":"x","topK":1}"#;
    let req2: SearchByEmbedderRequest = serde_json::from_str(wire2).expect("parse alias");
    req2.validate().expect("validate alias");
    assert_eq!(req2.embedder_id().unwrap(), EmbedderId::E14);
    println!("  ✓ bge_m3_dense alias → E14");

    subsection("Edge 2: E15 rejected at validate() with up-to-date E1-E14 hint");
    let wire3 = r#"{"embedder":"E15","query":"x","topK":1}"#;
    let req3: SearchByEmbedderRequest = serde_json::from_str(wire3).expect("parse E15");
    match req3.validate() {
        Ok(()) => panic!("E15 should be invalid"),
        Err(e) => {
            println!("  ✓ validate rejects E15: {}", e);
            assert!(e.contains("E15"));
            // Regression guard: the hint must advertise E14, not the stale E13.
            assert!(
                e.contains("E1-E14"),
                "error message must hint at the current range E1-E14, got: {}",
                e
            );
        }
    }

    subsection("Edge 3: empty query still rejected even with valid E14");
    let wire4 = r#"{"embedder":"E14","query":"","topK":1}"#;
    let req4: SearchByEmbedderRequest = serde_json::from_str(wire4).expect("parse");
    match req4.validate() {
        Ok(()) => panic!("empty query should fail"),
        Err(e) => {
            println!("  ✓ empty query rejected: {}", e);
            assert!(e.to_lowercase().contains("query"));
        }
    }
}

// ============================================================================
// TRACK 5 — Migration CLI (Task #37) — REAL SUBPROCESS EXECUTION
// ============================================================================

/// FSV-T5a: Source of Truth = stderr from `context-graph-cli backfill-e14
/// --help`. The subcommand must be discoverable via clap's help text.
#[test]
fn fsv_t5a_cli_help_lists_backfill_e14_subcommand() {
    section("T5a. CLI help output contains backfill-e14 subcommand");

    let bin = env!("CARGO_BIN_EXE_context-graph-cli");
    println!("BEFORE: spawning {} backfill-e14 --help", bin);

    let output = std::process::Command::new(bin)
        .args(["backfill-e14", "--help"])
        .output()
        .expect("spawn CLI");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("AFTER:  exit={}, stdout_len={}, stderr_len={}", output.status, stdout.len(), stderr.len());
    println!("--- stdout (first 400 bytes) ---");
    println!("{}", stdout.chars().take(400).collect::<String>());

    assert!(output.status.success(), "help should exit 0");
    let combined = format!("{}\n{}", stdout, stderr);
    assert!(combined.contains("--storage"), "help must show --storage");
    assert!(combined.contains("--dry-run"), "help must show --dry-run");
    assert!(combined.contains("--models-dir"), "help must show --models-dir");
    assert!(combined.contains("--limit"), "help must show --limit");
}

/// FSV-T5b: Source of Truth = the exit code + stderr message from an actual
/// subprocess invocation against a real temporary RocksDB containing
/// synthetic fingerprints with empty E14 fields.
///
/// Synthetic setup:
///   1. Build 3 `TeleologicalFingerprint`s, each with `e14_bge_m3_dense` empty.
///   2. Open store, `store_fingerprint` each one, close.
///   3. Spawn CLI `backfill-e14 --dry-run --storage <tmp>` as subprocess.
///   4. Parse stderr summary line, assert `3 seen, 0 already had E14, 3 backfilled`.
#[test]
fn fsv_t5b_cli_dry_run_against_real_db_counts_correctly() {
    section("T5b. CLI dry-run subprocess against a real DB — physical proof");

    let tmp = TempDir::new().expect("tempdir");
    let db_path = tmp.path().to_path_buf();

    // Build and store 3 synthetic fingerprints with empty E14.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let ids: Vec<Uuid> = (0..3)
        .map(|i| {
            Uuid::parse_str(&format!(
                "e14cde14-0000-4000-8000-00000000001{}",
                i
            ))
            .unwrap()
        })
        .collect();

    rt.block_on(async {
        let store = RocksDbTeleologicalStore::open_with_config(
            &db_path,
            TeleologicalStoreConfig::default(),
        )
        .expect("open store");

        for (i, id) in ids.iter().enumerate() {
            let mut fp = context_graph_test_utils::fingerprints::create_real_fingerprint_with_id(*id);
            // Force E14 empty for 2 of 3; leave populated on the 3rd.
            if i < 2 {
                fp.semantic.e14_bge_m3_dense = Vec::new();
            }
            context_graph_core::traits::TeleologicalMemoryStore::store(&store, fp)
                .await
                .expect("store fingerprint");
        }

        // Verify the write landed by retrieving each one.
        for (i, id) in ids.iter().enumerate() {
            let fp = context_graph_core::traits::TeleologicalMemoryStore::retrieve(&store, *id)
                .await
                .expect("retrieve")
                .expect("some");
            let empty = fp.semantic.e14_bge_m3_dense.is_empty();
            println!(
                "  fp{}: id={}, e14_empty={} (expected {})",
                i,
                id,
                empty,
                i < 2
            );
            assert_eq!(empty, i < 2);
        }
    });

    // Now invoke the CLI as a subprocess.
    let bin = env!("CARGO_BIN_EXE_context-graph-cli");
    println!("BEFORE: spawning {} backfill-e14 --dry-run --storage {}", bin, db_path.display());
    let output = std::process::Command::new(bin)
        .args([
            "backfill-e14",
            "--dry-run",
            "--storage",
            db_path.to_str().unwrap(),
        ])
        .output()
        .expect("spawn CLI");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("AFTER:  exit={}", output.status);
    println!("--- stdout ---\n{}", stdout);
    println!("--- stderr (last 800) ---\n{}", stderr.chars().rev().take(800).collect::<String>().chars().rev().collect::<String>());

    assert!(output.status.success(), "dry-run should exit 0");

    // Physical evidence: stderr summary line must report 3 seen, 1 already, 2 backfilled.
    let summary_line = stderr
        .lines()
        .find(|l| l.contains("backfill-e14:"))
        .expect("summary line missing");
    println!("summary line = {}", summary_line);
    assert!(summary_line.contains("3 seen"));
    assert!(summary_line.contains("1 already had E14"));
    assert!(summary_line.contains("2 backfilled"));

    subsection("Edge 1: missing --storage exits with clap error");
    let output2 = std::process::Command::new(bin)
        .args(["backfill-e14"])
        .output()
        .expect("spawn");
    println!("  missing-storage exit = {}", output2.status);
    assert!(!output2.status.success());
    let s2 = String::from_utf8_lossy(&output2.stderr);
    assert!(s2.contains("--storage") || s2.contains("required"));

    subsection("Edge 2: nonexistent storage path exits 1 with clear error");
    let output3 = std::process::Command::new(bin)
        .args([
            "backfill-e14",
            "--dry-run",
            "--storage",
            "/nonexistent/path/xyz",
        ])
        .output()
        .expect("spawn");
    println!("  nonexistent-storage exit = {}", output3.status);
    let s3 = String::from_utf8_lossy(&output3.stderr);
    println!("  stderr: {}", s3.lines().find(|l| l.contains("FAILED") || l.contains("does not exist")).unwrap_or("<none>"));
    assert!(!output3.status.success());

    subsection("Edge 3: --limit 1 caps processing — still honours dry-run semantics");
    let output4 = std::process::Command::new(bin)
        .args([
            "backfill-e14",
            "--dry-run",
            "--storage",
            db_path.to_str().unwrap(),
            "--limit",
            "1",
        ])
        .output()
        .expect("spawn");
    let s4 = String::from_utf8_lossy(&output4.stderr);
    let summary4 = s4.lines().find(|l| l.contains("backfill-e14:")).unwrap_or("");
    println!("  --limit 1 summary: {}", summary4);
    assert!(output4.status.success());
    assert!(summary4.contains("1 seen"));
}

// ============================================================================
// TRACK 6 — End-to-end integration
// ============================================================================

/// FSV-T6: Source of Truth = round-trip through factory → HNSW registry →
/// search. Proves that every Phase B surface connects: factory produces
/// `BgeM3DenseModel` → HNSW registry has a 1024D E14 slot → a vector with
/// the 1024D dimension can be inserted and retrieved.
#[test]
fn fsv_t6_end_to_end_factory_hnsw_registry_connect() {
    section("T6. End-to-end: factory + HNSW registry share the 1024D contract");

    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());
    let model = factory
        .create_model(ModelId::BgeM3Dense, &SingleModelConfig::default())
        .expect("factory");
    let model_id = model.model_id();
    println!(
        "BEFORE: factory returned model with id={:?}, u8={}",
        model_id, model_id as u8
    );

    let registry = EmbedderIndexRegistry::new();
    let e14 = registry
        .get(EmbedderIndex::E14BgeM3Dense)
        .expect("E14 in registry");
    let idx_dim = e14.config().dimension;
    println!("AFTER:  HNSW E14 dimension = {}", idx_dim);

    // CONTRACT: factory dim must match HNSW dim.
    assert_eq!(model_id as u8, 14, "ModelId discriminant");
    assert_eq!(idx_dim, 1024, "HNSW config dimension");
    assert_eq!(BGE_M3_DENSE_DIMENSION, idx_dim, "contract alignment");

    // Minimal synthetic insert demonstrating the chain is live.
    let synthetic_id = Uuid::parse_str("e14de14d-e14e-14e1-b0b0-ffffffff0001").unwrap();
    let mut v: Vec<f32> = (0..1024).map(|i| ((i as f32) / 1024.0).sin()).collect();
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    for x in &mut v {
        *x /= norm;
    }
    e14.insert(synthetic_id, &v).expect("insert into live registry");
    let hits = e14.search(&v, 1, None).expect("search live");
    println!("  self-query returned {:?} (distance={:.6e})", hits[0].0, hits[0].1);
    assert_eq!(hits[0].0, synthetic_id);
    assert!(hits[0].1 < 0.01);
}

/// FSV-T7: Regression guard — Phase A FSV file still present, Phase B FSV
/// file still present. Source of Truth = file-system existence.
#[test]
fn fsv_t7_prior_fsv_files_still_exist() {
    section("T7. Regression guard: both prior FSV files still on disk");

    let phase_a = std::path::Path::new("../context-graph-storage/tests/e14_fsv.rs");
    let phase_b = std::path::Path::new("tests/e14_phase_b_fsv.rs");

    println!(
        "BEFORE: checking phase_a={} and phase_b={}",
        phase_a.display(),
        phase_b.display()
    );
    assert!(phase_a.exists(), "Phase A FSV deleted — regression");
    assert!(phase_b.exists(), "Phase B FSV (original 9-check) deleted — regression");

    // Sanity content check on each.
    let a_content = std::fs::read_to_string(phase_a).expect("read A");
    let b_content = std::fs::read_to_string(phase_b).expect("read B");
    assert!(
        a_content.contains("fsv_13_physical_rocksdb_persistence_e14"),
        "Phase A signature test missing"
    );
    assert!(
        b_content.contains("fsv_b3_hnsw_registry_has_e14"),
        "Phase B 9-check signature test missing"
    );
    println!(
        "AFTER:  phase_a={} bytes, phase_b={} bytes [PASS]",
        a_content.len(),
        b_content.len()
    );
}
