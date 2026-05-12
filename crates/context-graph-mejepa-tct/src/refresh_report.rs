use std::collections::BTreeSet;
use std::time::SystemTime;

use bincode::Options;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::constellation::bincode_options;
use crate::error::TctError;
use crate::shrinkage::ShrinkageOrigin;
use crate::types::{
    validate_code_version, EmbedderId, EntityType, Language, MutationCategory, OracleOutcome,
};

pub const TCT_REFRESH_REPORT_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CellSupportRecord {
    pub mutation: MutationCategory,
    pub entity_type: EntityType,
    pub language: Language,
    pub embedder: EmbedderId,
    pub observed_samples: usize,
    pub centroid_sample_count: usize,
    pub centroid_origin: ShrinkageOrigin,
}

impl CellSupportRecord {
    pub fn try_new(
        mutation: MutationCategory,
        entity_type: EntityType,
        language: Language,
        embedder: EmbedderId,
        observed_samples: usize,
        centroid_sample_count: usize,
        centroid_origin: ShrinkageOrigin,
    ) -> Result<Self, TctError> {
        if observed_samples == 0 {
            return Err(TctError::InsufficientSamples {
                cell: format!("{mutation:?}/{entity_type:?}/{language:?}/{embedder}"),
                observed: 0,
                required: 1,
            });
        }
        if centroid_sample_count == 0 {
            return Err(TctError::InsufficientSamples {
                cell: format!("centroid/{mutation:?}/{entity_type:?}/{language:?}/{embedder}"),
                observed: 0,
                required: 1,
            });
        }
        Ok(Self {
            mutation,
            entity_type,
            language,
            embedder,
            observed_samples,
            centroid_sample_count,
            centroid_origin,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EntitySupportRecord {
    pub entity_type: EntityType,
    pub chunk_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CategorySupportRecord {
    pub mutation: MutationCategory,
    pub row_count: usize,
    pub chunk_count: usize,
}

impl CategorySupportRecord {
    pub fn try_new(
        mutation: MutationCategory,
        row_count: usize,
        chunk_count: usize,
    ) -> Result<Self, TctError> {
        validate_nonzero_support(
            &format!("category_support/{mutation:?}/row_count"),
            row_count,
        )?;
        validate_nonzero_support(
            &format!("category_support/{mutation:?}/chunk_count"),
            chunk_count,
        )?;
        Ok(Self {
            mutation,
            row_count,
            chunk_count,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LanguageSupportRecord {
    pub language: Language,
    pub row_count: usize,
    pub chunk_count: usize,
}

impl LanguageSupportRecord {
    pub fn try_new(
        language: Language,
        row_count: usize,
        chunk_count: usize,
    ) -> Result<Self, TctError> {
        validate_nonzero_support(
            &format!("language_support/{language:?}/row_count"),
            row_count,
        )?;
        validate_nonzero_support(
            &format!("language_support/{language:?}/chunk_count"),
            chunk_count,
        )?;
        Ok(Self {
            language,
            row_count,
            chunk_count,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OracleOutcomeSupportRecord {
    pub outcome: OracleOutcome,
    pub row_count: usize,
}

impl OracleOutcomeSupportRecord {
    pub fn try_new(outcome: OracleOutcome, row_count: usize) -> Result<Self, TctError> {
        validate_nonzero_support(&format!("oracle_outcome_support/{outcome:?}"), row_count)?;
        Ok(Self { outcome, row_count })
    }
}

impl EntitySupportRecord {
    pub fn try_new(entity_type: EntityType, chunk_count: usize) -> Result<Self, TctError> {
        if chunk_count == 0 {
            return Err(TctError::InsufficientSamples {
                cell: format!("entity_support/{entity_type:?}"),
                observed: 0,
                required: 1,
            });
        }
        Ok(Self {
            entity_type,
            chunk_count,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RefreshShrinkageSummary {
    pub total_cells: usize,
    pub own_cell: usize,
    pub language_aggregate: usize,
    pub entity_aggregate: usize,
    pub category_aggregate: usize,
}

impl RefreshShrinkageSummary {
    pub fn validate(&self) -> Result<(), TctError> {
        if self.total_cells == 0 {
            return Err(TctError::InsufficientSamples {
                cell: "refresh_report.shrinkage.total_cells".to_string(),
                observed: 0,
                required: 1,
            });
        }
        let sum = self
            .own_cell
            .checked_add(self.language_aggregate)
            .and_then(|value| value.checked_add(self.entity_aggregate))
            .and_then(|value| value.checked_add(self.category_aggregate))
            .ok_or_else(|| {
                TctError::invalid(
                    "refresh_report.shrinkage",
                    "shrinkage summary counts overflowed usize",
                )
            })?;
        if sum != self.total_cells {
            return Err(TctError::invalid(
                "refresh_report.shrinkage",
                format!("summary counts sum to {sum}, expected {}", self.total_cells),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperatorDiagnosticSummary {
    pub inspected_chunk_count: usize,
    pub rejected_chunk_count: usize,
    pub violating_embedder_count: usize,
    pub worst_margin: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RefreshRewardSignalSummary {
    pub source_row_count: usize,
    pub source_chunk_count: usize,
    pub mutation_category_count: usize,
    pub language_count: usize,
    pub oracle_outcome_count: usize,
    pub per_chunk_type_cell_count: usize,
    pub panel_level_cell_count: usize,
    pub strict_guard_rejection_count: usize,
    pub violating_chunk_count: usize,
    pub estimated_reward_scalar_count: usize,
}

impl RefreshRewardSignalSummary {
    pub fn validate(
        &self,
        source_row_count: usize,
        source_chunk_count: usize,
        mutation_category_count: usize,
        language_count: usize,
        oracle_outcome_count: usize,
        per_chunk_type_cell_count: usize,
        operator_diagnostics: OperatorDiagnosticSummary,
    ) -> Result<(), TctError> {
        if self.source_row_count != source_row_count {
            return Err(TctError::invalid(
                "refresh_report.reward_signal_summary.source_row_count",
                format!(
                    "summary row count {} must equal source row count {source_row_count}",
                    self.source_row_count
                ),
            ));
        }
        if self.source_chunk_count != source_chunk_count {
            return Err(TctError::invalid(
                "refresh_report.reward_signal_summary.source_chunk_count",
                format!(
                    "summary chunk count {} must equal source chunk count {source_chunk_count}",
                    self.source_chunk_count
                ),
            ));
        }
        if self.mutation_category_count != mutation_category_count {
            return Err(TctError::invalid(
                "refresh_report.reward_signal_summary.mutation_category_count",
                format!(
                    "summary category count {} must equal observed count {mutation_category_count}",
                    self.mutation_category_count
                ),
            ));
        }
        if self.language_count != language_count {
            return Err(TctError::invalid(
                "refresh_report.reward_signal_summary.language_count",
                format!(
                    "summary language count {} must equal observed count {language_count}",
                    self.language_count
                ),
            ));
        }
        if self.oracle_outcome_count != oracle_outcome_count {
            return Err(TctError::invalid(
                "refresh_report.reward_signal_summary.oracle_outcome_count",
                format!(
                    "summary oracle outcome count {} must equal observed count {oracle_outcome_count}",
                    self.oracle_outcome_count
                ),
            ));
        }
        if self.per_chunk_type_cell_count != per_chunk_type_cell_count {
            return Err(TctError::invalid(
                "refresh_report.reward_signal_summary.per_chunk_type_cell_count",
                format!(
                    "summary cell count {} must equal cell support len {per_chunk_type_cell_count}",
                    self.per_chunk_type_cell_count
                ),
            ));
        }
        validate_nonzero_support(
            "refresh_report.reward_signal_summary.panel_level_cell_count",
            self.panel_level_cell_count,
        )?;
        if self.strict_guard_rejection_count != operator_diagnostics.rejected_chunk_count {
            return Err(TctError::invalid(
                "refresh_report.reward_signal_summary.strict_guard_rejection_count",
                format!(
                    "summary strict rejection count {} must equal operator rejected chunk count {}",
                    self.strict_guard_rejection_count, operator_diagnostics.rejected_chunk_count
                ),
            ));
        }
        if self.violating_chunk_count != operator_diagnostics.rejected_chunk_count {
            return Err(TctError::invalid(
                "refresh_report.reward_signal_summary.violating_chunk_count",
                format!(
                    "summary violating chunk count {} must equal operator rejected chunk count {}",
                    self.violating_chunk_count, operator_diagnostics.rejected_chunk_count
                ),
            ));
        }
        if self.estimated_reward_scalar_count < source_chunk_count {
            return Err(TctError::invalid(
                "refresh_report.reward_signal_summary.estimated_reward_scalar_count",
                format!(
                    "estimated scalar count {} must be at least source chunk count {source_chunk_count}",
                    self.estimated_reward_scalar_count
                ),
            ));
        }
        Ok(())
    }
}

impl OperatorDiagnosticSummary {
    pub fn validate(&self) -> Result<(), TctError> {
        if self.inspected_chunk_count == 0 {
            return Err(TctError::InsufficientSamples {
                cell: "refresh_report.operator_diagnostics.inspected_chunk_count".to_string(),
                observed: 0,
                required: 1,
            });
        }
        if self.rejected_chunk_count > self.inspected_chunk_count {
            return Err(TctError::invalid(
                "refresh_report.operator_diagnostics.rejected_chunk_count",
                format!(
                    "rejected {} exceeds inspected {}",
                    self.rejected_chunk_count, self.inspected_chunk_count
                ),
            ));
        }
        if self.rejected_chunk_count > 0 && self.violating_embedder_count == 0 {
            return Err(TctError::invalid(
                "refresh_report.operator_diagnostics.violating_embedder_count",
                "rejected chunks require at least one violating embedder",
            ));
        }
        if !self.worst_margin.is_finite() {
            return Err(TctError::nan(
                "refresh_report.operator_diagnostics.worst_margin",
                format!("worst_margin is {}", self.worst_margin),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstellationRefreshReportInput {
    pub started_at: SystemTime,
    pub finished_at: SystemTime,
    pub constellation_version_id: [u8; 32],
    pub corpus_sha: [u8; 32],
    pub code_version: String,
    pub source_corpus_path: String,
    pub source_corpus_sha256: [u8; 32],
    pub source_row_count: usize,
    pub source_chunk_count: usize,
    pub ingested_panel_count: usize,
    pub per_entity_support: Vec<EntitySupportRecord>,
    pub per_category_support: Vec<CategorySupportRecord>,
    pub per_language_support: Vec<LanguageSupportRecord>,
    pub per_oracle_outcome_support: Vec<OracleOutcomeSupportRecord>,
    pub cell_support: Vec<CellSupportRecord>,
    pub shrinkage: RefreshShrinkageSummary,
    pub operator_diagnostics: OperatorDiagnosticSummary,
    pub reward_signal_summary: RefreshRewardSignalSummary,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConstellationRefreshReport {
    pub schema_version: u16,
    pub report_id: [u8; 32],
    pub started_at: SystemTime,
    pub finished_at: SystemTime,
    pub constellation_version_id: [u8; 32],
    pub corpus_sha: [u8; 32],
    pub code_version: String,
    pub source_corpus_path: String,
    pub source_corpus_sha256: [u8; 32],
    pub source_row_count: usize,
    pub source_chunk_count: usize,
    pub ingested_panel_count: usize,
    pub per_entity_support: Vec<EntitySupportRecord>,
    pub per_category_support: Vec<CategorySupportRecord>,
    pub per_language_support: Vec<LanguageSupportRecord>,
    pub per_oracle_outcome_support: Vec<OracleOutcomeSupportRecord>,
    pub cell_support: Vec<CellSupportRecord>,
    pub shrinkage: RefreshShrinkageSummary,
    pub operator_diagnostics: OperatorDiagnosticSummary,
    pub reward_signal_summary: RefreshRewardSignalSummary,
}

impl ConstellationRefreshReport {
    pub fn try_new(input: ConstellationRefreshReportInput) -> Result<Self, TctError> {
        let mut report = Self {
            schema_version: TCT_REFRESH_REPORT_SCHEMA_VERSION,
            report_id: [0; 32],
            started_at: input.started_at,
            finished_at: input.finished_at,
            constellation_version_id: input.constellation_version_id,
            corpus_sha: input.corpus_sha,
            code_version: input.code_version,
            source_corpus_path: input.source_corpus_path,
            source_corpus_sha256: input.source_corpus_sha256,
            source_row_count: input.source_row_count,
            source_chunk_count: input.source_chunk_count,
            ingested_panel_count: input.ingested_panel_count,
            per_entity_support: input.per_entity_support,
            per_category_support: input.per_category_support,
            per_language_support: input.per_language_support,
            per_oracle_outcome_support: input.per_oracle_outcome_support,
            cell_support: input.cell_support,
            shrinkage: input.shrinkage,
            operator_diagnostics: input.operator_diagnostics,
            reward_signal_summary: input.reward_signal_summary,
        };
        report.validate_without_report_id()?;
        report.report_id = report.compute_report_id()?;
        report.validate_integrity()?;
        Ok(report)
    }

    pub fn validate_integrity(&self) -> Result<(), TctError> {
        self.validate_without_report_id()?;
        let observed = self.compute_report_id()?;
        if observed != self.report_id {
            return Err(TctError::FrozenViolation {
                detail: format!(
                    "refresh report_id mismatch: stored={} recomputed={}",
                    hex::encode(self.report_id),
                    hex::encode(observed)
                ),
            });
        }
        Ok(())
    }

    pub fn compute_report_id(&self) -> Result<[u8; 32], TctError> {
        let payload = RefreshReportPayload {
            schema_version: self.schema_version,
            started_at: self.started_at,
            finished_at: self.finished_at,
            constellation_version_id: self.constellation_version_id,
            corpus_sha: self.corpus_sha,
            code_version: &self.code_version,
            source_corpus_path: &self.source_corpus_path,
            source_corpus_sha256: self.source_corpus_sha256,
            source_row_count: self.source_row_count,
            source_chunk_count: self.source_chunk_count,
            ingested_panel_count: self.ingested_panel_count,
            per_entity_support: &self.per_entity_support,
            per_category_support: &self.per_category_support,
            per_language_support: &self.per_language_support,
            per_oracle_outcome_support: &self.per_oracle_outcome_support,
            cell_support: &self.cell_support,
            shrinkage: self.shrinkage,
            operator_diagnostics: self.operator_diagnostics,
            reward_signal_summary: self.reward_signal_summary,
        };
        let bytes = bincode_options().serialize(&payload)?;
        Ok(Sha256::digest(bytes).into())
    }

    fn validate_without_report_id(&self) -> Result<(), TctError> {
        if self.schema_version != TCT_REFRESH_REPORT_SCHEMA_VERSION {
            return Err(TctError::invalid(
                "refresh_report.schema_version",
                format!(
                    "unsupported schema version {}; expected {TCT_REFRESH_REPORT_SCHEMA_VERSION}",
                    self.schema_version
                ),
            ));
        }
        if self.constellation_version_id == [0; 32] {
            return Err(TctError::invalid(
                "refresh_report.constellation_version_id",
                "constellation version id must be non-zero",
            ));
        }
        if self.corpus_sha == [0; 32] {
            return Err(TctError::invalid(
                "refresh_report.corpus_sha",
                "corpus sha must be non-zero",
            ));
        }
        if self.source_corpus_sha256 == [0; 32] {
            return Err(TctError::invalid(
                "refresh_report.source_corpus_sha256",
                "source corpus sha256 must be non-zero",
            ));
        }
        validate_code_version(&self.code_version)?;
        validate_single_line(
            "refresh_report.source_corpus_path",
            &self.source_corpus_path,
        )?;
        self.finished_at
            .duration_since(self.started_at)
            .map_err(|_| {
                TctError::invalid(
                    "refresh_report.finished_at",
                    "finished_at must be greater than or equal to started_at",
                )
            })?;
        if self.source_row_count == 0 {
            return Err(TctError::InsufficientSamples {
                cell: "refresh_report.source_row_count".to_string(),
                observed: 0,
                required: 1,
            });
        }
        if self.source_chunk_count == 0 {
            return Err(TctError::InsufficientSamples {
                cell: "refresh_report.source_chunk_count".to_string(),
                observed: 0,
                required: 1,
            });
        }
        let expected_ingested = self
            .source_row_count
            .checked_add(self.source_chunk_count)
            .ok_or_else(|| {
                TctError::invalid(
                    "refresh_report.ingested_panel_count",
                    "source row and chunk counts overflowed usize",
                )
            })?;
        if self.ingested_panel_count != expected_ingested {
            return Err(TctError::invalid(
                "refresh_report.ingested_panel_count",
                format!(
                    "ingested panel count {} must equal source rows + chunks {}",
                    self.ingested_panel_count, expected_ingested
                ),
            ));
        }
        if self.per_entity_support.is_empty() {
            return Err(TctError::InsufficientSamples {
                cell: "refresh_report.per_entity_support".to_string(),
                observed: 0,
                required: 1,
            });
        }
        let mut entity_seen = BTreeSet::new();
        let entity_total = self
            .per_entity_support
            .iter()
            .try_fold(0usize, |acc, item| {
                if !entity_seen.insert(item.entity_type) {
                    return Err(TctError::invalid(
                        "refresh_report.per_entity_support",
                        format!("duplicate entity support for {:?}", item.entity_type),
                    ));
                }
                if item.chunk_count == 0 {
                    return Err(TctError::InsufficientSamples {
                        cell: format!("refresh_report.per_entity_support/{:?}", item.entity_type),
                        observed: 0,
                        required: 1,
                    });
                }
                acc.checked_add(item.chunk_count).ok_or_else(|| {
                    TctError::invalid(
                        "refresh_report.per_entity_support",
                        "entity support count overflowed usize",
                    )
                })
            })?;
        if entity_total != self.source_chunk_count {
            return Err(TctError::invalid(
                "refresh_report.per_entity_support",
                format!(
                    "entity support sums to {entity_total}, expected {}",
                    self.source_chunk_count
                ),
            ));
        }
        let (category_row_total, category_chunk_total) =
            validate_category_support(&self.per_category_support)?;
        if category_row_total != self.source_row_count {
            return Err(TctError::invalid(
                "refresh_report.per_category_support",
                format!(
                    "category row support sums to {category_row_total}, expected {}",
                    self.source_row_count
                ),
            ));
        }
        if category_chunk_total != self.source_chunk_count {
            return Err(TctError::invalid(
                "refresh_report.per_category_support",
                format!(
                    "category chunk support sums to {category_chunk_total}, expected {}",
                    self.source_chunk_count
                ),
            ));
        }
        let (language_row_total, language_chunk_total) =
            validate_language_support(&self.per_language_support)?;
        if language_row_total != self.source_row_count {
            return Err(TctError::invalid(
                "refresh_report.per_language_support",
                format!(
                    "language row support sums to {language_row_total}, expected {}",
                    self.source_row_count
                ),
            ));
        }
        if language_chunk_total != self.source_chunk_count {
            return Err(TctError::invalid(
                "refresh_report.per_language_support",
                format!(
                    "language chunk support sums to {language_chunk_total}, expected {}",
                    self.source_chunk_count
                ),
            ));
        }
        let oracle_row_total = validate_oracle_outcome_support(&self.per_oracle_outcome_support)?;
        if oracle_row_total != self.source_row_count {
            return Err(TctError::invalid(
                "refresh_report.per_oracle_outcome_support",
                format!(
                    "oracle outcome row support sums to {oracle_row_total}, expected {}",
                    self.source_row_count
                ),
            ));
        }
        if self.cell_support.is_empty() {
            return Err(TctError::InsufficientSamples {
                cell: "refresh_report.cell_support".to_string(),
                observed: 0,
                required: 1,
            });
        }
        let mut cell_seen = BTreeSet::new();
        for cell in &self.cell_support {
            if !cell_seen.insert((
                cell.mutation,
                cell.entity_type,
                cell.language,
                cell.embedder,
            )) {
                return Err(TctError::invalid(
                    "refresh_report.cell_support",
                    format!(
                        "duplicate cell support for {:?}/{:?}/{:?}/{}",
                        cell.mutation, cell.entity_type, cell.language, cell.embedder
                    ),
                ));
            }
            if cell.observed_samples == 0 || cell.centroid_sample_count == 0 {
                return Err(TctError::InsufficientSamples {
                    cell: format!(
                        "refresh_report.cell_support/{:?}/{:?}/{:?}/{}",
                        cell.mutation, cell.entity_type, cell.language, cell.embedder
                    ),
                    observed: cell.observed_samples.min(cell.centroid_sample_count),
                    required: 1,
                });
            }
        }
        self.shrinkage.validate()?;
        if self.shrinkage.total_cells != self.cell_support.len() {
            return Err(TctError::invalid(
                "refresh_report.shrinkage.total_cells",
                format!(
                    "shrinkage total {} must equal cell_support len {}",
                    self.shrinkage.total_cells,
                    self.cell_support.len()
                ),
            ));
        }
        self.operator_diagnostics.validate()?;
        self.reward_signal_summary.validate(
            self.source_row_count,
            self.source_chunk_count,
            self.per_category_support.len(),
            self.per_language_support.len(),
            self.per_oracle_outcome_support.len(),
            self.cell_support.len(),
            self.operator_diagnostics,
        )?;
        Ok(())
    }
}

#[derive(Serialize)]
struct RefreshReportPayload<'a> {
    schema_version: u16,
    started_at: SystemTime,
    finished_at: SystemTime,
    constellation_version_id: [u8; 32],
    corpus_sha: [u8; 32],
    code_version: &'a str,
    source_corpus_path: &'a str,
    source_corpus_sha256: [u8; 32],
    source_row_count: usize,
    source_chunk_count: usize,
    ingested_panel_count: usize,
    per_entity_support: &'a [EntitySupportRecord],
    per_category_support: &'a [CategorySupportRecord],
    per_language_support: &'a [LanguageSupportRecord],
    per_oracle_outcome_support: &'a [OracleOutcomeSupportRecord],
    cell_support: &'a [CellSupportRecord],
    shrinkage: RefreshShrinkageSummary,
    operator_diagnostics: OperatorDiagnosticSummary,
    reward_signal_summary: RefreshRewardSignalSummary,
}

fn validate_single_line(field: &str, value: &str) -> Result<(), TctError> {
    if value.trim().is_empty() {
        return Err(TctError::invalid(field, "value must be non-empty"));
    }
    if value.chars().any(char::is_control) {
        return Err(TctError::invalid(
            field,
            "value must not contain control characters",
        ));
    }
    Ok(())
}

fn validate_nonzero_support(field: &str, value: usize) -> Result<(), TctError> {
    if value == 0 {
        return Err(TctError::InsufficientSamples {
            cell: field.to_string(),
            observed: 0,
            required: 1,
        });
    }
    Ok(())
}

fn add_support(acc: usize, value: usize, field: &str) -> Result<usize, TctError> {
    acc.checked_add(value)
        .ok_or_else(|| TctError::invalid(field, "support count overflowed usize"))
}

fn validate_category_support(items: &[CategorySupportRecord]) -> Result<(usize, usize), TctError> {
    if items.is_empty() {
        return Err(TctError::InsufficientSamples {
            cell: "refresh_report.per_category_support".to_string(),
            observed: 0,
            required: 1,
        });
    }
    let mut seen = BTreeSet::new();
    let mut rows = 0usize;
    let mut chunks = 0usize;
    for item in items {
        if !seen.insert(item.mutation) {
            return Err(TctError::invalid(
                "refresh_report.per_category_support",
                format!("duplicate category support for {:?}", item.mutation),
            ));
        }
        validate_nonzero_support(
            &format!(
                "refresh_report.per_category_support/{:?}/row_count",
                item.mutation
            ),
            item.row_count,
        )?;
        validate_nonzero_support(
            &format!(
                "refresh_report.per_category_support/{:?}/chunk_count",
                item.mutation
            ),
            item.chunk_count,
        )?;
        rows = add_support(rows, item.row_count, "refresh_report.per_category_support")?;
        chunks = add_support(
            chunks,
            item.chunk_count,
            "refresh_report.per_category_support",
        )?;
    }
    Ok((rows, chunks))
}

fn validate_language_support(items: &[LanguageSupportRecord]) -> Result<(usize, usize), TctError> {
    if items.is_empty() {
        return Err(TctError::InsufficientSamples {
            cell: "refresh_report.per_language_support".to_string(),
            observed: 0,
            required: 1,
        });
    }
    let mut seen = BTreeSet::new();
    let mut rows = 0usize;
    let mut chunks = 0usize;
    for item in items {
        if !seen.insert(item.language) {
            return Err(TctError::invalid(
                "refresh_report.per_language_support",
                format!("duplicate language support for {:?}", item.language),
            ));
        }
        validate_nonzero_support(
            &format!(
                "refresh_report.per_language_support/{:?}/row_count",
                item.language
            ),
            item.row_count,
        )?;
        validate_nonzero_support(
            &format!(
                "refresh_report.per_language_support/{:?}/chunk_count",
                item.language
            ),
            item.chunk_count,
        )?;
        rows = add_support(rows, item.row_count, "refresh_report.per_language_support")?;
        chunks = add_support(
            chunks,
            item.chunk_count,
            "refresh_report.per_language_support",
        )?;
    }
    Ok((rows, chunks))
}

fn validate_oracle_outcome_support(
    items: &[OracleOutcomeSupportRecord],
) -> Result<usize, TctError> {
    if items.is_empty() {
        return Err(TctError::InsufficientSamples {
            cell: "refresh_report.per_oracle_outcome_support".to_string(),
            observed: 0,
            required: 1,
        });
    }
    let mut seen = BTreeSet::new();
    let mut rows = 0usize;
    for item in items {
        if !seen.insert(item.outcome) {
            return Err(TctError::invalid(
                "refresh_report.per_oracle_outcome_support",
                format!("duplicate oracle outcome support for {:?}", item.outcome),
            ));
        }
        validate_nonzero_support(
            &format!(
                "refresh_report.per_oracle_outcome_support/{:?}/row_count",
                item.outcome
            ),
            item.row_count,
        )?;
        rows = add_support(
            rows,
            item.row_count,
            "refresh_report.per_oracle_outcome_support",
        )?;
    }
    Ok(rows)
}
