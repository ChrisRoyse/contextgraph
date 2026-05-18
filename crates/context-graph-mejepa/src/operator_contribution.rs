//! TASK-EK-014 — operator-contribution surface.
//!
//! Operator contributions are durable records of human work that has moved
//! into the ME-JEPA loop: accepted/rejected proposals, falsifications, probes,
//! critiques, and downstream outcome links.

use std::collections::{BTreeMap, BTreeSet};

use context_graph_mejepa_cf::CF_MEJEPA_OPERATOR_CONTRIBUTIONS;
use rocksdb::{IteratorMode, WriteOptions, DB};
use serde::{Deserialize, Serialize};
use thiserror::Error;

const SCHEMA_VERSION: u32 = 1;
const MAX_ID_BYTES: usize = 256;
const MAX_SUMMARY_BYTES: usize = 4096;
const MAX_METADATA_ENTRIES: usize = 64;
const MAX_METADATA_KEY_BYTES: usize = 128;
const MAX_METADATA_VALUE_BYTES: usize = 1024;
const WEEK_MS: i64 = 7 * 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum OperatorContributionEventKind {
    ProposalAccepted,
    ProposalRejected,
    FalsificationDiscovered,
    CandidateInstrumentAccepted,
    MechanismConjectureCritiqued,
    AdversarialProbeAuthored,
}

impl OperatorContributionEventKind {
    pub const fn all() -> [Self; 6] {
        [
            Self::ProposalAccepted,
            Self::ProposalRejected,
            Self::FalsificationDiscovered,
            Self::CandidateInstrumentAccepted,
            Self::MechanismConjectureCritiqued,
            Self::AdversarialProbeAuthored,
        ]
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ProposalAccepted => "ProposalAccepted",
            Self::ProposalRejected => "ProposalRejected",
            Self::FalsificationDiscovered => "FalsificationDiscovered",
            Self::CandidateInstrumentAccepted => "CandidateInstrumentAccepted",
            Self::MechanismConjectureCritiqued => "MechanismConjectureCritiqued",
            Self::AdversarialProbeAuthored => "AdversarialProbeAuthored",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownstreamOutcomeKind {
    #[serde(rename = "CP_Phi")]
    CpPhi,
    ShipGate,
    PerCellCorrelation,
}

impl DownstreamOutcomeKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CpPhi => "CP_Phi",
            Self::ShipGate => "ship_gate",
            Self::PerCellCorrelation => "per_cell_correlation",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DownstreamOutcomeRef {
    pub outcome_kind: DownstreamOutcomeKind,
    pub source_ref: String,
    pub metric_cell: Option<String>,
    pub delta_value: f32,
    pub baseline_value: Option<f32>,
    pub after_value: Option<f32>,
    pub quality_score: f32,
}

impl DownstreamOutcomeRef {
    pub fn new(
        outcome_kind: DownstreamOutcomeKind,
        source_ref: impl Into<String>,
        metric_cell: Option<String>,
        delta_value: f32,
        baseline_value: Option<f32>,
        after_value: Option<f32>,
        quality_score: f32,
    ) -> Self {
        Self {
            outcome_kind,
            source_ref: source_ref.into(),
            metric_cell,
            delta_value,
            baseline_value,
            after_value,
            quality_score,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OperatorContributionPayload {
    pub subject_id: String,
    pub summary: String,
    pub source_ref: String,
    pub metadata: BTreeMap<String, String>,
}

impl OperatorContributionPayload {
    pub fn new(
        subject_id: impl Into<String>,
        summary: impl Into<String>,
        source_ref: impl Into<String>,
        metadata: BTreeMap<String, String>,
    ) -> Self {
        Self {
            subject_id: subject_id.into(),
            summary: summary.into(),
            source_ref: source_ref.into(),
            metadata,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OperatorContribution {
    pub schema_version: u32,
    pub contribution_id: String,
    pub event_kind: OperatorContributionEventKind,
    pub operator_id: String,
    pub ts_unix_ms: i64,
    pub contribution_payload: OperatorContributionPayload,
    pub downstream_outcome: Option<DownstreamOutcomeRef>,
}

impl OperatorContribution {
    pub fn new(
        contribution_id: impl Into<String>,
        event_kind: OperatorContributionEventKind,
        operator_id: impl Into<String>,
        ts_unix_ms: i64,
        contribution_payload: OperatorContributionPayload,
        downstream_outcome: Option<DownstreamOutcomeRef>,
    ) -> Result<Self, OperatorContributionError> {
        let record = Self {
            schema_version: SCHEMA_VERSION,
            contribution_id: contribution_id.into(),
            event_kind,
            operator_id: operator_id.into(),
            ts_unix_ms,
            contribution_payload,
            downstream_outcome,
        };
        validate_operator_contribution(&record)?;
        Ok(record)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OperatorContributionOperatorRow {
    pub operator_id: String,
    pub contribution_count: usize,
    pub downstream_linked_count: usize,
    pub event_kind_counts: BTreeMap<String, usize>,
    pub mean_downstream_quality: f32,
    pub quality_score: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OperatorContributionQualityRow {
    pub rank: usize,
    pub operator_id: String,
    pub contribution_count: usize,
    pub downstream_linked_count: usize,
    pub linked_rate: f32,
    pub event_diversity: f32,
    pub mean_downstream_quality: f32,
    pub quality_score: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OperatorContributionTrendRow {
    pub week_start_unix_ms: i64,
    pub contribution_count: usize,
    pub downstream_linked_count: usize,
    pub migration_rate: f32,
    pub mean_downstream_quality: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OperatorContributionOutcomeSummary {
    pub contribution_id: String,
    pub operator_id: String,
    pub event_kind: String,
    pub outcome_kind: String,
    pub source_ref: String,
    pub metric_cell: Option<String>,
    pub delta_value: f32,
    pub quality_score: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct OperatorContributionReport {
    pub window: usize,
    pub operator_filter: Option<String>,
    pub total_persisted_count: usize,
    pub returned_count: usize,
    pub event_kind_distribution: BTreeMap<String, usize>,
    pub per_operator: Vec<OperatorContributionOperatorRow>,
    pub quality_ranking: Vec<OperatorContributionQualityRow>,
    pub migration_rate_trend: Vec<OperatorContributionTrendRow>,
    pub linked_outcomes: Vec<OperatorContributionOutcomeSummary>,
    pub contributions: Vec<OperatorContribution>,
    pub source_of_truth_cf: String,
}

#[derive(Debug, Error)]
pub enum OperatorContributionError {
    #[error("MEJEPA_OPERATOR_CONTRIBUTION_SCHEMA_VERSION_INVALID: expected schema version 1")]
    SchemaVersionInvalid,
    #[error("MEJEPA_OPERATOR_CONTRIBUTION_ID_INVALID: contribution_id must be single-line text up to 256 bytes")]
    ContributionIdInvalid,
    #[error("MEJEPA_OPERATOR_CONTRIBUTION_OPERATOR_INVALID: operator_id must be single-line text up to 256 bytes")]
    OperatorInvalid,
    #[error("MEJEPA_OPERATOR_CONTRIBUTION_TS_INVALID: ts_unix_ms must be positive")]
    TimestampInvalid,
    #[error("MEJEPA_OPERATOR_CONTRIBUTION_PAYLOAD_INVALID: {0}")]
    PayloadInvalid(String),
    #[error("MEJEPA_OPERATOR_CONTRIBUTION_OUTCOME_INVALID: {0}")]
    OutcomeInvalid(String),
    #[error("MEJEPA_OPERATOR_CONTRIBUTION_WINDOW_INVALID: window must be greater than zero")]
    WindowInvalid,
    #[error(
        "MEJEPA_OPERATOR_CONTRIBUTIONS_CF_MISSING: CF_MEJEPA_OPERATOR_CONTRIBUTIONS not present"
    )]
    CfMissing,
    #[error("MEJEPA_OPERATOR_CONTRIBUTION_SERIALIZE: {0}")]
    Serialize(String),
    #[error("MEJEPA_OPERATOR_CONTRIBUTION_WRITE: {0}")]
    Write(String),
    #[error("MEJEPA_OPERATOR_CONTRIBUTION_READBACK_MISSING: row absent after write")]
    ReadbackMissing,
    #[error("MEJEPA_OPERATOR_CONTRIBUTION_READBACK_MISMATCH: row contents differ from input")]
    ReadbackMismatch,
}

pub fn write_operator_contribution_sync_readback(
    db: &DB,
    record: &OperatorContribution,
) -> Result<(), OperatorContributionError> {
    validate_operator_contribution(record)?;
    let cf = db
        .cf_handle(CF_MEJEPA_OPERATOR_CONTRIBUTIONS)
        .ok_or(OperatorContributionError::CfMissing)?;
    let bytes = bincode::serialize(record)
        .map_err(|err| OperatorContributionError::Serialize(err.to_string()))?;
    let mut write_opts = WriteOptions::default();
    write_opts.set_sync(true);
    db.put_cf_opt(cf, record.contribution_id.as_bytes(), &bytes, &write_opts)
        .map_err(|err| OperatorContributionError::Write(err.to_string()))?;
    db.flush_cf(cf)
        .map_err(|err| OperatorContributionError::Write(err.to_string()))?;
    let readback = db
        .get_cf(cf, record.contribution_id.as_bytes())
        .map_err(|err| OperatorContributionError::Write(err.to_string()))?
        .ok_or(OperatorContributionError::ReadbackMissing)?;
    let decoded: OperatorContribution = bincode::deserialize(&readback)
        .map_err(|err| OperatorContributionError::Serialize(err.to_string()))?;
    if decoded != *record {
        return Err(OperatorContributionError::ReadbackMismatch);
    }
    Ok(())
}

pub fn read_operator_contribution(
    db: &DB,
    contribution_id: &str,
) -> Result<Option<OperatorContribution>, OperatorContributionError> {
    validate_text("contribution_id", contribution_id, MAX_ID_BYTES)
        .map_err(|_| OperatorContributionError::ContributionIdInvalid)?;
    let cf = db
        .cf_handle(CF_MEJEPA_OPERATOR_CONTRIBUTIONS)
        .ok_or(OperatorContributionError::CfMissing)?;
    let raw = db
        .get_cf(cf, contribution_id.as_bytes())
        .map_err(|err| OperatorContributionError::Write(err.to_string()))?;
    let decoded = raw
        .map(|bytes| bincode::deserialize(&bytes))
        .transpose()
        .map_err(|err| OperatorContributionError::Serialize(err.to_string()))?;
    if let Some(row) = &decoded {
        validate_operator_contribution(row)?;
    }
    Ok(decoded)
}

pub fn read_all_operator_contributions(
    db: &DB,
) -> Result<Vec<OperatorContribution>, OperatorContributionError> {
    let cf = db
        .cf_handle(CF_MEJEPA_OPERATOR_CONTRIBUTIONS)
        .ok_or(OperatorContributionError::CfMissing)?;
    let mut rows = Vec::new();
    for item in db.iterator_cf(cf, IteratorMode::Start) {
        let (_key, value) =
            item.map_err(|err| OperatorContributionError::Write(err.to_string()))?;
        let row: OperatorContribution = bincode::deserialize(&value)
            .map_err(|err| OperatorContributionError::Serialize(err.to_string()))?;
        validate_operator_contribution(&row)?;
        rows.push(row);
    }
    Ok(rows)
}

pub fn operator_contribution_report_from_db(
    db: &DB,
    window: usize,
    operator_filter: Option<&str>,
) -> Result<OperatorContributionReport, OperatorContributionError> {
    let rows = read_all_operator_contributions(db)?;
    operator_contribution_report_for_rows(rows, window, operator_filter)
}

pub fn operator_contribution_report_for_rows(
    rows: Vec<OperatorContribution>,
    window: usize,
    operator_filter: Option<&str>,
) -> Result<OperatorContributionReport, OperatorContributionError> {
    if window == 0 {
        return Err(OperatorContributionError::WindowInvalid);
    }
    if let Some(operator_id) = operator_filter {
        validate_text("operator_id", operator_id, MAX_ID_BYTES)
            .map_err(|_| OperatorContributionError::OperatorInvalid)?;
    }
    for row in &rows {
        validate_operator_contribution(row)?;
    }

    let total_persisted_count = rows.len();
    let mut selected = rows
        .into_iter()
        .filter(|row| {
            operator_filter
                .map(|operator_id| row.operator_id == operator_id)
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();
    selected.sort_by(|left, right| {
        right
            .ts_unix_ms
            .cmp(&left.ts_unix_ms)
            .then_with(|| left.contribution_id.cmp(&right.contribution_id))
    });
    selected.truncate(window);

    let event_kind_distribution = event_kind_distribution(&selected);
    let per_operator = per_operator_rows(&selected);
    let quality_ranking = quality_ranking(&per_operator);
    let migration_rate_trend = migration_rate_trend(&selected);
    let linked_outcomes = linked_outcomes(&selected);

    Ok(OperatorContributionReport {
        window,
        operator_filter: operator_filter.map(str::to_string),
        total_persisted_count,
        returned_count: selected.len(),
        event_kind_distribution,
        per_operator,
        quality_ranking,
        migration_rate_trend,
        linked_outcomes,
        contributions: selected,
        source_of_truth_cf: CF_MEJEPA_OPERATOR_CONTRIBUTIONS.to_string(),
    })
}

pub fn render_operator_contributions_weekly_section(
    report: &OperatorContributionReport,
) -> Result<String, OperatorContributionError> {
    if report.window == 0 {
        return Err(OperatorContributionError::WindowInvalid);
    }
    let mut section = String::from(
        "## Operator Contributions\n\n| operator_id | contributions | downstream_linked | mean_downstream_quality | quality_score |\n| --- | ---: | ---: | ---: | ---: |\n",
    );
    if report.per_operator.is_empty() {
        section.push_str("| none | 0 | 0 | 0.000000 | 0.000000 |\n");
    } else {
        for row in &report.per_operator {
            section.push_str(&format!(
                "| {} | {} | {} | {:.6} | {:.6} |\n",
                escape_markdown_cell(&row.operator_id),
                row.contribution_count,
                row.downstream_linked_count,
                row.mean_downstream_quality,
                row.quality_score
            ));
        }
    }

    section.push_str(
        "\n### Contribution Quality Ranking\n\n| rank | operator_id | linked_rate | event_diversity | quality_score |\n| ---: | --- | ---: | ---: | ---: |\n",
    );
    if report.quality_ranking.is_empty() {
        section.push_str("| 0 | none | 0.000000 | 0.000000 | 0.000000 |\n");
    } else {
        for row in &report.quality_ranking {
            section.push_str(&format!(
                "| {} | {} | {:.6} | {:.6} | {:.6} |\n",
                row.rank,
                escape_markdown_cell(&row.operator_id),
                row.linked_rate,
                row.event_diversity,
                row.quality_score
            ));
        }
    }

    section.push_str(
        "\n### Migration-Rate Trend\n\n| week_start_unix_ms | contributions | downstream_linked | migration_rate | mean_downstream_quality |\n| ---: | ---: | ---: | ---: | ---: |\n",
    );
    if report.migration_rate_trend.is_empty() {
        section.push_str("| 0 | 0 | 0 | 0.000000 | 0.000000 |\n");
    } else {
        for row in &report.migration_rate_trend {
            section.push_str(&format!(
                "| {} | {} | {} | {:.6} | {:.6} |\n",
                row.week_start_unix_ms,
                row.contribution_count,
                row.downstream_linked_count,
                row.migration_rate,
                row.mean_downstream_quality
            ));
        }
    }

    section.push_str(&format!(
        "\n- returned_count: {}\n- total_persisted_count: {}\n- source_of_truth: {}\n",
        report.returned_count, report.total_persisted_count, report.source_of_truth_cf
    ));
    Ok(section)
}

pub fn operator_quality_score(
    report: &OperatorContributionReport,
    operator_id: &str,
) -> Option<f32> {
    report
        .quality_ranking
        .iter()
        .find(|row| row.operator_id == operator_id)
        .map(|row| row.quality_score)
}

fn validate_operator_contribution(
    record: &OperatorContribution,
) -> Result<(), OperatorContributionError> {
    if record.schema_version != SCHEMA_VERSION {
        return Err(OperatorContributionError::SchemaVersionInvalid);
    }
    validate_text("contribution_id", &record.contribution_id, MAX_ID_BYTES)
        .map_err(|_| OperatorContributionError::ContributionIdInvalid)?;
    validate_text("operator_id", &record.operator_id, MAX_ID_BYTES)
        .map_err(|_| OperatorContributionError::OperatorInvalid)?;
    if record.ts_unix_ms <= 0 {
        return Err(OperatorContributionError::TimestampInvalid);
    }
    validate_payload(&record.contribution_payload)?;
    if let Some(outcome) = &record.downstream_outcome {
        validate_outcome(outcome)?;
    }
    Ok(())
}

fn validate_payload(
    payload: &OperatorContributionPayload,
) -> Result<(), OperatorContributionError> {
    validate_text("payload.subject_id", &payload.subject_id, MAX_ID_BYTES)
        .map_err(OperatorContributionError::PayloadInvalid)?;
    validate_text("payload.summary", &payload.summary, MAX_SUMMARY_BYTES)
        .map_err(OperatorContributionError::PayloadInvalid)?;
    validate_text("payload.source_ref", &payload.source_ref, MAX_ID_BYTES)
        .map_err(OperatorContributionError::PayloadInvalid)?;
    if payload.metadata.len() > MAX_METADATA_ENTRIES {
        return Err(OperatorContributionError::PayloadInvalid(format!(
            "metadata has {} entries; max is {}",
            payload.metadata.len(),
            MAX_METADATA_ENTRIES
        )));
    }
    for (key, value) in &payload.metadata {
        validate_text("payload.metadata.key", key, MAX_METADATA_KEY_BYTES)
            .map_err(OperatorContributionError::PayloadInvalid)?;
        validate_text("payload.metadata.value", value, MAX_METADATA_VALUE_BYTES)
            .map_err(OperatorContributionError::PayloadInvalid)?;
    }
    Ok(())
}

fn validate_outcome(outcome: &DownstreamOutcomeRef) -> Result<(), OperatorContributionError> {
    validate_text(
        "downstream_outcome.source_ref",
        &outcome.source_ref,
        MAX_ID_BYTES,
    )
    .map_err(OperatorContributionError::OutcomeInvalid)?;
    if let Some(metric_cell) = &outcome.metric_cell {
        validate_text("downstream_outcome.metric_cell", metric_cell, MAX_ID_BYTES)
            .map_err(OperatorContributionError::OutcomeInvalid)?;
    }
    validate_finite("downstream_outcome.delta_value", outcome.delta_value)
        .map_err(OperatorContributionError::OutcomeInvalid)?;
    if let Some(value) = outcome.baseline_value {
        validate_finite("downstream_outcome.baseline_value", value)
            .map_err(OperatorContributionError::OutcomeInvalid)?;
    }
    if let Some(value) = outcome.after_value {
        validate_finite("downstream_outcome.after_value", value)
            .map_err(OperatorContributionError::OutcomeInvalid)?;
    }
    if !outcome.quality_score.is_finite() || !(0.0..=1.0).contains(&outcome.quality_score) {
        return Err(OperatorContributionError::OutcomeInvalid(
            "downstream_outcome.quality_score must be finite and in [0,1]".to_string(),
        ));
    }
    Ok(())
}

fn validate_text(field: &str, value: &str, max_bytes: usize) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{field} must be non-empty"));
    }
    if value.len() > max_bytes {
        return Err(format!("{field} exceeds {max_bytes} bytes"));
    }
    if value.chars().any(char::is_control) {
        return Err(format!("{field} must be single-line text"));
    }
    Ok(())
}

fn validate_finite(field: &str, value: f32) -> Result<(), String> {
    if !value.is_finite() {
        return Err(format!("{field} must be finite"));
    }
    Ok(())
}

fn event_kind_distribution(rows: &[OperatorContribution]) -> BTreeMap<String, usize> {
    let mut distribution = OperatorContributionEventKind::all()
        .into_iter()
        .map(|kind| (kind.as_str().to_string(), 0usize))
        .collect::<BTreeMap<_, _>>();
    for row in rows {
        *distribution
            .entry(row.event_kind.as_str().to_string())
            .or_insert(0) += 1;
    }
    distribution
}

#[derive(Default)]
struct OperatorAggregate {
    count: usize,
    linked_count: usize,
    quality_sum: f32,
    event_kinds: BTreeSet<OperatorContributionEventKind>,
    event_kind_counts: BTreeMap<String, usize>,
}

fn per_operator_rows(rows: &[OperatorContribution]) -> Vec<OperatorContributionOperatorRow> {
    let mut aggregates: BTreeMap<String, OperatorAggregate> = BTreeMap::new();
    for row in rows {
        let aggregate = aggregates.entry(row.operator_id.clone()).or_default();
        aggregate.count += 1;
        aggregate.event_kinds.insert(row.event_kind);
        *aggregate
            .event_kind_counts
            .entry(row.event_kind.as_str().to_string())
            .or_insert(0) += 1;
        if let Some(outcome) = &row.downstream_outcome {
            aggregate.linked_count += 1;
            aggregate.quality_sum += outcome.quality_score;
        }
    }

    aggregates
        .into_iter()
        .map(|(operator_id, aggregate)| {
            let mean_downstream_quality = if aggregate.linked_count == 0 {
                0.0
            } else {
                aggregate.quality_sum / aggregate.linked_count as f32
            };
            let linked_rate = if aggregate.count == 0 {
                0.0
            } else {
                aggregate.linked_count as f32 / aggregate.count as f32
            };
            let event_diversity = aggregate.event_kinds.len() as f32
                / OperatorContributionEventKind::all().len() as f32;
            let quality_score =
                contribution_quality_score(mean_downstream_quality, linked_rate, event_diversity);
            OperatorContributionOperatorRow {
                operator_id,
                contribution_count: aggregate.count,
                downstream_linked_count: aggregate.linked_count,
                event_kind_counts: aggregate.event_kind_counts,
                mean_downstream_quality,
                quality_score,
            }
        })
        .collect()
}

fn quality_ranking(
    rows: &[OperatorContributionOperatorRow],
) -> Vec<OperatorContributionQualityRow> {
    let mut ranking = rows
        .iter()
        .map(|row| {
            let linked_rate = if row.contribution_count == 0 {
                0.0
            } else {
                row.downstream_linked_count as f32 / row.contribution_count as f32
            };
            let event_diversity = row.event_kind_counts.len() as f32
                / OperatorContributionEventKind::all().len() as f32;
            OperatorContributionQualityRow {
                rank: 0,
                operator_id: row.operator_id.clone(),
                contribution_count: row.contribution_count,
                downstream_linked_count: row.downstream_linked_count,
                linked_rate,
                event_diversity,
                mean_downstream_quality: row.mean_downstream_quality,
                quality_score: row.quality_score,
            }
        })
        .collect::<Vec<_>>();
    ranking.sort_by(|left, right| {
        right
            .quality_score
            .total_cmp(&left.quality_score)
            .then_with(|| left.operator_id.cmp(&right.operator_id))
    });
    for (idx, row) in ranking.iter_mut().enumerate() {
        row.rank = idx + 1;
    }
    ranking
}

fn contribution_quality_score(
    mean_downstream_quality: f32,
    linked_rate: f32,
    event_diversity: f32,
) -> f32 {
    (0.70 * mean_downstream_quality + 0.20 * linked_rate + 0.10 * event_diversity).clamp(0.0, 1.0)
}

fn migration_rate_trend(rows: &[OperatorContribution]) -> Vec<OperatorContributionTrendRow> {
    let mut buckets: BTreeMap<i64, (usize, usize, f32)> = BTreeMap::new();
    for row in rows {
        let week_start = (row.ts_unix_ms / WEEK_MS) * WEEK_MS;
        let entry = buckets.entry(week_start).or_insert((0, 0, 0.0));
        entry.0 += 1;
        if let Some(outcome) = &row.downstream_outcome {
            entry.1 += 1;
            entry.2 += outcome.quality_score;
        }
    }
    let denominator = rows.len().max(1) as f32;
    buckets
        .into_iter()
        .map(
            |(week_start_unix_ms, (contribution_count, downstream_linked_count, quality_sum))| {
                OperatorContributionTrendRow {
                    week_start_unix_ms,
                    contribution_count,
                    downstream_linked_count,
                    migration_rate: contribution_count as f32 / denominator,
                    mean_downstream_quality: if downstream_linked_count == 0 {
                        0.0
                    } else {
                        quality_sum / downstream_linked_count as f32
                    },
                }
            },
        )
        .collect()
}

fn linked_outcomes(rows: &[OperatorContribution]) -> Vec<OperatorContributionOutcomeSummary> {
    let mut outcomes = rows
        .iter()
        .filter_map(|row| {
            row.downstream_outcome
                .as_ref()
                .map(|outcome| OperatorContributionOutcomeSummary {
                    contribution_id: row.contribution_id.clone(),
                    operator_id: row.operator_id.clone(),
                    event_kind: row.event_kind.as_str().to_string(),
                    outcome_kind: outcome.outcome_kind.as_str().to_string(),
                    source_ref: outcome.source_ref.clone(),
                    metric_cell: outcome.metric_cell.clone(),
                    delta_value: outcome.delta_value,
                    quality_score: outcome.quality_score,
                })
        })
        .collect::<Vec<_>>();
    outcomes.sort_by(|left, right| {
        right
            .quality_score
            .total_cmp(&left.quality_score)
            .then_with(|| left.contribution_id.cmp(&right.contribution_id))
    });
    outcomes
}

fn escape_markdown_cell(value: &str) -> String {
    value.replace('\n', " ").replace('|', "\\|")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn payload(subject: &str) -> OperatorContributionPayload {
        OperatorContributionPayload::new(subject, "summary", "test:source", BTreeMap::new())
    }

    #[test]
    fn invalid_operator_contribution_rejected() {
        let err = OperatorContribution::new(
            "",
            OperatorContributionEventKind::ProposalAccepted,
            "operator-a",
            1_700_000_000_000,
            payload("proposal"),
            None,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            OperatorContributionError::ContributionIdInvalid
        ));
    }

    #[test]
    fn report_scores_and_distributions_are_deterministic() {
        let rows = vec![
            OperatorContribution::new(
                "c1",
                OperatorContributionEventKind::ProposalAccepted,
                "operator-a",
                1_700_000_000_000,
                payload("proposal"),
                Some(DownstreamOutcomeRef::new(
                    DownstreamOutcomeKind::CpPhi,
                    "train-cert:1",
                    None,
                    0.03,
                    Some(0.20),
                    Some(0.23),
                    0.80,
                )),
            )
            .unwrap(),
            OperatorContribution::new(
                "c2",
                OperatorContributionEventKind::ProposalRejected,
                "operator-b",
                1_700_000_001_000,
                payload("proposal"),
                None,
            )
            .unwrap(),
            OperatorContribution::new(
                "c3",
                OperatorContributionEventKind::AdversarialProbeAuthored,
                "operator-a",
                1_700_000_002_000,
                payload("probe"),
                None,
            )
            .unwrap(),
        ];
        let report = operator_contribution_report_for_rows(rows, 10, None).unwrap();
        assert_eq!(report.returned_count, 3);
        assert_eq!(
            report.event_kind_distribution["ProposalAccepted"], 1,
            "distribution must include exact event counts"
        );
        assert_eq!(report.per_operator.len(), 2);
        assert_eq!(report.quality_ranking[0].operator_id, "operator-a");
        assert!(report.quality_ranking[0].quality_score > 0.0);
    }
}
