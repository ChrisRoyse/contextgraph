use crate::dynamicjepa::error::{DynamicJepaError, DynamicJepaResult};
use crate::dynamicjepa::ids::{AdapterId, DomainPackId, EventId};
use crate::dynamicjepa::record_header::DjRecordHeader;
use crate::dynamicjepa::validation::Validate;
use serde::{Deserialize, Serialize};

pub const RAW_DOMAIN_EVENT_RECORD_VERSION: u8 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawDomainEvent {
    pub header: DjRecordHeader,
    pub event_id: EventId,
    pub domain_pack_id: DomainPackId,
    pub adapter_id: AdapterId,
    pub source_kind: SourceKind,
    pub source_uri: String,
    pub source_offset: u64,
    pub payload_format: PayloadFormat,
    pub payload_bytes: Vec<u8>,
    pub payload_hash: [u8; 32],
    pub received_at_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceKind {
    JsonlFixture,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PayloadFormat {
    Json,
}

impl Validate for RawDomainEvent {
    fn validate(&self) -> DynamicJepaResult<()> {
        self.event_id.validate()?;
        self.domain_pack_id.validate()?;
        self.adapter_id.validate()?;
        if self.source_uri.trim().is_empty() {
            return Err(DynamicJepaError::validation(
                "RawDomainEvent.source_uri",
                "source_uri must not be empty",
                "store the fixture path or source URI for operator inspection",
            ));
        }
        if self.payload_bytes.is_empty() {
            return Err(DynamicJepaError::validation(
                "RawDomainEvent.payload_bytes",
                "raw event payload must not be empty",
                "empty input files fail before writing raw event rows",
            ));
        }
        if self.payload_hash == [0; 32] {
            return Err(DynamicJepaError::validation(
                "RawDomainEvent.payload_hash",
                "payload_hash must be SHA-256 of payload_bytes",
                "compute the raw payload hash before persistence",
            ));
        }
        if self.received_at_unix_ms < 0 {
            return Err(DynamicJepaError::validation(
                "RawDomainEvent.received_at_unix_ms",
                "timestamp must be non-negative",
                "write Unix epoch milliseconds",
            ));
        }
        Ok(())
    }
}

crate::impl_dynamic_jepa_record!(
    RawDomainEvent,
    RAW_DOMAIN_EVENT_RECORD_VERSION,
    "RawDomainEvent"
);
