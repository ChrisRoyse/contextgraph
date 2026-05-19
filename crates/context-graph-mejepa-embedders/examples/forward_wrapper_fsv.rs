use context_graph_core::memory::ast::Language;
use context_graph_mejepa_embedders::{
    embedder_model_id, routing_coverage, AlgorithmicEmbedderForward, EmbedderForward, EmbedderId,
    EmbedderInput, EmbedderOutput, ModelsConfig, PretrainedEmbedderForward,
    SUPPORTED_ALGORITHMIC_FORWARD_EMBEDDERS, SUPPORTED_FORWARD_EMBEDDERS,
    SUPPORTED_PRETRAINED_FORWARD_EMBEDDERS,
};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

#[tokio::main(flavor = "current_thread")]
async fn main() -> FsvResult<()> {
    let root = PathBuf::from("/zfs/archive/contextgraph/fsv/contextgraph-mejepa-forward-fsv");
    reset_root(&root)?;
    let output_dir = root.join("forward-outputs");
    fs::create_dir_all(&output_dir)?;
    let evidence_path = root.join("forward-wrapper-evidence.json");
    let config_path = std::env::var("CONTEXTGRAPH_MEJEPA_MODELS_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from("/zfs/hot/contextgraph/models/mejepa_models_config.toml")
        });

    let config = ModelsConfig::load(&config_path)?;
    let mut proven_outputs = BTreeMap::<String, String>::new();
    let mut pretrained_outputs = Vec::new();
    let mut e7_language_outputs = Vec::new();

    for embedder in SUPPORTED_PRETRAINED_FORWARD_EMBEDDERS {
        let registration = config.registration(*embedder)?;
        let output_path = output_dir.join(format!("{}-forward-output.json", embedder.slug()));
        let before = json!({
            "embedder": embedder.slug(),
            "models_config": config_path.display().to_string(),
            "registration_path": registration.path,
            "registration_manifest_sha256": registration.manifest_sha256,
            "output_exists": output_path.exists(),
        });
        let forward = PretrainedEmbedderForward::load(registration).await?;
        let input = canonical_input(*embedder, None)?;
        let output = forward.forward(&input).await?;
        let output_json = write_forward_output(&output_path, &output)?;
        require(
            &format!("{} canonical vector dimension", embedder.slug()),
            output_json["vector_len"] == json!(embedder.dimension()),
        )?;
        require(
            &format!("{} canonical vector is non-zero", embedder.slug()),
            output_json["vector_l2_norm"].as_f64().unwrap_or_default() > 0.0,
        )?;
        proven_outputs.insert(
            embedder.slug().to_string(),
            output_path.display().to_string(),
        );
        pretrained_outputs.push(json!({
            "before_state": before,
            "trigger": format!(
                "PretrainedEmbedderForward::load({}) then forward({} canonical input)",
                embedder.slug(),
                embedder.slug()
            ),
            "after_state": output_json,
            "passes": true,
        }));

        if *embedder == EmbedderId::E7 {
            e7_language_outputs = run_e7_language_matrix(&forward, &output_dir).await?;
            for row in &e7_language_outputs {
                let language = row["language"]
                    .as_str()
                    .ok_or("e7 language output missing language")?;
                let output_path = row["output_path"]
                    .as_str()
                    .ok_or("e7 language output missing path")?;
                proven_outputs.insert(format!("e7:{language}"), output_path.to_string());
            }
        }
    }

    let mut algorithmic_outputs = Vec::new();
    for embedder in SUPPORTED_ALGORITHMIC_FORWARD_EMBEDDERS {
        let output_path = output_dir.join(format!("{}-algorithmic-output.json", embedder.slug()));
        let before = json!({
            "embedder": embedder.slug(),
            "output_exists": output_path.exists(),
        });
        let forward = AlgorithmicEmbedderForward::load(*embedder)?;
        let input = canonical_input(*embedder, None)?;
        let output = forward.forward(&input).await?;
        let output_json = write_forward_output(&output_path, &output)?;
        require(
            &format!("{} algorithmic vector dimension", embedder.slug()),
            output_json["vector_len"] == json!(embedder.dimension()),
        )?;
        require(
            &format!("{} algorithmic vector is non-zero", embedder.slug()),
            output_json["vector_l2_norm"].as_f64().unwrap_or_default() > 0.0,
        )?;
        proven_outputs.insert(
            embedder.slug().to_string(),
            output_path.display().to_string(),
        );
        algorithmic_outputs.push(json!({
            "before_state": before,
            "trigger": format!(
                "AlgorithmicEmbedderForward::load({}) then forward({} canonical input)",
                embedder.slug(),
                embedder.slug()
            ),
            "after_state": output_json,
            "passes": true,
        }));
    }

    let routing_rows = verify_routing_matrix(&proven_outputs)?;
    let routing_path = root.join("routing-forward-resolution.json");
    write_json_checked(&routing_path, &routing_rows)?;
    let routing_readback: Value = serde_json::from_slice(&fs::read(&routing_path)?)?;
    require(
        "routing row count readback",
        routing_readback.as_array().map(Vec::len) == Some(13 * 11),
    )?;

    let e10_registration = config.registration(EmbedderId::E10)?;
    let e10_forward = PretrainedEmbedderForward::load(e10_registration).await?;
    let edge_cases = vec![
        edge_unsupported_forward_slot(&root)?,
        edge_empty_input(&e10_forward, &root).await?,
        edge_mismatched_input_embedder(&e10_forward, &root).await?,
    ];

    let evidence = json!({
        "source_of_truth": {
            "evidence_json": evidence_path.display().to_string(),
            "output_dir": output_dir.display().to_string(),
            "routing_forward_resolution_json": routing_path.display().to_string(),
            "models_config": config_path.display().to_string(),
        },
        "happy_path": {
            "pretrained_forward_outputs": pretrained_outputs,
            "algorithmic_forward_outputs": algorithmic_outputs,
            "e7_language_matrix_outputs": e7_language_outputs,
            "routing_forward_resolution": {
                "before_state": {
                    "languages": 11,
                    "entity_types": 13,
                    "supported_forward_embedders": SUPPORTED_FORWARD_EMBEDDERS
                        .iter()
                        .map(|id| id.slug())
                        .collect::<Vec<_>>(),
                },
                "after_state": {
                    "row_count": routing_rows.len(),
                    "routing_resolution_json": routing_path.display().to_string(),
                },
                "passes": true,
            }
        },
        "edge_cases": edge_cases,
        "passes": true,
    });
    write_json_checked(&evidence_path, &evidence)?;
    let evidence_readback: Value = serde_json::from_slice(&fs::read(&evidence_path)?)?;
    require(
        "evidence readback passes",
        evidence_readback["passes"] == Value::Bool(true),
    )?;
    require(
        "evidence readback output dir",
        evidence_readback["source_of_truth"]["output_dir"] == output_dir.display().to_string(),
    )?;
    require(
        "evidence readback pretrained output count",
        evidence_readback["happy_path"]["pretrained_forward_outputs"]
            .as_array()
            .map(Vec::len)
            == Some(SUPPORTED_PRETRAINED_FORWARD_EMBEDDERS.len()),
    )?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}

async fn run_e7_language_matrix(
    forward: &PretrainedEmbedderForward,
    output_dir: &Path,
) -> FsvResult<Vec<Value>> {
    let mut rows = Vec::new();
    for language in Language::all() {
        let output_path = output_dir.join(format!("e7-{}-forward-output.json", language.slug()));
        let before = json!({
            "language": language.slug(),
            "output_exists": output_path.exists(),
        });
        let input = canonical_input(EmbedderId::E7, Some(language))?;
        let output = forward.forward(&input).await?;
        let output_json = write_forward_output(&output_path, &output)?;
        require(
            &format!("e7 {} vector dimension", language.slug()),
            output_json["vector_len"] == json!(EmbedderId::E7.dimension()),
        )?;
        rows.push(json!({
            "language": language.slug(),
            "output_path": output_path.display().to_string(),
            "before_state": before,
            "trigger": format!("E7 forward with {} source_id", language.slug()),
            "after_state": output_json,
            "passes": true,
        }));
    }
    Ok(rows)
}

fn verify_routing_matrix(proven_outputs: &BTreeMap<String, String>) -> FsvResult<Vec<Value>> {
    let rows = routing_coverage()?;
    require("routing coverage row count", rows.len() == 13 * 11)?;
    let mut evidence = Vec::with_capacity(rows.len());
    for row in rows {
        let mut resolved = BTreeMap::new();
        for embedder in &row.embedders {
            require(
                &format!(
                    "route {} {} uses supported forward embedder {}",
                    row.language, row.entity_type, embedder
                ),
                SUPPORTED_FORWARD_EMBEDDERS.contains(embedder),
            )?;
            let key = if *embedder == EmbedderId::E7 {
                format!("e7:{}", row.language)
            } else {
                embedder.slug().to_string()
            };
            let path = proven_outputs.get(&key).ok_or_else(|| {
                format!(
                    "route {} {} requires {} but no proved output exists for key {}",
                    row.language, row.entity_type, embedder, key
                )
            })?;
            require(
                &format!("proved output path exists for {key}"),
                Path::new(path).is_file(),
            )?;
            resolved.insert(embedder.slug().to_string(), path.clone());
        }
        evidence.push(json!({
            "language": row.language,
            "entity_type": row.entity_type,
            "embedders": row
                .embedders
                .iter()
                .map(|id| id.slug())
                .collect::<Vec<_>>(),
            "resolved_forward_output_files": resolved,
            "passes": true,
        }));
    }
    Ok(evidence)
}

fn canonical_input(embedder: EmbedderId, language: Option<Language>) -> FsvResult<EmbedderInput> {
    let (text, source_id) = match embedder {
        EmbedderId::E2 | EmbedderId::E3 | EmbedderId::E4 => (
            format!(
                "epoch:1778599800\nfn temporal_signal_{}() -> i32 {{ 2 + 2 }}",
                embedder.slug()
            ),
            format!("forward-fsv/{}/temporal.rs", embedder.slug()),
        ),
        EmbedderId::E7 => {
            let language = language.unwrap_or(Language::Rust);
            (
                sample_code(language).to_string(),
                format!("forward-fsv/e7/sample.{}", extension_for_language(language)),
            )
        }
        EmbedderId::E9 => (
            "fn hdc_signal(alpha: i32, beta: i32) -> i32 { alpha + beta }".to_string(),
            "forward-fsv/e9/hdc.rs".to_string(),
        ),
        _ => (
            format!(
                "ContextGraph full forward-wrapper FSV for {}: model artifact, tokenizer, and vector output must be verified from source of truth.",
                embedder.slug()
            ),
            format!("forward-fsv/{}/text.txt", embedder.slug()),
        ),
    };
    Ok(EmbedderInput {
        embedder,
        text,
        source_id,
    })
}

fn sample_code(language: Language) -> &'static str {
    match language {
        Language::Rust => "pub fn add(a: i32, b: i32) -> i32 { a + b }",
        Language::Python => "def add(a: int, b: int) -> int:\n    return a + b\n",
        Language::JavaScript => "export function add(a, b) { return a + b; }",
        Language::TypeScript => {
            "export function add(a: number, b: number): number { return a + b; }"
        }
        Language::Go => "package sample\nfunc add(a int, b int) int { return a + b }\n",
        Language::Java => "class Sample { static int add(int a, int b) { return a + b; } }",
        Language::C => "int add(int a, int b) { return a + b; }",
        Language::Cpp => "int add(int a, int b) { return a + b; }",
        Language::CSharp => "class Sample { static int Add(int a, int b) { return a + b; } }",
        Language::Ruby => "def add(a, b)\n  a + b\nend\n",
        Language::Php => "<?php function add($a, $b) { return $a + $b; }",
    }
}

fn extension_for_language(language: Language) -> &'static str {
    match language {
        Language::Rust => "rs",
        Language::Python => "py",
        Language::JavaScript => "js",
        Language::TypeScript => "ts",
        Language::Go => "go",
        Language::Java => "java",
        Language::C => "c",
        Language::Cpp => "cpp",
        Language::CSharp => "cs",
        Language::Ruby => "rb",
        Language::Php => "php",
    }
}

fn write_forward_output(path: &Path, output: &EmbedderOutput) -> FsvResult<Value> {
    let output_json = json!({
        "embedder": output.embedder.slug(),
        "source_id": output.source_id,
        "model_version": output.model_version,
        "precision_class": output.precision_class,
        "vector_len": output.vector.len(),
        "vector_l2_norm": l2_norm(&output.vector),
        "vector_sha256": vector_sha256(&output.vector),
        "first_8": output.vector.iter().take(8).copied().collect::<Vec<_>>(),
    });
    write_json_checked(path, &output_json)?;
    let readback: Value = serde_json::from_slice(&fs::read(path)?)?;
    require(
        "forward output readback embedder",
        readback["embedder"] == output_json["embedder"],
    )?;
    require(
        "forward output readback source_id",
        readback["source_id"] == output_json["source_id"],
    )?;
    require(
        "forward output readback vector hash",
        readback["vector_sha256"] == output_json["vector_sha256"],
    )?;
    require(
        "forward output readback precision",
        readback["precision_class"] == output_json["precision_class"],
    )?;
    Ok(output_json)
}

fn edge_unsupported_forward_slot(root: &Path) -> FsvResult<Value> {
    let before = json!({"embedder": "e17", "files_before": file_count(root)?});
    let err = embedder_model_id(EmbedderId::E17).unwrap_err();
    let after = json!({"error_code": err.code(), "files_after": file_count(root)?});
    require(
        "unsupported slot fails closed",
        err.code() == "MEJEPA_EMBED_FORWARD_FAILED",
    )?;
    require(
        "unsupported slot writes no files",
        before["files_before"] == after["files_after"],
    )?;
    Ok(json!({
        "case": "edge_unsupported_forward_slot",
        "before_state": before,
        "trigger": "embedder_model_id(E17)",
        "after_state": after,
        "passes": true,
    }))
}

async fn edge_empty_input(forward: &PretrainedEmbedderForward, root: &Path) -> FsvResult<Value> {
    let before = json!({"text_len": 0, "files_before": file_count(root)?});
    let err = forward
        .forward(&EmbedderInput {
            embedder: EmbedderId::E10,
            text: "".to_string(),
            source_id: "forward-fsv/e10/empty.txt".to_string(),
        })
        .await
        .unwrap_err();
    let after = json!({"error_code": err.code(), "files_after": file_count(root)?});
    require(
        "empty input fails closed",
        err.code() == "MEJEPA_EMBED_INVALID_INPUT",
    )?;
    require(
        "empty input writes no files",
        before["files_before"] == after["files_after"],
    )?;
    Ok(json!({
        "case": "edge_empty_input",
        "before_state": before,
        "trigger": "forward(E10 empty text)",
        "after_state": after,
        "passes": true,
    }))
}

async fn edge_mismatched_input_embedder(
    forward: &PretrainedEmbedderForward,
    root: &Path,
) -> FsvResult<Value> {
    let before =
        json!({"loaded_wrapper": "e10", "input_embedder": "e1", "files_before": file_count(root)?});
    let err = forward
        .forward(&EmbedderInput {
            embedder: EmbedderId::E1,
            text: "mismatched embedder should not be forwarded".to_string(),
            source_id: "forward-fsv/e1/mismatch.txt".to_string(),
        })
        .await
        .unwrap_err();
    let after = json!({"error_code": err.code(), "files_after": file_count(root)?});
    require(
        "mismatched input embedder fails closed",
        err.code() == "MEJEPA_EMBED_FORWARD_FAILED",
    )?;
    require(
        "mismatched input writes no files",
        before["files_before"] == after["files_after"],
    )?;
    Ok(json!({
        "case": "edge_mismatched_input_embedder",
        "before_state": before,
        "trigger": "E10 wrapper receives E1 input",
        "after_state": after,
        "passes": true,
    }))
}

fn reset_root(root: &Path) -> FsvResult<()> {
    if root.exists() {
        fs::remove_dir_all(root)?;
    }
    fs::create_dir_all(root)?;
    fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;
    Ok(())
}

fn write_json_checked(path: &Path, value: &impl Serialize) -> FsvResult<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    if let Some(parent) = path.parent() {
        File::open(parent)?.sync_all()?;
    }
    require("JSON byte readback equality", fs::read(path)? == bytes)?;
    Ok(())
}

fn file_count(path: &Path) -> FsvResult<usize> {
    Ok(fs::read_dir(path)?.count())
}

fn vector_sha256(vector: &[f32]) -> String {
    let mut hasher = Sha256::new();
    for value in vector {
        hasher.update(value.to_le_bytes());
    }
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn l2_norm(vector: &[f32]) -> f64 {
    vector
        .iter()
        .map(|value| (*value as f64) * (*value as f64))
        .sum::<f64>()
        .sqrt()
}

fn require(label: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("forward wrapper FSV assertion failed: {label}").into())
    }
}
