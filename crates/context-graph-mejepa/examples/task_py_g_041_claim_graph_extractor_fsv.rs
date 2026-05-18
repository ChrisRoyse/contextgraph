use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};
use context_graph_mejepa::{
    claim_reconciliation_records, count_claim_reconciliation_records, current_unix_ms,
    extract_claim_graph, open_infer_rocksdb, read_claim_reconciliation_records, reconcile_claims,
    reconcile_claims_at_after_sha, write_claim_reconciliation_records_sync_readback,
    write_json_0600, AgentClaimGraph, ClaimKind, ReconciliationStatus, ShiftEntry,
};
use context_graph_mejepa_cf::CF_MEJEPA_CLAIM_RECONCILIATION;
use serde::Serialize;
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/task-py-g-041-claim-graph-extractor-fsv";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClaimGraphFsvReport {
    task_id: String,
    issue: String,
    status: String,
    all_passed: bool,
    run_root: PathBuf,
    source_of_truth_cf: String,
    db_path: PathBuf,
    repo_path: PathBuf,
    repo_after_sha: String,
    real_response_source: String,
    agent_history_files_seen: usize,
    agent_history_response_rows: usize,
    real_response_strings_sampled: usize,
    real_response_strings_with_claims: usize,
    green_rows_total: usize,
    green_added_modified_rows: usize,
    green_added_modified_matched: usize,
    green_added_modified_match_rate: f32,
    cf_rows_before: usize,
    cf_rows_after: usize,
    cf_rows_reopened: usize,
    write_summary_rows: usize,
    reopened_rows_equal: bool,
    edge_cases: Vec<EdgeCase>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EdgeCase {
    name: String,
    expected: String,
    actual: String,
    passed: bool,
}

fn main() -> Result<()> {
    let run_id = format!("run-{}", current_unix_ms());
    let run_root = PathBuf::from(FSV_ROOT).join(&run_id);
    fs::create_dir_all(&run_root)?;
    let repo_path = run_root.join("repo");
    let db_path = run_root.join("rocksdb");
    build_green_repo(&repo_path)?;
    let repo_after_sha = git_output(&repo_path, ["rev-parse", "HEAD"])?;
    let shift = shift_for(&repo_path, "src/calc.py", "green-shift-0001")?;

    let agent_history =
        agent_history_stats(Path::new("/mnt/d/contextgraph/runtime/agent-history"))?;
    let (real_responses, real_response_source) =
        load_real_response_strings(Path::new("/mnt/d/contextgraph/runtime/test-outcomes"), 50)?;
    let real_graphs = real_responses
        .iter()
        .map(|response| extract_claim_graph(response))
        .collect::<Result<Vec<_>, _>>()
        .context("extract real response claim graphs")?;
    let real_with_claims = real_graphs
        .iter()
        .filter(|graph| !graph.claims.is_empty())
        .count();

    let green_claims = green_claim_strings();
    let graph = extract_claim_graph(&green_claims.join("\n"))?;
    let green_rows =
        reconcile_claims_at_after_sha(&graph, &repo_path, repo_after_sha.trim(), &[shift])
            .context("reconcile green claim graph")?;
    let added_modified_total = green_rows
        .iter()
        .filter(|row| matches!(row.claim.kind, ClaimKind::Added(_) | ClaimKind::Modified(_)))
        .count();
    let added_modified_matched = green_rows
        .iter()
        .filter(|row| {
            matches!(row.claim.kind, ClaimKind::Added(_) | ClaimKind::Modified(_))
                && row.status == ReconciliationStatus::Matched
        })
        .count();
    let match_rate = added_modified_matched as f32 / added_modified_total.max(1) as f32;

    let edge_cases = run_edge_cases(&repo_path, repo_after_sha.trim())?;
    let db = open_infer_rocksdb(&db_path)?;
    let cf_rows_before = count_claim_reconciliation_records(db.as_ref())?;
    let prediction_id = [0x41; 16];
    let records =
        claim_reconciliation_records(prediction_id, &green_rows, &repo_path, current_unix_ms())?;
    let summary = write_claim_reconciliation_records_sync_readback(db.as_ref(), &records)?;
    let cf_rows_after = count_claim_reconciliation_records(db.as_ref())?;
    drop(db);

    let reopened = open_infer_rocksdb(&db_path)?;
    let reopened_rows = read_claim_reconciliation_records(reopened.as_ref(), prediction_id)?;
    let cf_rows_reopened = count_claim_reconciliation_records(reopened.as_ref())?;
    let reopened_rows_equal = reopened_rows == records;

    let all_passed = real_responses.len() == 50
        && real_with_claims == 50
        && added_modified_total >= 10
        && match_rate >= 0.90
        && cf_rows_before == 0
        && cf_rows_after == records.len()
        && cf_rows_reopened == records.len()
        && summary.rows_written == records.len()
        && reopened_rows_equal
        && edge_cases.iter().all(|case| case.passed);

    let report = ClaimGraphFsvReport {
        task_id: "TASK-PY-G-041".to_string(),
        issue: "https://github.com/ChrisRoyse/contextgraph/issues/260".to_string(),
        status: if all_passed { "pass" } else { "fail" }.to_string(),
        all_passed,
        run_root: run_root.clone(),
        source_of_truth_cf: CF_MEJEPA_CLAIM_RECONCILIATION.to_string(),
        db_path,
        repo_path,
        repo_after_sha: repo_after_sha.trim().to_string(),
        real_response_source,
        agent_history_files_seen: agent_history.0,
        agent_history_response_rows: agent_history.1,
        real_response_strings_sampled: real_responses.len(),
        real_response_strings_with_claims: real_with_claims,
        green_rows_total: green_rows.len(),
        green_added_modified_rows: added_modified_total,
        green_added_modified_matched: added_modified_matched,
        green_added_modified_match_rate: match_rate,
        cf_rows_before,
        cf_rows_after,
        cf_rows_reopened,
        write_summary_rows: summary.rows_written,
        reopened_rows_equal,
        edge_cases,
    };

    let report_path = run_root.join("claim_graph_extractor_fsv.json");
    write_json_0600(&report_path, &report)?;
    let report_sha256 = sha256_file(&report_path)?;
    write_json_0600(
        &PathBuf::from(FSV_ROOT).join("claim_graph_extractor_fsv.latest.json"),
        &report,
    )?;

    if !all_passed {
        bail!("TASK-PY-G-041 FSV failed: {}", report_path.display());
    }
    println!("TASK-PY-G-041 FSV PASS");
    println!("report_path={}", report_path.display());
    println!("report_sha256={report_sha256}");
    println!("real_response_strings_with_claims={real_with_claims}/50");
    println!("green_added_modified_match_rate={match_rate:.3}");
    Ok(())
}

fn build_green_repo(repo: &Path) -> Result<()> {
    fs::create_dir_all(repo.join("src"))?;
    fs::create_dir_all(repo.join("tests"))?;
    fs::write(
        repo.join("src/calc.py"),
        "def add_one(value):\n    return value + 1\n\n\ndef multiply(value, factor):\n    return value * factor\n\n\ndef normalize_name(name):\n    return name.strip().lower()\n",
    )?;
    fs::write(
        repo.join("tests/test_calc.py"),
        "from src.calc import add_one, multiply\n\n\ndef test_add_one():\n    assert add_one(2) == 3\n\n\ndef test_multiply():\n    assert multiply(3, 4) == 12\n",
    )?;
    run_git(repo, ["init"])?;
    run_git(repo, ["config", "user.email", "fsv@example.invalid"])?;
    run_git(repo, ["config", "user.name", "ContextGraph FSV"])?;
    run_git(repo, ["add", "."])?;
    run_git(repo, ["commit", "-m", "claim graph fsv green repo"])?;
    Ok(())
}

fn green_claim_strings() -> Vec<String> {
    let mut rows = Vec::new();
    for _ in 0..6 {
        rows.push("I added add_one in src/calc.py at line 1.".to_string());
        rows.push("I modified multiply in src/calc.py at line 5.".to_string());
    }
    rows.push("The test tests/test_calc.py::test_add_one will pass now.".to_string());
    rows
}

fn run_edge_cases(repo: &Path, repo_after_sha: &str) -> Result<Vec<EdgeCase>> {
    let mut cases = Vec::new();
    let empty = extract_claim_graph("")?;
    let empty_rows = reconcile_claims(&empty, repo, &[])?;
    cases.push(edge(
        "empty_prose",
        "empty claim graph and zero reconciliation rows",
        format!("claims={} rows={}", empty.claims.len(), empty_rows.len()),
        empty.claims.is_empty() && empty_rows.is_empty(),
    ));

    let missing = extract_claim_graph("I added missing_helper in src/missing.py at line 1.")?;
    let missing_rows = reconcile_claims(&missing, repo, &[])?;
    cases.push(edge(
        "nonexistent_file",
        "status Missing with missing file evidence",
        format!("{:?}", missing_rows[0].status),
        missing_rows[0].status == ReconciliationStatus::Missing,
    ));

    let out_of_range = extract_claim_graph("I modified add_one in src/calc.py at line 999.")?;
    let out_rows = reconcile_claims(&out_of_range, repo, &[])?;
    let out_records = claim_reconciliation_records([0x42; 16], &out_rows, repo, current_unix_ms())?;
    cases.push(edge(
        "line_out_of_range",
        "status Ambiguous with reason LINE_OUT_OF_RANGE",
        format!("{:?} {:?}", out_rows[0].status, out_records[0].reason_code),
        out_rows[0].status == ReconciliationStatus::Ambiguous
            && out_records[0].reason_code.as_deref() == Some("LINE_OUT_OF_RANGE"),
    ));

    let will_pass = extract_claim_graph(
        "The test tests/test_missing.py::test_absent will pass after this change.",
    )?;
    let will_pass_rows = reconcile_claims(&will_pass, repo, &[])?;
    cases.push(edge(
        "will_pass_missing_test",
        "status Missing for nonexistent test file",
        format!("{:?}", will_pass_rows[0].status),
        will_pass_rows[0].status == ReconciliationStatus::Missing,
    ));

    fs::write(repo.join("src/non_utf8.py"), [0xff, 0xfe, 0xfd])?;
    let non_utf8 = extract_claim_graph("I modified bad in src/non_utf8.py at line 1.")?;
    let non_utf8_err = reconcile_claims(&non_utf8, repo, &[]).unwrap_err();
    cases.push(edge(
        "non_utf8",
        "CLAIM_GRAPH_UNSUPPORTED_LANGUAGE",
        non_utf8_err.code().to_string(),
        non_utf8_err.code() == "CLAIM_GRAPH_UNSUPPORTED_LANGUAGE",
    ));

    fs::write(repo.join("README.md"), "# not python\n")?;
    let non_python = extract_claim_graph("I modified README in README.md at line 1.")?;
    let non_python_err = reconcile_claims(&non_python, repo, &[]).unwrap_err();
    cases.push(edge(
        "non_python",
        "CLAIM_GRAPH_UNSUPPORTED_LANGUAGE",
        non_python_err.code().to_string(),
        non_python_err.code() == "CLAIM_GRAPH_UNSUPPORTED_LANGUAGE",
    ));

    let unreachable = extract_claim_graph("I modified add_one in src/calc.py at line 1.")?;
    let after_err =
        reconcile_claims_at_after_sha(&unreachable, repo, &"0".repeat(40), &[]).unwrap_err();
    cases.push(edge(
        "after_sha_unreachable",
        "fail closed for unreachable after sha",
        after_err.code().to_string(),
        after_err.code() == "MEJEPA_INFER_INVALID_INPUT",
    ));

    let reachable_rows = reconcile_claims_at_after_sha(&unreachable, repo, repo_after_sha, &[])?;
    cases.push(edge(
        "after_sha_reachable",
        "reachable after sha reconciles normally",
        format!("{:?}", reachable_rows[0].status),
        reachable_rows[0].status == ReconciliationStatus::Matched,
    ));

    Ok(cases)
}

fn load_real_response_strings(root: &Path, limit: usize) -> Result<(Vec<String>, String)> {
    let mut responses = Vec::new();
    let mut seen = BTreeSet::new();
    for path in jsonl_files(root)? {
        let text = fs::read_to_string(&path)?;
        for line in text.lines() {
            let value: serde_json::Value = serde_json::from_str(line)?;
            let Some(command) = value.get("command").and_then(|value| value.as_str()) else {
                continue;
            };
            if !command.contains("gh issue comment") {
                continue;
            }
            for response in response_strings_from_command(command) {
                push_claim_response(&response, &mut responses, &mut seen)?;
                if responses.len() >= limit {
                    return Ok((
                        responses,
                        "/mnt/d/contextgraph/runtime/test-outcomes/*.jsonl gh issue comment payloads"
                            .to_string(),
                    ));
                }
            }
        }
    }
    if responses.len() < limit {
        load_github_issue_comment_responses(limit, &mut responses, &mut seen)?;
    }
    if responses.len() < limit {
        bail!(
            "only found {} real response strings with claims",
            responses.len()
        );
    }
    Ok((
        responses,
        "/mnt/d/contextgraph/runtime/test-outcomes/*.jsonl plus GitHub issue comments".to_string(),
    ))
}

fn push_claim_response(
    response: &str,
    responses: &mut Vec<String>,
    seen: &mut BTreeSet<String>,
) -> Result<()> {
    let trimmed = response.trim();
    if trimmed.is_empty() || !seen.insert(trimmed.to_string()) {
        return Ok(());
    }
    let graph = extract_claim_graph(trimmed)?;
    if !graph.claims.is_empty() {
        responses.push(trimmed.to_string());
    }
    Ok(())
}

fn load_github_issue_comment_responses(
    limit: usize,
    responses: &mut Vec<String>,
    seen: &mut BTreeSet<String>,
) -> Result<()> {
    let output = Command::new("gh")
        .args([
            "api",
            "repos/ChrisRoyse/contextgraph/issues/comments?per_page=100",
        ])
        .output()
        .context("query GitHub issue comments")?;
    if !output.status.success() {
        bail!(
            "gh api issue comments failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let comments: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    let Some(comments) = comments.as_array() else {
        bail!("gh api issue comments returned non-array JSON");
    };
    for comment in comments {
        let Some(body) = comment.get("body").and_then(|value| value.as_str()) else {
            continue;
        };
        push_claim_response(body, responses, seen)?;
        if responses.len() >= limit {
            return Ok(());
        }
    }
    Ok(())
}

fn response_strings_from_command(command: &str) -> Vec<String> {
    let mut out = Vec::new();
    collect_flag_values(command, "--body", &mut out);
    collect_flag_values(command, "--comment", &mut out);
    if out.is_empty() {
        out.push(command.to_string());
    }
    out
}

fn collect_flag_values(command: &str, flag: &str, out: &mut Vec<String>) {
    let mut cursor = 0usize;
    while let Some(relative) = command[cursor..].find(flag) {
        let flag_start = cursor + relative;
        let value_start = flag_start + flag.len();
        if command[value_start..]
            .chars()
            .next()
            .is_some_and(|ch| !ch.is_whitespace())
        {
            cursor = value_start;
            continue;
        }
        let Some((value, next_cursor)) = shell_flag_value(command, value_start) else {
            cursor = value_start;
            continue;
        };
        if !value.trim().is_empty() {
            out.push(value);
        }
        cursor = next_cursor;
    }
}

fn shell_flag_value(command: &str, start: usize) -> Option<(String, usize)> {
    let mut cursor = start;
    while let Some(ch) = command[cursor..].chars().next() {
        if !ch.is_whitespace() {
            break;
        }
        cursor += ch.len_utf8();
    }
    let quote = command[cursor..].chars().next()?;
    if quote != '"' && quote != '\'' {
        let end = command[cursor..]
            .find(char::is_whitespace)
            .map(|relative| cursor + relative)
            .unwrap_or(command.len());
        return Some((command[cursor..end].to_string(), end));
    }
    cursor += quote.len_utf8();
    let mut value = String::new();
    let mut escaped = false;
    for (relative, ch) in command[cursor..].char_indices() {
        if escaped {
            value.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == quote {
            return Some((value, cursor + relative + ch.len_utf8()));
        }
        value.push(ch);
    }
    None
}

fn jsonl_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut stack = vec![root.to_path_buf()];
    let mut out = Vec::new();
    while let Some(path) = stack.pop() {
        for entry in fs::read_dir(&path).with_context(|| format!("read {}", path.display()))? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
                out.push(path);
            }
        }
    }
    out.sort();
    Ok(out)
}

fn agent_history_stats(root: &Path) -> Result<(usize, usize)> {
    let files = jsonl_files(root).unwrap_or_default();
    let mut response_rows = 0usize;
    for path in &files {
        let text = fs::read_to_string(path)?;
        for line in text.lines() {
            let value: serde_json::Value = serde_json::from_str(line)?;
            if value.get("agent_response").is_some() || value.get("response").is_some() {
                response_rows += 1;
            }
        }
    }
    Ok((files.len(), response_rows))
}

fn shift_for(repo: &Path, rel_path: &str, shift_id: &str) -> Result<ShiftEntry> {
    let after_sha = sha256_file(&repo.join(rel_path))?;
    Ok(ShiftEntry {
        shift_id: shift_id.to_string(),
        file: PathBuf::from(rel_path),
        before_sha: [0u8; 32],
        after_sha: hex_to_32(&after_sha)?,
    })
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path)?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn hex_to_32(value: &str) -> Result<[u8; 32]> {
    let bytes = hex::decode(value)?;
    if bytes.len() != 32 {
        bail!("expected 32-byte sha");
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&bytes);
    Ok(out)
}

fn run_git<const N: usize>(repo: &Path, args: [&str; N]) -> Result<()> {
    let status = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .status()
        .with_context(|| format!("run git in {}", repo.display()))?;
    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("git command failed in {}", repo.display()))
    }
}

fn git_output<const N: usize>(repo: &Path, args: [&str; N]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()
        .with_context(|| format!("run git in {}", repo.display()))?;
    if !output.status.success() {
        bail!("git command failed in {}", repo.display());
    }
    Ok(String::from_utf8(output.stdout)?)
}

fn edge(name: &str, expected: &str, actual: String, passed: bool) -> EdgeCase {
    EdgeCase {
        name: name.to_string(),
        expected: expected.to_string(),
        actual,
        passed,
    }
}

#[allow(dead_code)]
fn _graph_claim_count(graph: &AgentClaimGraph) -> usize {
    graph.claims.len()
}
