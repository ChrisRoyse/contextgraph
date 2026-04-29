use crate::dynamicjepa::error::{DynamicJepaError, DynamicJepaResult};
use crate::dynamicjepa::ids::DomainPackId;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DjRecordHeader {
    pub record_id: Uuid,
    pub record_version: u8,
    pub domain_pack_id: DomainPackId,
    pub domain_pack_version: String,
    pub created_at_unix_ms: i64,
    pub source_run_id: Option<Uuid>,
    pub content_hash: [u8; 32],
}

impl DjRecordHeader {
    pub fn new(
        record_id: Uuid,
        record_version: u8,
        domain_pack_id: DomainPackId,
        domain_pack_version: impl Into<String>,
        created_at_unix_ms: i64,
        source_run_id: Option<Uuid>,
    ) -> Self {
        Self {
            record_id,
            record_version,
            domain_pack_id,
            domain_pack_version: domain_pack_version.into(),
            created_at_unix_ms,
            source_run_id,
            content_hash: [0; 32],
        }
    }
}

pub fn validate_semver(value: &str, field: &str) -> DynamicJepaResult<()> {
    let parts: Vec<&str> = value.split('.').collect();
    if parts.len() != 3
        || parts
            .iter()
            .any(|part| part.is_empty() || part.parse::<u64>().is_err())
    {
        return Err(DynamicJepaError::validation(
            field,
            format!("version {value:?} is not simple semver major.minor.patch"),
            "use a version like 1.0.0 and bump it on schema changes",
        ));
    }
    Ok(())
}

pub fn validate_header(
    header: &DjRecordHeader,
    expected_version: u8,
    payload_type: &'static str,
) -> DynamicJepaResult<()> {
    if header.record_id.is_nil() {
        return Err(DynamicJepaError::validation(
            format!("{payload_type}.header.record_id"),
            "record_id must not be nil",
            "generate the record UUID once at the writer boundary",
        ));
    }
    if header.record_version != expected_version {
        return Err(DynamicJepaError::codec(
            expected_version,
            header.record_version,
            payload_type,
            "wipe the pre-production DB or regenerate the record with the current schema",
        ));
    }
    header.domain_pack_id.validate()?;
    validate_semver(
        &header.domain_pack_version,
        &format!("{payload_type}.header.domain_pack_version"),
    )?;
    if header.created_at_unix_ms < 0 {
        return Err(DynamicJepaError::validation(
            format!("{payload_type}.header.created_at_unix_ms"),
            format!(
                "timestamp must be non-negative, got {}",
                header.created_at_unix_ms
            ),
            "write Unix epoch milliseconds",
        ));
    }
    if header.content_hash == [0; 32] {
        return Err(DynamicJepaError::validation(
            format!("{payload_type}.header.content_hash"),
            "content_hash must be computed before persistence",
            "call refresh_content_hash before encoding the record",
        ));
    }
    Ok(())
}
