use crate::frozen_hook::hash_f32s;
use crate::materialize::TimeStep;
use crate::{InstrumentError, InstrumentResult, Panel};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const PANEL_JSON_SCHEMA_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PanelProvenance {
    pub code_version: String,
    pub embedder_versions: BTreeMap<String, String>,
    pub corpus_sha: String,
    pub frozen_at_unix_ms: i64,
    pub source_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PanelEnvelope {
    pub schema_version: u8,
    pub time_step: TimeStep,
    pub panel_hash: String,
    pub panel: Panel,
    pub provenance: PanelProvenance,
}

impl PanelEnvelope {
    pub fn try_new(
        time_step: TimeStep,
        panel: Panel,
        provenance: PanelProvenance,
    ) -> InstrumentResult<Self> {
        validate_provenance(&provenance)?;
        let panel_hash = hash_f32s(panel.data());
        Ok(Self {
            schema_version: PANEL_JSON_SCHEMA_VERSION,
            time_step,
            panel_hash,
            panel,
            provenance,
        })
    }

    pub fn validate(&self) -> InstrumentResult<()> {
        if self.schema_version != PANEL_JSON_SCHEMA_VERSION {
            return Err(InstrumentError::invalid(
                "PanelEnvelope.schema_version",
                format!(
                    "unsupported panel JSON schema version {}; expected {}",
                    self.schema_version, PANEL_JSON_SCHEMA_VERSION
                ),
                "regenerate the panel JSON with the current schema; no migration fallback exists",
            ));
        }
        validate_provenance(&self.provenance)?;
        let actual = hash_f32s(self.panel.data());
        if actual != self.panel_hash {
            return Err(InstrumentError::invalid(
                "PanelEnvelope.panel_hash",
                format!(
                    "panel hash mismatch: envelope has {}, data hashes to {}",
                    self.panel_hash, actual
                ),
                "read the source panel again and reject the corrupted JSON",
            ));
        }
        Ok(())
    }
}

pub fn validate_provenance(provenance: &PanelProvenance) -> InstrumentResult<()> {
    if provenance.code_version.trim().is_empty() {
        return Err(InstrumentError::invalid(
            "PanelProvenance.code_version",
            "code_version is empty",
            "record the git SHA or build version that produced this panel",
        ));
    }
    if provenance.embedder_versions.is_empty() {
        return Err(InstrumentError::invalid(
            "PanelProvenance.embedder_versions",
            "embedder_versions is empty",
            "record each active embedder or deterministic instrument version",
        ));
    }
    if provenance.frozen_at_unix_ms < 0 {
        return Err(InstrumentError::invalid(
            "PanelProvenance.frozen_at_unix_ms",
            format!(
                "frozen_at_unix_ms must be non-negative, got {}",
                provenance.frozen_at_unix_ms
            ),
            "write Unix epoch milliseconds at panel materialization time",
        ));
    }
    validate_sha256_hex("PanelProvenance.corpus_sha", &provenance.corpus_sha)?;
    validate_sha256_hex("PanelProvenance.source_sha256", &provenance.source_sha256)?;
    Ok(())
}

fn validate_sha256_hex(field: &'static str, value: &str) -> InstrumentResult<()> {
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(InstrumentError::invalid(
            field,
            format!("expected 64 lowercase hex chars, got {value:?}"),
            "record a real SHA-256 digest in lowercase hex",
        ));
    }
    if value.bytes().any(|b| b.is_ascii_uppercase()) {
        return Err(InstrumentError::invalid(
            field,
            format!("SHA-256 digest must be lowercase hex, got {value:?}"),
            "normalize digest bytes to lowercase hex at the writer boundary",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InstrumentSlot, PanelBuilder};

    fn provenance() -> PanelProvenance {
        PanelProvenance {
            code_version: "test-sha".into(),
            embedder_versions: [("e_oracle".into(), "deterministic-v1".into())].into(),
            corpus_sha: "a".repeat(64),
            frozen_at_unix_ms: 1,
            source_sha256: "b".repeat(64),
        }
    }

    fn panel() -> Panel {
        let mut builder = PanelBuilder::new();
        builder
            .set_slot(
                InstrumentSlot::EOracle,
                &vec![1.0; InstrumentSlot::EOracle.dim()],
            )
            .unwrap();
        builder.build().unwrap()
    }

    #[test]
    fn panel_envelope_validates_hash_and_provenance() {
        let envelope = PanelEnvelope::try_new(TimeStep::T2, panel(), provenance()).unwrap();
        envelope.validate().unwrap();
        let mut bad = envelope.clone();
        bad.panel_hash = "c".repeat(64);
        assert_eq!(
            bad.validate().unwrap_err().code(),
            "MEJEPA_INSTRUMENTS_INVALID_INPUT"
        );
    }

    #[test]
    fn panel_envelope_rejects_bad_provenance() {
        let mut bad = provenance();
        bad.corpus_sha = "ABC".into();
        assert_eq!(
            validate_provenance(&bad).unwrap_err().code(),
            "MEJEPA_INSTRUMENTS_INVALID_INPUT"
        );
    }
}
