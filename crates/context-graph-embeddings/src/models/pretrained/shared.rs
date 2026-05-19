//! Shared model state type for pretrained embedders.
//!
//! Most pretrained models follow the same Unloaded/Loaded pattern with
//! weights + tokenizer. This generic eliminates the 6+ duplicate enum
//! definitions across the pretrained model modules.

use tokenizers::Tokenizer;

/// Internal state for pretrained model weight management.
///
/// Generic over the weight type `W` so different architectures
/// (BertWeights, QwenWeights, etc.) can share the same enum.
///
/// Models with extra state in their Loaded variant (graph projections,
/// LoRA trained state, MLM heads) should define their own enum.
#[allow(dead_code)]
pub(crate) enum ModelState<W> {
    /// Unloaded - no weights in memory.
    Unloaded,

    /// Loaded with model weights and tokenizer (GPU-accelerated).
    Loaded {
        /// Model weights on GPU (type depends on architecture).
        weights: W,
        /// HuggingFace tokenizer for text encoding (boxed to reduce enum size).
        tokenizer: Box<Tokenizer>,
    },
}

#[cfg(test)]
const PRETRAINED_TEST_MODELS_ROOT: &str = "/zfs/hot/contextgraph/models";

#[cfg(test)]
const PRETRAINED_TEST_MODELS_REGISTRY: &str = "mejepa_models_config.toml";

#[cfg(test)]
fn configured_test_models_root() -> std::path::PathBuf {
    for var in [
        "CONTEXT_GRAPH_MODELS_PATH",
        "CONTEXTGRAPH_MODELS_ROOT",
        "EMBEDDING_MODELS_DIR",
    ] {
        if let Ok(value) = std::env::var(var) {
            return std::path::PathBuf::from(value);
        }
    }

    std::path::PathBuf::from(PRETRAINED_TEST_MODELS_ROOT)
}

#[cfg(test)]
pub(crate) fn pretrained_test_model_path(model_dir: &str) -> std::path::PathBuf {
    pretrained_test_model_path_result(model_dir).unwrap_or_else(|message| panic!("{message}"))
}

#[cfg(test)]
pub(crate) fn pretrained_test_model_path_result(
    model_dir: &str,
) -> Result<std::path::PathBuf, String> {
    let root = configured_test_models_root();
    let registry = root.join(PRETRAINED_TEST_MODELS_REGISTRY);
    if !registry.is_file() {
        return Err(format!(
            "PRETRAINED_TEST_MODELS_ROOT_MISSING: expected registry {}. \
             Set CONTEXT_GRAPH_MODELS_PATH, CONTEXTGRAPH_MODELS_ROOT, or EMBEDDING_MODELS_DIR \
             to the real ME-JEPA model artifact root; mocks and empty model dirs are invalid.",
            registry.display()
        ));
    }

    let registry_text = std::fs::read_to_string(&registry).map_err(|err| {
        format!(
            "PRETRAINED_TEST_MODELS_REGISTRY_UNREADABLE: failed to read {}: {err}",
            registry.display()
        )
    })?;
    let registry_value: toml::Value = toml::from_str(&registry_text).map_err(|err| {
        format!(
            "PRETRAINED_TEST_MODELS_REGISTRY_INVALID: failed to parse {}: {err}",
            registry.display()
        )
    })?;

    let model_path = root.join(model_dir);
    let model_path_text = model_path.to_string_lossy();
    let is_pinned_active_model = registry_value
        .get("embedders")
        .and_then(toml::Value::as_table)
        .map(|embedders| {
            embedders.values().any(|embedder| {
                embedder
                    .get("path")
                    .and_then(toml::Value::as_str)
                    .is_some_and(|path| path == model_path_text)
            })
        })
        .unwrap_or(false);

    if !is_pinned_active_model {
        return Err(format!(
            "PRETRAINED_TEST_MODEL_NOT_PINNED: model_dir={} resolved to {}, \
             but that path is not pinned in active registry {}. \
             Retired or experimental artifacts must not be loaded by active pretrained tests.",
            model_dir,
            model_path.display(),
            registry.display()
        ));
    }

    let tokenizer_path = model_path.join("tokenizer.json");
    if !tokenizer_path.is_file() {
        return Err(format!(
            "PRETRAINED_TEST_TOKENIZER_MISSING: expected real tokenizer {} for model_dir={}. \
             The pretrained integration tests require real downloaded artifacts.",
            tokenizer_path.display(),
            model_dir
        ));
    }

    Ok(model_path)
}

#[cfg(test)]
mod tests {
    use super::pretrained_test_model_path_result;

    #[test]
    fn active_registry_accepts_real_semantic_artifact() {
        let model_path = pretrained_test_model_path_result("semantic")
            .expect("semantic must be pinned in the active ME-JEPA registry");
        assert!(
            model_path.join("tokenizer.json").is_file(),
            "semantic tokenizer must exist on disk at {}",
            model_path.join("tokenizer.json").display()
        );
    }

    #[test]
    fn retired_causal_artifact_is_rejected_by_active_registry() {
        let err = pretrained_test_model_path_result("causal")
            .expect_err("retired E5 causal artifact must not be an active pretrained test input");
        assert!(
            err.contains("PRETRAINED_TEST_MODEL_NOT_PINNED"),
            "expected retired model to fail closed at registry gate, got: {err}"
        );
        assert!(
            err.contains("causal"),
            "error must name the rejected model_dir for operator diagnosis: {err}"
        );
    }
}
