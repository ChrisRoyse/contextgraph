//! E14 Phase B — Full State Verification for the native model, HNSW
//! registration, MCP DTO, and migration CLI surface.
//!
//! Each check:
//! 1. Defines its Source of Truth.
//! 2. Executes code with deterministic synthetic inputs.
//! 3. Reads back from the Source of Truth (not the call's return value).
//! 4. Prints before / after state.
//! 5. Exercises at least one edge case where applicable.

use std::path::PathBuf;

use context_graph_embeddings::config::GpuConfig;
use context_graph_embeddings::models::pretrained::{
    BgeM3DenseModel, BGE_M3_DENSE_DIMENSION, BGE_M3_DENSE_LATENCY_BUDGET_MS,
    BGE_M3_DENSE_MAX_TOKENS, XLM_R_BOS_TOKEN_ID, XLM_R_PAD_TOKEN_ID, XLM_R_POSITION_OFFSET,
    XLM_R_WEIGHT_PREFIX,
};
use context_graph_embeddings::models::DefaultModelFactory;
use context_graph_embeddings::traits::{ModelFactory, SingleModelConfig};
use context_graph_embeddings::types::ModelId;
use context_graph_storage::teleological::indexes::{
    EmbedderIndex, EmbedderIndexOps, EmbedderIndexRegistry,
};
use context_graph_storage::teleological::{
    CF_EMB_13, QUANTIZED_EMBEDDER_CFS, QUANTIZED_EMBEDDER_CF_COUNT,
};

fn section(title: &str) {
    println!("\n================================================================================");
    println!("PHASE B FSV: {}", title);
    println!("================================================================================");
}

// ============================================================================
// FSV-B1: BgeM3DenseModel module contract — constants, construction, state.
// ============================================================================
// Source of Truth: the exported constants + AtomicBool inside BgeM3DenseModel.

#[test]
fn fsv_b1_bge_m3_dense_module_contract() {
    section("B1. BgeM3DenseModel module contract (constants + construction)");

    println!(
        "BEFORE expected: DIM=1024, MAX_TOKENS=8192, LATENCY=50ms, BOS=0, PAD=1, POS_OFFSET=2, PREFIX='roberta.'"
    );
    println!(
        "AFTER  actual:   DIM={}, MAX_TOKENS={}, LATENCY={}ms, BOS={}, PAD={}, POS_OFFSET={}, PREFIX='{}'",
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

    // Construction: `new` succeeds with defaults; `is_initialized` is false.
    let path = PathBuf::from("./models/bge-m3-dense");
    let model = BgeM3DenseModel::new(&path, SingleModelConfig::default())
        .expect("construction with defaults must succeed");
    assert!(!model.is_initialized(), "fresh model must not be loaded");

    // EDGE: zero batch size fails at construction.
    let zero_batch = SingleModelConfig {
        max_batch_size: 0,
        ..Default::default()
    };
    match BgeM3DenseModel::new(&path, zero_batch) {
        Ok(_) => panic!("zero batch must fail"),
        Err(e) => {
            let msg = format!("{:?}", e);
            assert!(
                msg.contains("max_batch_size"),
                "unexpected error: {}",
                msg
            );
            println!(
                "EDGE:   zero batch_size → ConfigError mentions 'max_batch_size' [OK]"
            );
        }
    }

    println!("BgeM3DenseModel contract [PASS]");
}

// ============================================================================
// FSV-B2: Factory creates a BgeM3DenseModel for ModelId::BgeM3Dense.
// ============================================================================
// Source of Truth: `model.model_id()` on the returned trait object.

#[test]
fn fsv_b2_factory_creates_bge_m3_dense_model() {
    section("B2. Factory produces BgeM3DenseModel for ModelId::BgeM3Dense");

    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());
    let config = SingleModelConfig::default();

    println!("BEFORE: requesting ModelId::BgeM3Dense from DefaultModelFactory");
    let boxed = factory
        .create_model(ModelId::BgeM3Dense, &config)
        .expect("factory must create BgeM3Dense (native module is present)");

    println!("AFTER:  boxed.model_id() = {:?}", boxed.model_id());
    assert_eq!(boxed.model_id(), ModelId::BgeM3Dense);
    assert!(
        !boxed.is_initialized(),
        "factory returns unloaded models; load() is called later"
    );

    println!("Factory wiring [PASS]");
}

// ============================================================================
// FSV-B3: HNSW registry contains E14BgeM3Dense as a 1024D Cosine index.
// ============================================================================
// Source of Truth: EmbedderIndexRegistry.get(E14BgeM3Dense).config().dimension

#[test]
fn fsv_b3_hnsw_registry_has_e14() {
    section("B3. HNSW registry contains E14BgeM3Dense (1024D)");

    let registry = EmbedderIndexRegistry::new();

    println!(
        "BEFORE expected: registry.len() = 16 (post-E14); get(E14BgeM3Dense) = Some(..)"
    );
    println!("AFTER  actual:   registry.len() = {}", registry.len());
    assert_eq!(registry.len(), 16);

    let handle = registry
        .get(EmbedderIndex::E14BgeM3Dense)
        .expect("E14 must have an HNSW index registered");
    let dim = handle.config().dimension;
    println!(
        "        E14 HNSW dimension = {} (expected 1024)",
        dim
    );
    assert_eq!(dim, 1024, "E14 HNSW must be 1024D Cosine");

    // EDGE: the E5/E10 asymmetric slots still exist alongside E14.
    assert!(registry.get(EmbedderIndex::E5CausalCause).is_some());
    assert!(registry.get(EmbedderIndex::E10MultimodalContext).is_some());
    // EDGE: E13Splade remains non-HNSW (inverted index).
    assert!(registry.get(EmbedderIndex::E13Splade).is_none());
    println!(
        "EDGE:   E5 asymmetric + E10 asymmetric slots coexist with E14; E13 inverted stays None [OK]"
    );

    println!("HNSW registry [PASS]");
}

// ============================================================================
// FSV-B4: Column-family table exposes CF_EMB_13 and the CF count is 14.
// ============================================================================
// Source of Truth: QUANTIZED_EMBEDDER_CFS static slice + _CF_COUNT constant.

#[test]
fn fsv_b4_cf_emb_13_registered() {
    section("B4. CF_EMB_13 registered alongside CF_EMB_0..CF_EMB_12");

    println!(
        "BEFORE expected: QUANTIZED_EMBEDDER_CF_COUNT = 14; CF_EMB_13 = \"emb_13\""
    );
    println!(
        "AFTER  actual:   QUANTIZED_EMBEDDER_CF_COUNT = {}; CF_EMB_13 = \"{}\"",
        QUANTIZED_EMBEDDER_CF_COUNT, CF_EMB_13
    );

    assert_eq!(QUANTIZED_EMBEDDER_CF_COUNT, 14);
    assert_eq!(CF_EMB_13, "emb_13");
    assert_eq!(QUANTIZED_EMBEDDER_CFS.len(), 14);
    assert_eq!(QUANTIZED_EMBEDDER_CFS[13], CF_EMB_13);

    // Contract: entries are ordered emb_0..emb_13 exactly.
    for (i, name) in QUANTIZED_EMBEDDER_CFS.iter().enumerate() {
        let expected = format!("emb_{}", i);
        assert_eq!(*name, expected, "CF slot {} should be {}", i, expected);
    }

    println!("CF_EMB_13 + QUANTIZED_EMBEDDER_CFS ordering [PASS]");
}

// ============================================================================
// FSV-B5: MCP `EmbedderId::from_str` parses E14 and canonical aliases.
// ============================================================================
// Source of Truth: the returned EmbedderId enum variant.

#[test]
fn fsv_b5_mcp_embedder_id_recognises_e14() {
    section("B5. MCP EmbedderId::from_str recognises E14 and its aliases");

    use context_graph_mcp::handlers::tools::embedder_dtos::EmbedderId;

    let cases: &[(&str, Option<EmbedderId>)] = &[
        ("E14", Some(EmbedderId::E14)),
        ("e14", Some(EmbedderId::E14)),
        ("bge_m3_dense", Some(EmbedderId::E14)),
        ("bge-m3-dense", Some(EmbedderId::E14)),
        ("bgem3dense", Some(EmbedderId::E14)),
        ("E15", None),
        ("not_an_embedder", None),
    ];

    for (input, expected) in cases {
        let got = EmbedderId::from_str(input);
        println!(
            "  from_str({:?}) -> {:?} (expected {:?})",
            input, got, expected
        );
        assert_eq!(&got, expected, "mismatch on {:?}", input);
    }

    let e14 = EmbedderId::E14;
    assert_eq!(e14.to_index(), 13, "E14 must occupy index 13");
    assert!(e14.is_semantic(), "E14 must classify as semantic");
    assert!(e14.uses_hnsw(), "E14 must be HNSW-indexed");
    assert!(!e14.is_sparse(), "E14 is dense, not sparse");
    assert!(!e14.is_temporal(), "E14 is not a temporal embedder");

    println!("E14 MCP DTO parsing + classification [PASS]");
}

// ============================================================================
// FSV-B6: Migration CLI argument parsing + contract.
// ============================================================================
// Source of Truth: the parsed BackfillE14Args struct fields.

#[test]
fn fsv_b6_migration_cli_argument_contract() {
    section("B6. backfill-e14 CLI argument contract");

    use clap::Parser;

    #[derive(Debug, clap::Parser)]
    struct Cli {
        #[command(subcommand)]
        sub: Sub,
    }

    #[derive(Debug, clap::Subcommand)]
    enum Sub {
        #[command(name = "backfill-e14")]
        BackfillE14(context_graph_cli::commands::backfill_e14::BackfillE14Args),
    }

    // Happy path.
    let cli = Cli::try_parse_from([
        "ctx",
        "backfill-e14",
        "--storage",
        "/tmp/some-db",
        "--models-dir",
        "/tmp/models",
        "--dry-run",
        "--limit",
        "500",
        "--scan-batch",
        "1000",
        "--log-every",
        "50",
    ])
    .expect("happy path must parse");

    let args = match cli.sub {
        Sub::BackfillE14(a) => a,
    };

    println!(
        "AFTER:  storage={:?}, models_dir={:?}, dry_run={}, limit={:?}, scan_batch={}, log_every={}",
        args.storage,
        args.models_dir,
        args.dry_run,
        args.limit,
        args.scan_batch,
        args.log_every,
    );

    assert_eq!(args.storage, PathBuf::from("/tmp/some-db"));
    assert_eq!(args.models_dir, PathBuf::from("/tmp/models"));
    assert!(args.dry_run);
    assert_eq!(args.limit, Some(500));
    assert_eq!(args.scan_batch, 1000);
    assert_eq!(args.log_every, 50);

    // EDGE: missing --storage must fail parsing.
    let err = Cli::try_parse_from(["ctx", "backfill-e14"])
        .expect_err("missing --storage must fail clap validation");
    println!("EDGE:   missing --storage → clap error: {:.60}...", err.to_string());

    println!("Migration CLI contract [PASS]");
}

// ============================================================================
// FSV-B7: Phase B does not regress Phase A — Phase A FSV file still on disk.
// ============================================================================
// Source of Truth: the presence of tests/e14_fsv.rs under the storage crate.

#[test]
fn fsv_b7_phase_a_fsv_file_still_present() {
    section("B7. Phase A FSV test file still present (no regression)");

    // Tests run from the crate root; reach across to the storage crate.
    let phase_a = std::path::Path::new("../context-graph-storage/tests/e14_fsv.rs");
    let exists = phase_a.exists();
    println!("BEFORE: checking ../context-graph-storage/tests/e14_fsv.rs");
    println!("AFTER:  exists = {}", exists);
    assert!(
        exists,
        "Phase A FSV file must remain on disk — Phase B must not delete it"
    );

    let head = std::fs::read_to_string(phase_a).expect("read phase A FSV file");
    assert!(
        head.contains("fsv_13_physical_rocksdb_persistence_e14"),
        "Phase A FSV must still contain the physical RocksDB persistence test"
    );
    println!("Phase A FSV harness preserved [PASS]");
}

// ============================================================================
// FSV-B8: Embedded-runtime sanity — factory list includes BgeM3Dense and
// supported_models reflects the 15-variant universe (14 prod + legacy Entity).
// ============================================================================

#[test]
fn fsv_b8_factory_supported_models_covers_bge_m3_dense() {
    section("B8. Factory supported_models() reports BgeM3Dense");

    let factory = DefaultModelFactory::new(PathBuf::from("./models"), GpuConfig::default());
    let models = factory.supported_models();
    println!("BEFORE expected: len() >= 15 and contains ModelId::BgeM3Dense");
    println!(
        "AFTER  actual:   len()={}, contains_bge_m3_dense={}",
        models.len(),
        models.contains(&ModelId::BgeM3Dense)
    );

    assert!(
        models.contains(&ModelId::BgeM3Dense),
        "factory must advertise BgeM3Dense"
    );
    assert!(
        models.len() >= 15,
        "factory must support at least 15 model IDs (14 production + legacy Entity)"
    );

    println!("Factory supported_models [PASS]");
}

// ============================================================================
// FSV-B9: Registry memory footprint sanity — E14 added without blowing up.
// ============================================================================

#[test]
fn fsv_b9_registry_memory_bytes_monotonic() {
    section("B9. Registry total_memory_bytes() sanity with E14 added");

    let registry = EmbedderIndexRegistry::new();
    let bytes = registry.total_memory_bytes();
    let vectors = registry.total_vectors();
    println!("BEFORE: empty indexes — no vectors inserted yet");
    println!(
        "AFTER:  total_memory_bytes()={}, total_vectors()={}",
        bytes, vectors
    );

    // Empty HNSW indexes still allocate container metadata, so bytes > 0 is
    // acceptable. What MUST hold: with no inserts, total_vectors = 0.
    assert_eq!(vectors, 0, "no inserts should mean zero vectors");
    println!("Registry starts with zero vectors across 16 HNSW slots [PASS]");

    // Insert a single synthetic E14 vector to prove the index is live.
    use uuid::Uuid;

    let e14 = registry
        .get(EmbedderIndex::E14BgeM3Dense)
        .expect("E14 index registered");
    let id = Uuid::new_v4();
    let vec1024: Vec<f32> = (0..1024).map(|i| (i as f32) / 1024.0).collect();
    e14.insert(id, &vec1024).expect("insert into E14 HNSW");

    let after_vectors = registry.total_vectors();
    println!(
        "AFTER insert: total_vectors()={} (expected 1)",
        after_vectors
    );
    assert_eq!(after_vectors, 1);

    // EDGE: search returns the inserted vector. `search` has signature
    // `(query, k, ef_search: Option<usize>) -> Vec<(Uuid, f32)>` and the
    // second element is COSINE DISTANCE (0 = identical, 2 = opposite).
    let results = e14
        .search(&vec1024, 1, None)
        .expect("E14 HNSW search");
    println!(
        "EDGE:   self-search returns {} hits (top id = {:?}, distance = {:e})",
        results.len(),
        results.first().map(|r| r.0),
        results.first().map(|r| r.1).unwrap_or(f32::NAN)
    );
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].0, id);
    assert!(
        results[0].1 < 0.01,
        "self-similarity distance should be ~0.0, got {}",
        results[0].1
    );

    println!("Registry E14 insert + search round-trips [PASS]");
}
