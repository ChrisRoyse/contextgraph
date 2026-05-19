// Smoke FSV for AST chunking -> embedder routing -> Phase 1 panel materialization.

use context_graph_core::memory::ast::{
    chunk_path, chunk_with_options, route_for_key, AstChunkOptions, Language, RoutingKey,
    RoutingResult,
};
use context_graph_mejepa_instruments::{
    CodeInstrumentInput, Diagnostic, DiagnosticSeverity, DiffInstrumentInput, EAstInstrument,
    ECfgInstrument, ECommitMsgInstrument, EDataFlowInstrument, EDiffInstrument, EOracleInstrument,
    EProblemInstrument, EReasoningInstrument, ERuntimeInstrument, EStaticAnalysisInstrument,
    ETestInstrument, ETraceInstrument, ETypeGraphInstrument, EWitnessInstrument, InstrumentSlot,
    OracleVerdict, Panel, PanelBuilder, PerTestOutcome, ReasoningEvent, ReasoningInput,
    RuntimeInput, ScalarInput, ScalarsInstrument, StaticAnalysisInput, TestOutcome,
    TextInstrumentInput, TraceEvent, TraceEventKind, TraceInput, WitnessChainInput,
};
use context_graph_witness::{shake256_32, WitnessEntry, ZERO_HASH};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

type FsvResult<T> = Result<T, Box<dyn std::error::Error>>;

fn main() -> FsvResult<()> {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf();
    let root =
        PathBuf::from("/zfs/archive/contextgraph/fsv/contextgraph-mejepa-ast-embedder-smoke-fsv");
    if root.exists() {
        fs::remove_dir_all(&root)?;
    }
    fs::create_dir_all(&root)?;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;

    let manifest_path = repo.join("tasks/swebench-lite/sympy__sympy-20639.json");
    let manifest: Value = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    let samples = chunk_real_samples(&repo)?;
    require("sample count", samples.len() == 11)?;

    let python_source = fs::read_to_string(repo.join("samplecode/sample.py"))?;
    let panel = build_full_panel(&python_source, &manifest)?;
    let panel_path = root.join("panel.json");
    write_checked(
        &panel_path,
        serde_json::to_string_pretty(&panel)?.as_bytes(),
    )?;
    let panel_readback: Panel = serde_json::from_str(&fs::read_to_string(&panel_path)?)?;
    require("panel readback equality", panel_readback == panel)?;
    require(
        "panel all slots filled",
        InstrumentSlot::all()
            .iter()
            .all(|slot| panel_readback.is_filled(*slot)),
    )?;

    let chunks_path = root.join("chunks.json");
    write_checked(
        &chunks_path,
        serde_json::to_string_pretty(&samples)?.as_bytes(),
    )?;
    let chunk_readback: Value = serde_json::from_str(&fs::read_to_string(&chunks_path)?)?;
    require(
        "chunk readback contains 11 samples",
        chunk_readback
            .as_array()
            .is_some_and(|items| items.len() == 11),
    )?;

    let edge_cases = json!({
        "empty_python_source": edge_empty_source(),
        "invalid_python_syntax": edge_invalid_python(),
        "mixed_php_html": edge_mixed_php(&repo)?,
    });
    require(
        "edge cases pass",
        edge_cases
            .as_object()
            .expect("edge object")
            .values()
            .all(|value| value["passes"] == Value::Bool(true)),
    )?;

    let evidence = json!({
        "source_of_truth": {
            "root": root.display().to_string(),
            "swebench_manifest": manifest_path.display().to_string(),
            "chunk_json": chunks_path.display().to_string(),
            "panel_json": panel_path.display().to_string()
        },
        "happy_path": {
            "before_state": {
                "sample_files": 11,
                "swebench_instance_id": manifest["instance_id"],
                "problem_statement_bytes": manifest["problem_statement"].as_str().unwrap_or("").len(),
                "python_source_bytes": python_source.len()
            },
            "after_state": {
                "panel_dim": panel_readback.data().len(),
                "panel_filled_mask": panel_readback.filled_mask(),
                "filled_slots": InstrumentSlot::all().iter().map(InstrumentSlot::slug).collect::<Vec<_>>(),
                "sample_reports": samples,
                "route_internal": internal_route_report()?,
                "e_ast_first_8": &panel_readback.slot(InstrumentSlot::EAst)[0..8],
                "e_diff_first_8": &panel_readback.slot(InstrumentSlot::EDiff)[0..8],
                "e_problem_first_8": &panel_readback.slot(InstrumentSlot::EProblem)[0..8]
            },
            "passes": true
        },
        "edge_cases": edge_cases,
        "passes": true
    });
    let evidence_path = root.join("ast-embedder-smoke-evidence.json");
    let text = serde_json::to_string_pretty(&evidence)?;
    write_checked(&evidence_path, text.as_bytes())?;
    let evidence_readback: Value = serde_json::from_str(&fs::read_to_string(&evidence_path)?)?;
    require(
        "evidence readback passes",
        evidence_readback["passes"] == Value::Bool(true),
    )?;
    println!(
        "AST embedder smoke FSV evidence: {}",
        evidence_path.display()
    );
    println!("{text}");
    Ok(())
}

fn chunk_real_samples(repo: &Path) -> FsvResult<Vec<Value>> {
    let sample_specs = [
        ("samplecode/sample.py", Language::Python, None),
        ("samplecode/sample.rs", Language::Rust, None),
        ("samplecode/sample.js", Language::JavaScript, None),
        ("samplecode/sample.ts", Language::TypeScript, None),
        ("samplecode/sample.go", Language::Go, None),
        ("samplecode/Sample.java", Language::Java, None),
        ("samplecode/sample.c", Language::C, None),
        ("samplecode/sample.cpp", Language::Cpp, None),
        ("samplecode/sample.cs", Language::CSharp, None),
        ("samplecode/sample.rb", Language::Ruby, None),
        (
            "samplecode/clean_sample.php",
            Language::Php,
            Some(
                "<?php\nnamespace Fsv;\nclass Solver { public function compute(int $x): int { return $x + 1; } }\nfunction test_compute(): void { $s = new Solver(); if ($s->compute(1) !== 2) { throw new \\Exception('bad'); } }\n",
            ),
        ),
    ];
    sample_specs
        .iter()
        .map(|(rel, language, source)| chunk_one(repo, rel, *language, source.map(str::to_string)))
        .collect()
}

fn chunk_one(
    repo: &Path,
    rel: &str,
    language: Language,
    override_source: Option<String>,
) -> FsvResult<Value> {
    let path = repo.join(rel);
    let source = match override_source {
        Some(source) => source,
        None => fs::read_to_string(&path)?,
    };
    let chunks = chunk_path(source.as_bytes(), &path.display().to_string())?;
    require(
        "language matches request",
        chunks.iter().all(|c| c.language == language),
    )?;
    require("non-empty chunks", !chunks.is_empty())?;
    let mut entity_counts = BTreeMap::<String, usize>::new();
    let mut routed = BTreeSet::<String>::new();
    for chunk in &chunks {
        require("chunk has content", !chunk.content.trim().is_empty())?;
        require(
            "chunk route non-empty",
            !chunk.embedder_routing_set.is_empty(),
        )?;
        *entity_counts
            .entry(format!("{:?}", chunk.entity_type))
            .or_default() += 1;
        routed.extend(
            chunk
                .embedder_routing_set
                .iter()
                .map(|id| id.slug().to_string()),
        );
    }
    Ok(json!({
        "before_state": {
            "path": rel,
            "language": language.slug(),
            "source_bytes": source.len(),
            "source_hash": hex32(&shake256_32(source.as_bytes()))
        },
        "after_state": {
            "chunk_count": chunks.len(),
            "entity_counts": entity_counts,
            "routed_embedders": routed.into_iter().collect::<Vec<_>>(),
            "first_chunk": {
                "entity_type": format!("{:?}", chunks[0].entity_type),
                "line_start": chunks[0].line_start,
                "line_end": chunks[0].line_end,
                "sha256": chunks[0].sha256,
            }
        },
        "passes": true
    }))
}

fn build_full_panel(source: &str, manifest: &Value) -> FsvResult<Panel> {
    let code = CodeInstrumentInput {
        language: "python".into(),
        path: "samplecode/sample.py".into(),
        source: source.into(),
    };
    let diff = DiffInstrumentInput {
        language: "python".into(),
        path: "samplecode/sample.py".into(),
        before_source: source.into(),
        after_source: source.replace("return n\n", "return n + 0\n"),
    };
    let problem = manifest["problem_statement"].as_str().unwrap_or("");
    let text = |kind: &str, body: &str| TextInstrumentInput {
        text: body.to_string(),
        source_id: format!("sympy__sympy-20639::{kind}"),
        language: Some("python".into()),
    };
    let mut builder = PanelBuilder::new();
    builder.set_from_instrument(&EAstInstrument, &code)?;
    builder.set_from_instrument(&ECfgInstrument, &code)?;
    builder.set_from_instrument(&EDataFlowInstrument, &code)?;
    builder.set_from_instrument(&ETypeGraphInstrument, &code)?;
    builder.set_from_instrument(&ETestInstrument, &text("test", "test_issue_6285"))?;
    builder.set_from_instrument(&ETraceInstrument, &trace_input())?;
    builder.set_from_instrument(&EDiffInstrument, &diff)?;
    builder.set_from_instrument(&EWitnessInstrument, &witness_chain()?)?;
    builder.set_from_instrument(&EOracleInstrument, &oracle_from_manifest(manifest))?;
    builder.set_from_instrument(&EProblemInstrument, &text("problem", problem))?;
    builder.set_from_instrument(
        &ECommitMsgInstrument,
        &text("commit", "sample Python AST smoke"),
    )?;
    builder.set_from_instrument(&EStaticAnalysisInstrument, &static_analysis(source))?;
    builder.set_from_instrument(&ERuntimeInstrument, &runtime_input())?;
    builder.set_from_instrument(&EReasoningInstrument, &reasoning_input())?;
    builder.set_from_instrument(&ScalarsInstrument, &scalar_input())?;
    builder.require_slots(&InstrumentSlot::all())?;
    Ok(builder.build()?)
}

fn internal_route_report() -> FsvResult<Value> {
    let keys = [
        RoutingKey::FunctionSignature,
        RoutingKey::TestAssertion,
        RoutingKey::DiffHunk,
        RoutingKey::AstNodeSequence,
        RoutingKey::CfgBasicBlock,
        RoutingKey::DefUseEdge,
    ];
    let mut rows = Vec::new();
    for key in keys {
        rows.push(match route_for_key(key, Some(Language::Python))? {
            RoutingResult::Embedders(ids) => json!({
                "key": format!("{key:?}"),
                "embedders": ids.iter().map(|id| id.slug()).collect::<Vec<_>>()
            }),
            RoutingResult::HandledByInstrument(slot) => {
                json!({"key": format!("{key:?}"), "instrument": slot.slug()})
            }
        });
    }
    Ok(Value::Array(rows))
}

fn oracle_from_manifest(manifest: &Value) -> OracleVerdict {
    let per_test = manifest["fail_to_pass"]
        .as_array()
        .map(|tests| {
            tests
                .iter()
                .map(|value| PerTestOutcome {
                    test_id: value.as_str().unwrap_or("unknown_test").to_string(),
                    outcome: TestOutcome::Pass,
                    runtime_ms: 1,
                })
                .collect()
        })
        .unwrap_or_default();
    OracleVerdict {
        per_test,
        exception: None,
    }
}

fn trace_input() -> TraceInput {
    TraceInput {
        events: vec![TraceEvent {
            function_id: "samplecode.sample.fibonacci".into(),
            line: 13,
            value_hash: "sha256:sample-trace".into(),
            timestamp_ms: 7,
            kind: TraceEventKind::LineHit,
        }],
    }
}

fn witness_chain() -> FsvResult<WitnessChainInput> {
    let first = WitnessEntry::new(ZERO_HASH, shake256_32(b"ast-smoke-source"), 10, 1);
    let second = WitnessEntry::new(first.chain_hash(), shake256_32(b"ast-smoke-panel"), 20, 2);
    Ok(WitnessChainInput::from_entries(&[first, second])?)
}

fn static_analysis(source: &str) -> StaticAnalysisInput {
    StaticAnalysisInput {
        source_text: source.into(),
        diagnostics: vec![Diagnostic {
            tool: "pyright".into(),
            severity: DiagnosticSeverity::Warning,
            category: "reportUnusedImport".into(),
            line: 3,
            column: 1,
        }],
        churn_30d: Some(2),
    }
}

fn runtime_input() -> RuntimeInput {
    RuntimeInput {
        wall_time_ms: 42,
        peak_rss_bytes: 32 * 1024 * 1024,
        exit_code: 0,
        timed_out: false,
        coverage_percent: Some(91.0),
        network_events: 0,
        filesystem_writes: 1,
    }
}

fn reasoning_input() -> ReasoningInput {
    ReasoningInput {
        task_id: "sympy__sympy-20639".into(),
        transcript: "Read real samplecode, chunked AST entities, routed embedders, built panel."
            .into(),
        events: vec![ReasoningEvent {
            actor: "codex".into(),
            event_type: "smoke_fsv".into(),
            text: "verified AST-to-panel path".into(),
        }],
    }
}

fn scalar_input() -> ScalarInput {
    ScalarInput {
        bfs_depth: 2,
        blame_age_days: 14,
        churn_lines_30d: 8,
        coverage_delta: 1.5,
        repo_health_score: 0.95,
        files_touched: 1,
        hunks_touched: 1,
    }
}

fn edge_empty_source() -> Value {
    let before = json!({"language": "python", "source_bytes": 3});
    let err = chunk_with_options(
        b" \n\t",
        Language::Python,
        &AstChunkOptions::for_path("empty.py"),
    )
    .unwrap_err();
    json!({"before_state": before, "after_state": {"error_code": err.code()}, "passes": err.code() == "MEJEPA_CHUNKER_EMPTY_SOURCE"})
}

fn edge_invalid_python() -> Value {
    let source = b"def broken(:\n    return 1\n";
    let err = chunk_with_options(
        source,
        Language::Python,
        &AstChunkOptions::for_path("broken.py"),
    )
    .unwrap_err();
    json!({"before_state": {"language": "python", "source_bytes": source.len()}, "after_state": {"error_code": err.code()}, "passes": err.code() == "MEJEPA_CHUNKER_PARSE_FAILED"})
}

fn edge_mixed_php(repo: &Path) -> FsvResult<Value> {
    let source = fs::read_to_string(repo.join("samplecode/sample.php"))?;
    let err = chunk_with_options(
        source.as_bytes(),
        Language::Php,
        &AstChunkOptions::for_path("samplecode/sample.php"),
    )
    .unwrap_err();
    Ok(
        json!({"before_state": {"language": "php", "source_bytes": source.len()}, "after_state": {"error_code": err.code()}, "passes": err.code() == "MEJEPA_CHUNKER_MIXED_LANGUAGE"}),
    )
}

fn write_checked(path: &Path, bytes: &[u8]) -> FsvResult<()> {
    let mut file = File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    if let Some(parent) = path.parent() {
        File::open(parent)?.sync_all()?;
    }
    require("readback bytes match", fs::read(path)? == bytes)?;
    Ok(())
}

fn hex32(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn require(label: &str, condition: bool) -> FsvResult<()> {
    if condition {
        Ok(())
    } else {
        Err(format!("AST embedder smoke FSV invariant failed: {label}").into())
    }
}
