// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use serde::{Deserialize, Serialize};

use crate::categories::StorageCategory;
use crate::entry::EntryId;
use crate::tier::Tier;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EvictionCandidate {
    pub entry_id: EntryId,
    pub category: StorageCategory,
    pub size_bytes: u64,
    pub score: f32,
    pub tier: Tier,
    pub gold: bool,
    pub last_read_unix: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct QuotaStateRecord {
    pub category: StorageCategory,
    pub unrecoverable: bool,
    pub reason: String,
    pub updated_unix: i64,
}
