//! BGE-M3 Tokenizer Probe (E14 Phase 1, Task 1)
//!
//! Verifies that the existing `tokenizers` v0.22 crate can load the real
//! BAAI/bge-m3 XLM-RoBERTa SentencePiece `tokenizer.json` and tokenize
//! real Shakespeare text.
//!
//! # Hard rules (from SMAP coordinator)
//! - NO workarounds. If the tokenizers crate can't load bge-m3's tokenizer,
//!   that is a HIGH blocker; this test MUST fail loudly.
//! - NO mock tokenizer.json. Use the real pinned model artifact.
//! - NO assumption it'll work — verify end-to-end.
//!
//! # Strategy
//! ME-JEPA model artifacts are pinned in `mejepa_models_config.toml` under the
//! local model root. The existing codebase pattern is
//! `tokenizers::Tokenizer::from_file(&model_path.join("tokenizer.json"))`
//! (see semantic/loader.rs, kepler/model.rs, etc.). We resolve the E14
//! bge-m3-dense artifact from that active registry directly.
//!
//! If the model root or registry is missing, the test fails with a clear setup
//! error. That is preferable to silently passing on an unverified tokenizer.
//!
//! # Acceptance (Task 1 from 12_task_breakdown_pilot.md)
//! - Tokenization succeeds without panic
//! - Token count is 8-15 for "Shall I compare thee to a summer's day?"
//! - Runs cleanly via `cargo test`
//!
//! # Run
//! ```bash
//! cargo test --workspace -p context-graph-embeddings \
//!     --test bge_m3_tokenizer_probe -- --nocapture
//! ```

use std::path::{Path, PathBuf};

use tokenizers::Tokenizer;

/// Canonical local ME-JEPA model artifact root used by this workstation.
const DEFAULT_MODELS_ROOT: &str = "/zfs/hot/contextgraph/models";
const MODELS_REGISTRY_FILENAME: &str = "mejepa_models_config.toml";
const BGE_M3_MODEL_DIR: &str = "bge-m3-dense";
const BGE_M3_TOKENIZER_FILENAME: &str = "tokenizer.json";

fn configured_models_root() -> PathBuf {
    for var in [
        "CONTEXT_GRAPH_MODELS_PATH",
        "CONTEXTGRAPH_MODELS_ROOT",
        "EMBEDDING_MODELS_DIR",
    ] {
        if let Ok(value) = std::env::var(var) {
            return PathBuf::from(value);
        }
    }

    PathBuf::from(DEFAULT_MODELS_ROOT)
}

/// Resolve the pinned bge-m3 tokenizer.json path from the active model registry.
///
/// Fails loudly if the registry does not pin `bge-m3-dense` or if the tokenizer
/// file is missing. Retired, cache-only, or experimental paths must not be used
/// by this probe.
fn resolve_bge_m3_tokenizer_path() -> PathBuf {
    let root = configured_models_root();
    let registry_path = root.join(MODELS_REGISTRY_FILENAME);
    assert!(
        registry_path.is_file(),
        "BGE_M3_MODEL_REGISTRY_MISSING: expected active model registry at {}. \
         Set CONTEXT_GRAPH_MODELS_PATH, CONTEXTGRAPH_MODELS_ROOT, or EMBEDDING_MODELS_DIR \
         to the real ME-JEPA model artifact root.",
        registry_path.display()
    );

    let registry_text = std::fs::read_to_string(&registry_path)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", registry_path.display()));
    let registry_value: toml::Value = toml::from_str(&registry_text)
        .unwrap_or_else(|err| panic!("failed to parse {}: {err}", registry_path.display()));

    let model_dir = root.join(BGE_M3_MODEL_DIR);
    let model_dir_text = model_dir.to_string_lossy();
    let is_pinned = registry_value
        .get("embedders")
        .and_then(toml::Value::as_table)
        .map(|embedders| {
            embedders.values().any(|embedder| {
                embedder
                    .get("path")
                    .and_then(toml::Value::as_str)
                    .is_some_and(|path| path == model_dir_text)
            })
        })
        .unwrap_or(false);
    assert!(
        is_pinned,
        "BGE_M3_MODEL_NOT_PINNED: expected {} to be pinned in active registry {}",
        model_dir.display(),
        registry_path.display()
    );

    let tokenizer_path = model_dir.join(BGE_M3_TOKENIZER_FILENAME);
    assert!(
        tokenizer_path.is_file(),
        "BGE_M3_TOKENIZER_MISSING: expected real tokenizer at {}",
        tokenizer_path.display()
    );

    tokenizer_path
}

/// Tokenize with `add_special_tokens=true` (matches production usage).
///
/// Wraps the tokenizers crate's `tokenize` signature: it returns its own
/// `Result<Encoding, Box<dyn Error + Send + Sync>>`. We don't ignore the
/// error — we panic with the original message so failures are visible.
fn tokenize(tokenizer: &Tokenizer, text: &str) -> tokenizers::Encoding {
    tokenizer
        .encode(text, /* add_special_tokens = */ true)
        .unwrap_or_else(|e| {
            panic!("tokenizer.encode({:?}) failed: {}", text, e);
        })
}

#[test]
fn bge_m3_xlm_roberta_sentencepiece_loads_and_tokenizes() {
    // ---- Phase A: locate the real tokenizer.json ----
    let tokenizer_path = resolve_bge_m3_tokenizer_path();
    println!("bge-m3 tokenizer.json path: {}", tokenizer_path.display());

    let metadata = std::fs::metadata(&tokenizer_path).expect("stat tokenizer.json");
    println!("bge-m3 tokenizer.json size: {} bytes", metadata.len());
    // A real bge-m3 tokenizer.json is ~17 MB. Be permissive but catch the
    // case where somebody stubbed in an empty/truncated file.
    assert!(
        metadata.len() > 1_000_000,
        "tokenizer.json at {} is suspiciously small ({} bytes) — likely truncated",
        tokenizer_path.display(),
        metadata.len()
    );

    // ---- Phase B: load via tokenizers::Tokenizer::from_file ----
    let tokenizer = Tokenizer::from_file(&tokenizer_path).unwrap_or_else(|e| {
        panic!(
            "Tokenizer::from_file({}) FAILED — this is a HIGH blocker for E14. \
             The `tokenizers` v0.22 crate could not parse bge-m3's XLM-RoBERTa \
             SentencePiece tokenizer.json. Error: {}",
            tokenizer_path.display(),
            e
        );
    });
    println!(
        "Loaded bge-m3 tokenizer: vocab_size={}",
        tokenizer.get_vocab_size(true)
    );

    // XLM-RoBERTa vocab size is 250002. Assert the order of magnitude so a
    // wrong-file swap (e.g. bert-base 30522) would be caught.
    let vocab_size = tokenizer.get_vocab_size(true);
    assert!(
        vocab_size > 200_000,
        "vocab_size={} does not match XLM-RoBERTa (~250002); wrong tokenizer?",
        vocab_size
    );

    // ---- Phase C: tokenize three real inputs ----
    let inputs: &[(&str, &str)] = &[
        ("shakespeare_1", "Shall I compare thee to a summer's day?"),
        ("shakespeare_2", "Thou art more lovely and more temperate:"),
        ("modern_1", "A weather forecast: partly cloudy, 72F."),
    ];

    // Run the first input with full verbose output (evidence of success).
    for (i, (label, text)) in inputs.iter().enumerate() {
        let enc = tokenize(&tokenizer, text);
        let ids = enc.get_ids();
        let tokens = enc.get_tokens();

        println!("[{}] {:?} -> {} tokens", label, text, ids.len());
        if i == 0 {
            // First input: print every token as evidence.
            println!("  ids:    {:?}", ids);
            println!("  tokens: {:?}", tokens);
        }

        // Task 1 acceptance: >= 5 tokens (generic probe range)
        //                    <= 50 tokens (sanity: short strings shouldn't explode)
        assert!(
            ids.len() >= 5,
            "{}: expected >= 5 tokens, got {} (text={:?}, tokens={:?})",
            label,
            ids.len(),
            text,
            tokens
        );
        assert!(
            ids.len() <= 50,
            "{}: expected <= 50 tokens, got {} (text={:?})",
            label,
            ids.len(),
            text
        );

        // Token IDs are u32 by construction (non-negative). Assert they're
        // within the declared vocab — catches corrupt decoder state or a
        // mismatched vocab/merges.
        for (pos, &id) in ids.iter().enumerate() {
            assert!(
                (id as usize) < vocab_size,
                "{}: token id {} at pos {} exceeds vocab_size {}",
                label,
                id,
                pos,
                vocab_size
            );
        }
    }

    // ---- Phase D: stricter assertion for the spec-cited Shakespeare input ----
    // Task 1 spec says: "Token count is 12-15 for that string". The spec also
    // says "~8-15 tokens for 'Shall I compare...'". We use the stated range
    // 8-15 (the SMAP prompt explicitly called out "reasonable (12-15)" but
    // since SentencePiece can vary, bracket 8-15 is the coordinator-approved
    // acceptance). Failing this is a legitimate signal something's wrong.
    let enc0 = tokenize(&tokenizer, inputs[0].1);
    let n = enc0.get_ids().len();
    assert!(
        (8..=15).contains(&n),
        "Expected 8-15 tokens for Sonnet 18 line, got {}. Tokens: {:?}",
        n,
        enc0.get_tokens()
    );

    // ---- Phase E: decode round-trip (approximate — SentencePiece normalizes) ----
    for (label, text) in inputs {
        let enc = tokenize(&tokenizer, text);
        let decoded = tokenizer
            .decode(enc.get_ids(), /* skip_special_tokens = */ true)
            .unwrap_or_else(|e| {
                panic!("decode failed on {}: {}", label, e);
            });
        println!("[{}] decode: {:?}", label, decoded);

        // SentencePiece normalizes whitespace and may drop/alter punctuation
        // spacing. We can't expect bit-identical round-trip, but we can
        // expect overlap on content words. Use a soft check: at least one
        // content word from the original should appear in the decoded form.
        // (Lowercase comparison to be tolerant of SP casing quirks.)
        let decoded_lc = decoded.to_lowercase();
        let picked_word = text
            .split_whitespace()
            .find(|w| w.len() >= 4)
            .expect("test input has at least one 4+ char word");
        let picked_lc = picked_word
            .trim_end_matches(|c: char| !c.is_alphanumeric())
            .to_lowercase();
        assert!(
            decoded_lc.contains(&picked_lc),
            "decode round-trip for {} lost content word {:?}: decoded={:?}",
            label,
            picked_lc,
            decoded
        );
    }

    // ---- Phase F: verify special tokens are present as expected ----
    // XLM-RoBERTa uses <s>=0, <pad>=1, </s>=2, <unk>=3
    // Encoded strings should have <s> at position 0 and </s> at the end
    // (since add_special_tokens=true).
    let enc0 = tokenize(&tokenizer, inputs[0].1);
    let ids = enc0.get_ids();
    assert_eq!(
        ids.first().copied(),
        Some(0),
        "expected <s> (id=0) at start of encoding; got {:?}",
        ids.first()
    );
    assert_eq!(
        ids.last().copied(),
        Some(2),
        "expected </s> (id=2) at end of encoding; got {:?}",
        ids.last()
    );

    println!("\n=== BGE-M3 TOKENIZER PROBE: PASS ===");
    println!("tokenizers v0.22 successfully loads XLM-RoBERTa SentencePiece");
    println!("from BAAI/bge-m3 and tokenizes real Shakespeare text.");
    println!("E14 Phase 1 Task 1: UNBLOCKED for agent-e14-2 (enum plumbing).");
}

/// Sanity test: the path resolver itself is deterministic and pinned to the
/// active local model artifact root.
#[test]
fn resolve_path_finds_existing_snapshot() {
    let path = resolve_bge_m3_tokenizer_path();
    assert!(
        path.is_file(),
        "resolved path must be an existing file: {}",
        path.display()
    );
    assert!(
        path.file_name()
            .map(|n| n == "tokenizer.json")
            .unwrap_or(false),
        "resolved path must end in tokenizer.json: {}",
        path.display()
    );
    // Ensure we picked a root snapshot tokenizer.json, not the onnx/ one.
    let parent = path.parent().expect("has parent");
    assert!(
        parent.ends_with(BGE_M3_MODEL_DIR),
        "resolved tokenizer path must come from the pinned {} artifact, got {}",
        BGE_M3_MODEL_DIR,
        path.display()
    );
    let _: &Path = &path; // ensure Path trait is imported + used
}
