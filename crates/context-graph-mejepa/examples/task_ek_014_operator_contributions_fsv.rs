use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{ensure, Context, Result};
use context_graph_mejepa::{
    count_cf, current_unix_ms, open_infer_rocksdb, operator_contribution_report_for_rows,
    operator_quality_score, read_all_operator_contributions, read_operator_contribution,
    render_operator_contributions_weekly_section, write_json_0600,
    write_operator_contribution_sync_readback, DownstreamOutcomeKind, DownstreamOutcomeRef,
    OperatorContribution, OperatorContributionEventKind, OperatorContributionPayload,
};
use context_graph_mejepa_cf::CF_MEJEPA_OPERATOR_CONTRIBUTIONS;
use serde_json::json;
use sha2::{Digest, Sha256};

const FSV_ROOT: &str = "/mnt/d/contextgraph/fsv/task-ek-014-operator-contributions-fsv";
const SYNTHETIC_BASE_TS: i64 = 1_800_000_000_000;
const WEEK_MS: i64 = 7 * 24 * 60 * 60 * 1000;

fn main() -> Result<()> {
    let started_at_unix_ms = current_unix_ms();
    let run_root = PathBuf::from(FSV_ROOT).join(format!("run-{started_at_unix_ms}"));
    let db_path = run_root.join("infer-rocksdb");
    let fsv_path = run_root.join("operator_contributions_fsv.json");
    let canonical_path = PathBuf::from(FSV_ROOT).join("operator_contributions_fsv.json");
    fs::create_dir_all(&run_root).context("create TASK-EK-014 FSV dir")?;

    let coverage_rows = all_kinds_all_operators_rows();
    let burn_in_rows = burn_in_rows();
    let window_rows = hundred_event_window_rows();
    let expected_window_distribution = expected_hundred_event_distribution();
    let expected_operator_counts = BTreeMap::from([
        ("operator-alpha".to_string(), 50usize),
        ("operator-beta".to_string(), 30usize),
        ("operator-gamma".to_string(), 20usize),
    ]);

    let mut all_rows = Vec::new();
    all_rows.extend(burn_in_rows.clone());
    all_rows.extend(coverage_rows.clone());
    all_rows.extend(window_rows.clone());

    let db = open_infer_rocksdb(&db_path).context("open TASK-EK-014 RocksDB")?;
    let cf_count_before = count_cf(db.as_ref(), CF_MEJEPA_OPERATOR_CONTRIBUTIONS)
        .context("count contribution CF before")?;
    for row in &all_rows {
        write_operator_contribution_sync_readback(db.as_ref(), row)
            .with_context(|| format!("persist {}", row.contribution_id))?;
    }
    let coverage_readback_identical = coverage_rows
        .iter()
        .map(|row| {
            read_operator_contribution(db.as_ref(), &row.contribution_id)
                .with_context(|| format!("read {}", row.contribution_id))
                .map(|readback| readback == Some(row.clone()))
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .all(|passed| passed);

    let hundred_report =
        context_graph_mejepa::operator_contribution_report_from_db(db.as_ref(), 100, None)
            .context("read 100-event contribution window")?;
    let hundred_distribution_ok =
        hundred_report.event_kind_distribution == expected_window_distribution;
    let hundred_operator_counts = hundred_report
        .per_operator
        .iter()
        .map(|row| (row.operator_id.clone(), row.contribution_count))
        .collect::<BTreeMap<_, _>>();
    let hundred_operator_counts_ok = hundred_operator_counts == expected_operator_counts;

    let proposal_quality_score = operator_quality_score(&hundred_report, "operator-alpha")
        .context("operator-alpha quality score")?;
    let proposal_linkage = hundred_report
        .linked_outcomes
        .iter()
        .find(|outcome| outcome.contribution_id == "window-000")
        .cloned();
    let downstream_outcome_ok = proposal_linkage
        .as_ref()
        .map(|outcome| {
            outcome.outcome_kind == "CP_Phi"
                && outcome.event_kind == "ProposalAccepted"
                && outcome.delta_value > 0.0
                && outcome.quality_score >= 0.90
                && proposal_quality_score > 0.0
        })
        .unwrap_or(false);

    let burn_in_report =
        operator_contribution_report_for_rows(burn_in_rows.clone(), burn_in_rows.len(), None)
            .context("build four-week burn-in report")?;
    let burn_in_markdown = render_operator_contributions_weekly_section(&burn_in_report)
        .context("render burn-in weekly section")?;
    let burn_in_trend_ok = burn_in_report.migration_rate_trend.len() == 4
        && burn_in_report
            .migration_rate_trend
            .iter()
            .all(|row| row.contribution_count == 10 && row.migration_rate > 0.0)
        && burn_in_markdown.contains("## Operator Contributions")
        && burn_in_markdown.contains("Migration-Rate Trend");

    let direct_readback =
        read_all_operator_contributions(db.as_ref()).context("direct contribution CF readback")?;
    let cf_count_after_write = count_cf(db.as_ref(), CF_MEJEPA_OPERATOR_CONTRIBUTIONS)
        .context("count contribution CF after write")?;
    drop(db);

    let reopened = open_infer_rocksdb(&db_path).context("reopen TASK-EK-014 RocksDB")?;
    let reopened_readback =
        read_all_operator_contributions(reopened.as_ref()).context("reopened CF readback")?;
    let cf_count_after_reopen = count_cf(reopened.as_ref(), CF_MEJEPA_OPERATOR_CONTRIBUTIONS)
        .context("count contribution CF after reopen")?;
    let reopen_readback_identical = direct_readback == reopened_readback
        && reopened_readback.len() == all_rows.len()
        && cf_count_after_reopen == all_rows.len() as u64;

    let all_passed = coverage_readback_identical
        && hundred_distribution_ok
        && hundred_operator_counts_ok
        && downstream_outcome_ok
        && burn_in_trend_ok
        && reopen_readback_identical;

    let evidence = json!({
        "task": "TASK-EK-014",
        "issue": 336,
        "passes": all_passed,
        "all_passed": all_passed,
        "sot": {
            "operator_contribution_cf": CF_MEJEPA_OPERATOR_CONTRIBUTIONS,
            "db_path": db_path,
            "fsv_path": fsv_path,
            "canonical_path": canonical_path,
            "inner_llm_invoked": false
        },
        "sot_cases": {
            "all_6_event_kinds_x_3_operators_identical_readback": {
                "passed": coverage_readback_identical,
                "event_kind_count": OperatorContributionEventKind::all().len(),
                "operator_count": 3,
                "row_count": coverage_rows.len()
            },
            "known_100_event_window_distribution": {
                "passed": hundred_distribution_ok && hundred_operator_counts_ok,
                "expected_event_distribution": expected_window_distribution,
                "actual_event_distribution": hundred_report.event_kind_distribution,
                "expected_operator_counts": expected_operator_counts,
                "actual_operator_counts": hundred_operator_counts
            },
            "downstream_outcome_linkage_and_quality_score": {
                "passed": downstream_outcome_ok,
                "linked_outcome": proposal_linkage,
                "operator_alpha_quality_score": proposal_quality_score
            },
            "four_week_burn_in_migration_rate_trend": {
                "passed": burn_in_trend_ok,
                "trend": burn_in_report.migration_rate_trend,
                "weekly_section_contains_operator_contributions": burn_in_markdown.contains("## Operator Contributions")
            },
            "independent_cf_reopen_byte_identical_readback": {
                "passed": reopen_readback_identical,
                "direct_readback_rows": direct_readback.len(),
                "reopened_readback_rows": reopened_readback.len()
            }
        },
        "readback": {
            "cf_count_before": cf_count_before,
            "cf_count_after_write": cf_count_after_write,
            "cf_count_after_reopen": cf_count_after_reopen,
            "total_rows": all_rows.len()
        },
        "surfaces": {
            "cli_tool": "context-graph-cli mejepa operator-contributions --window <N> [--operator <id>] [--format markdown|json]",
            "mcp_tool": "mcp__cgreality__mejepa_operator_contributions",
            "weekly_report_section": "## Operator Contributions",
            "source_of_truth": CF_MEJEPA_OPERATOR_CONTRIBUTIONS
        },
        "checks": {
            "coverage_readback_identical": coverage_readback_identical,
            "hundred_distribution_ok": hundred_distribution_ok,
            "hundred_operator_counts_ok": hundred_operator_counts_ok,
            "downstream_outcome_ok": downstream_outcome_ok,
            "burn_in_trend_ok": burn_in_trend_ok,
            "reopen_readback_identical": reopen_readback_identical
        }
    });
    write_json_0600(&fsv_path, &evidence).context("write TASK-EK-014 run FSV")?;
    write_json_0600(&canonical_path, &evidence).context("write TASK-EK-014 canonical FSV")?;
    let fsv_sha256 = sha256_file(&fsv_path).context("hash TASK-EK-014 FSV")?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "all_passed": all_passed,
            "fsv_path": fsv_path,
            "canonical_path": canonical_path,
            "fsv_sha256": fsv_sha256,
            "cf": CF_MEJEPA_OPERATOR_CONTRIBUTIONS,
            "total_rows": all_rows.len()
        }))?
    );
    ensure!(all_passed, "TASK-EK-014 operator contributions FSV failed");
    Ok(())
}

fn all_kinds_all_operators_rows() -> Vec<OperatorContribution> {
    let operators = ["operator-alpha", "operator-beta", "operator-gamma"];
    let mut rows = Vec::new();
    for (operator_index, operator_id) in operators.iter().enumerate() {
        for (kind_index, kind) in OperatorContributionEventKind::all().into_iter().enumerate() {
            rows.push(record(
                format!("coverage-{operator_index}-{kind_index}"),
                kind,
                operator_id,
                SYNTHETIC_BASE_TS + 4 * WEEK_MS + (operator_index * 100 + kind_index) as i64,
                None,
            ));
        }
    }
    rows
}

fn hundred_event_window_rows() -> Vec<OperatorContribution> {
    let kinds = OperatorContributionEventKind::all();
    (0..100)
        .map(|idx| {
            let operator_id = if idx < 50 {
                "operator-alpha"
            } else if idx < 80 {
                "operator-beta"
            } else {
                "operator-gamma"
            };
            let kind = kinds[idx % kinds.len()];
            let outcome = if idx == 0 {
                Some(DownstreamOutcomeRef::new(
                    DownstreamOutcomeKind::CpPhi,
                    "train-cert:cp-phi-window-000",
                    None,
                    0.037,
                    Some(0.411),
                    Some(0.448),
                    0.92,
                ))
            } else if idx % 10 == 0 {
                Some(DownstreamOutcomeRef::new(
                    DownstreamOutcomeKind::PerCellCorrelation,
                    format!("eval-window:{idx}"),
                    Some("mutation_category=bugfix,language=python".to_string()),
                    0.010,
                    Some(0.870),
                    Some(0.880),
                    0.55,
                ))
            } else {
                None
            };
            record(
                format!("window-{idx:03}"),
                kind,
                operator_id,
                SYNTHETIC_BASE_TS + 5 * WEEK_MS + idx as i64,
                outcome,
            )
        })
        .collect()
}

fn burn_in_rows() -> Vec<OperatorContribution> {
    let kinds = OperatorContributionEventKind::all();
    (0..4)
        .flat_map(|week| {
            (0..10).map(move |idx| {
                let operator_id = match (week + idx) % 3 {
                    0 => "operator-alpha",
                    1 => "operator-beta",
                    _ => "operator-gamma",
                };
                let outcome = if idx % 3 == 0 {
                    Some(DownstreamOutcomeRef::new(
                        DownstreamOutcomeKind::ShipGate,
                        format!("ship-gate:week-{week}:row-{idx}"),
                        Some(format!("week-{week}")),
                        0.004 + week as f32 * 0.001,
                        Some(0.920),
                        Some(0.924 + week as f32 * 0.001),
                        0.40 + week as f32 * 0.05,
                    ))
                } else {
                    None
                };
                record(
                    format!("burn-in-{week}-{idx}"),
                    kinds[(week * 10 + idx) % kinds.len()],
                    operator_id,
                    SYNTHETIC_BASE_TS + week as i64 * WEEK_MS + idx as i64,
                    outcome,
                )
            })
        })
        .collect()
}

fn expected_hundred_event_distribution() -> BTreeMap<String, usize> {
    OperatorContributionEventKind::all()
        .into_iter()
        .enumerate()
        .map(|(idx, kind)| {
            let expected = if idx < 4 { 17 } else { 16 };
            (kind.as_str().to_string(), expected)
        })
        .collect()
}

fn record(
    contribution_id: impl Into<String>,
    event_kind: OperatorContributionEventKind,
    operator_id: &str,
    ts_unix_ms: i64,
    downstream_outcome: Option<DownstreamOutcomeRef>,
) -> OperatorContribution {
    let contribution_id = contribution_id.into();
    let payload = OperatorContributionPayload::new(
        format!("subject:{contribution_id}"),
        format!("synthetic operator contribution {contribution_id}"),
        "fsv:task-ek-014",
        BTreeMap::from([("fixture".to_string(), "task-ek-014".to_string())]),
    );
    OperatorContribution::new(
        contribution_id,
        event_kind,
        operator_id,
        ts_unix_ms,
        payload,
        downstream_outcome,
    )
    .expect("valid synthetic operator contribution")
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    Ok(hex::encode(Sha256::digest(&bytes)))
}
