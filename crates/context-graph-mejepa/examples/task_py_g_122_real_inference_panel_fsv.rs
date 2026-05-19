use std::error::Error;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

use context_graph_mejepa::gates::{sha256_bytes, valid_witness_segment};
use context_graph_mejepa::types::{
    AstDiff, DiffHunk, Language, PatchBundle, TaskContext, TaskEnvironment, TaskId, TestId,
};
use context_graph_mejepa::{materialize_inference_panels, MejepaInferError};
use context_graph_mejepa_instruments::materialize::{active_slots, TimeStep};
use context_graph_mejepa_instruments::{
    hash_f32s, CodeInstrumentInput, DiffInstrumentInput, EAstInstrument, ECommitMsgInstrument,
    EDiffInstrument, EProblemInstrument, ETestInstrument, Instrument, InstrumentSlot,
    TextInstrumentInput,
};
use serde::{Deserialize, Serialize};

const DEFAULT_FSV_ROOT: &str = "/tmp/contextgraph-task-py-g-122-real-inference-panel-fsv";

#[derive(Debug, Serialize, Deserialize)]
struct SlotCheck {
    slot: String,
    panel_hash: String,
    direct_encoder_hash: String,
    matches_direct_encoder: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct EdgeCase {
    name: String,
    error_code: Option<String>,
    passed: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct FsvReport {
    issue: String,
    source_of_truth: String,
    happy_path_slot_checks: Vec<SlotCheck>,
    changed_commit_only_changed_expected_slot: bool,
    edge_cases: Vec<EdgeCase>,
    compiler_source_has_legacy_deterministic_vector: bool,
    passes: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let root = std::env::var("CG_MEJEPA_TASK_PY_G_122_FSV_ROOT")
        .unwrap_or_else(|_| DEFAULT_FSV_ROOT.to_string());
    fs::create_dir_all(&root)?;

    let (patch, context) = sample_patch_context("change answer increment");
    let (_panel_t0, panel_t1, _panel_t2) = materialize_inference_panels(&patch, &context)?;
    let direct = direct_inputs(&patch, &context);
    let slot_checks = vec![
        slot_check(
            InstrumentSlot::EAst,
            panel_t1.slot(InstrumentSlot::EAst),
            EAstInstrument.encode(&direct.after_code)?,
        ),
        slot_check(
            InstrumentSlot::ETest,
            panel_t1.slot(InstrumentSlot::ETest),
            ETestInstrument.encode(&direct.tests)?,
        ),
        slot_check(
            InstrumentSlot::EDiff,
            panel_t1.slot(InstrumentSlot::EDiff),
            EDiffInstrument.encode(&direct.diff)?,
        ),
        slot_check(
            InstrumentSlot::EProblem,
            panel_t1.slot(InstrumentSlot::EProblem),
            EProblemInstrument.encode(&direct.problem)?,
        ),
        slot_check(
            InstrumentSlot::ECommitMsg,
            panel_t1.slot(InstrumentSlot::ECommitMsg),
            ECommitMsgInstrument.encode(&direct.commit)?,
        ),
    ];

    let changed_commit_only_changed_expected_slot =
        changed_commit_only_changed_expected_slot(&context)?;
    let edge_cases = vec![
        edge("empty_minimal_source", empty_minimal_source_edge()),
        edge("malformed_source", malformed_source_edge()),
        edge("missing_witness", missing_witness_edge()),
    ];
    let compiler_source =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/compiler.rs"))?;
    let compiler_source_has_legacy_deterministic_vector =
        compiler_source.contains("fn deterministic_vector(");
    let passes = slot_checks.iter().all(|check| check.matches_direct_encoder)
        && changed_commit_only_changed_expected_slot
        && edge_cases.iter().all(|case| case.passed)
        && !compiler_source_has_legacy_deterministic_vector;
    let report = FsvReport {
        issue: "#422 TASK-PY-G-122 real inference panel materialization".to_string(),
        source_of_truth: Path::new(&root)
            .join("task_py_g_122_real_inference_panel_fsv.json")
            .display()
            .to_string(),
        happy_path_slot_checks: slot_checks,
        changed_commit_only_changed_expected_slot,
        edge_cases,
        compiler_source_has_legacy_deterministic_vector,
        passes,
    };
    write_json_readback(
        &Path::new(&root).join("task_py_g_122_real_inference_panel_fsv.json"),
        &report,
    )?;
    if !passes {
        return Err("TASK-PY-G-122 FSV failed".into());
    }
    println!("TASK-PY-G-122 FSV PASS: {root}");
    Ok(())
}

#[derive(Debug)]
struct DirectInputs {
    after_code: CodeInstrumentInput,
    diff: DiffInstrumentInput,
    tests: TextInstrumentInput,
    problem: TextInstrumentInput,
    commit: TextInstrumentInput,
}

fn direct_inputs(patch: &PatchBundle, context: &TaskContext) -> DirectInputs {
    let language = "python".to_string();
    let hunk = &patch.ast_diff.hunks[0];
    let path = hunk.path.to_string_lossy().replace('\\', "/");
    DirectInputs {
        after_code: CodeInstrumentInput {
            language: language.clone(),
            path: path.clone(),
            source: hunk.after.clone(),
        },
        diff: DiffInstrumentInput {
            language: language.clone(),
            path,
            before_source: hunk.before.clone(),
            after_source: hunk.after.clone(),
        },
        tests: TextInstrumentInput {
            text: context
                .tests
                .iter()
                .map(|test| test.0.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
            source_id: format!("{}:tests", context.task_id.0),
            language: Some(language.clone()),
        },
        problem: TextInstrumentInput {
            text: context.problem_statement.clone(),
            source_id: format!("{}:problem", context.task_id.0),
            language: Some(language.clone()),
        },
        commit: TextInstrumentInput {
            text: patch.commit_message.clone(),
            source_id: format!(
                "{}:commit:{}",
                context.task_id.0,
                hex::encode(patch.patch_sha)
            ),
            language: Some(language),
        },
    }
}

fn slot_check(slot: InstrumentSlot, panel_slot: &[f32], direct_vector: Vec<f32>) -> SlotCheck {
    SlotCheck {
        slot: format!("{slot:?}"),
        panel_hash: hash_f32s(panel_slot),
        direct_encoder_hash: hash_f32s(&direct_vector),
        matches_direct_encoder: panel_slot == direct_vector.as_slice(),
    }
}

fn changed_commit_only_changed_expected_slot(
    context: &TaskContext,
) -> Result<bool, MejepaInferError> {
    let (patch_a, _) = sample_patch_context("change answer increment");
    let (patch_b, _) = sample_patch_context("document answer increment precisely");
    let (_, panel_a, _) = materialize_inference_panels(&patch_a, context)?;
    let (_, panel_b, _) = materialize_inference_panels(&patch_b, context)?;
    for slot in active_slots(TimeStep::T1) {
        let equal = panel_a.slot(slot) == panel_b.slot(slot);
        if slot == InstrumentSlot::ECommitMsg {
            if equal {
                return Ok(false);
            }
        } else if !equal {
            return Ok(false);
        }
    }
    Ok(true)
}

fn edge(name: &str, result: Result<(), MejepaInferError>) -> EdgeCase {
    match result {
        Ok(()) => EdgeCase {
            name: name.to_string(),
            error_code: None,
            passed: false,
        },
        Err(err) => EdgeCase {
            name: name.to_string(),
            error_code: Some(err.code().to_string()),
            passed: matches!(
                err.code(),
                "MEJEPA_INFER_INVALID_INPUT" | "MEJEPA_INFER_INSTRUMENT"
            ),
        },
    }
}

fn empty_minimal_source_edge() -> Result<(), MejepaInferError> {
    let (mut patch, context) = sample_patch_context("change answer increment");
    patch.ast_diff.hunks[0].before.clear();
    materialize_inference_panels(&patch, &context).map(|_| ())
}

fn malformed_source_edge() -> Result<(), MejepaInferError> {
    let (mut patch, context) = sample_patch_context("change answer increment");
    patch.ast_diff.hunks[0].after = "def broken(:\n".to_string();
    materialize_inference_panels(&patch, &context).map(|_| ())
}

fn missing_witness_edge() -> Result<(), MejepaInferError> {
    let (mut patch, context) = sample_patch_context("change answer increment");
    patch.witness_chain_segment.clear();
    materialize_inference_panels(&patch, &context).map(|_| ())
}

fn sample_patch_context(commit_message: &str) -> (PatchBundle, TaskContext) {
    let before = "def answer(value: int) -> int:\n    return value + 1\n";
    let after = "def answer(value: int) -> int:\n    return value + 2\n";
    let patch = PatchBundle::try_new(
        AstDiff {
            hunks: vec![DiffHunk {
                path: PathBuf::from("src/example.py"),
                pre_sha: sha256_bytes(before.as_bytes()),
                post_sha: sha256_bytes(after.as_bytes()),
                before: before.to_string(),
                after: after.to_string(),
            }],
        },
        valid_witness_segment(),
        commit_message.to_string(),
        sha256_bytes(b"stable-patch-sha"),
    )
    .expect("sample patch is valid");
    let context = TaskContext {
        task_id: TaskId("task-py-g-122".to_string()),
        session_id: [12; 16],
        language: Language::Python,
        problem_statement: "verify the answer increment changes by one".to_string(),
        tests: vec![TestId("tests/test_example.py::test_answer".to_string())],
        environment: TaskEnvironment {
            repo_root: PathBuf::from("/zfs/archive/contextgraph/test-repo"),
            python_version: Some("3.11".to_string()),
            os: "linux".to_string(),
        },
        claim_graph: None,
        skill_citations: vec![],
    };
    (patch, context)
}

fn write_json_readback<T>(path: &Path, value: &T) -> Result<(), Box<dyn Error>>
where
    T: Serialize + for<'de> Deserialize<'de>,
{
    let bytes = serde_json::to_vec_pretty(value)?;
    let mut file = File::create(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    let readback = fs::read(path)?;
    if readback != bytes {
        return Err(format!("readback mismatch for {}", path.display()).into());
    }
    let decoded: T = serde_json::from_slice(&readback)?;
    let decoded_bytes = serde_json::to_vec_pretty(&decoded)?;
    if decoded_bytes != bytes {
        return Err(format!("JSON roundtrip mismatch for {}", path.display()).into());
    }
    Ok(())
}
