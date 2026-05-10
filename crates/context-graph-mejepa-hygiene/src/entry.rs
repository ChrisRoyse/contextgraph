// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use serde::{Deserialize, Serialize};

use crate::categories::StorageCategory;
use crate::tier::Tier;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct EntryId {
    pub cf_name: String,
    pub key: Vec<u8>,
}

impl EntryId {
    pub fn new(cf_name: impl Into<String>, key: impl Into<Vec<u8>>) -> Self {
        Self {
            cf_name: cf_name.into(),
            key: key.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct EntryAccessFrequency {
    pub score: f32,
    pub last_read_unix: i64,
    pub read_count: u64,
}

impl EntryAccessFrequency {
    pub fn new(score: f32, last_read_unix: i64, read_count: u64) -> Self {
        Self {
            score,
            last_read_unix,
            read_count,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TierTransition {
    pub from: Tier,
    pub to: Tier,
    pub at_unix: i64,
    pub reason: String,
    pub before_bytes: u64,
    pub after_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HygieneEntryMeta {
    pub entry_id: EntryId,
    pub category: StorageCategory,
    pub tier: Tier,
    pub frequency: EntryAccessFrequency,
    pub size_bytes: u64,
    pub gold: bool,
    pub corrupt: bool,
    pub fidelity_lost: bool,
    pub created_unix: i64,
    pub updated_unix: i64,
    pub transition_log: Vec<TierTransition>,
}

impl HygieneEntryMeta {
    pub const MAX_TRANSITION_LOG_ENTRIES: usize = 512;

    pub fn new(
        entry_id: EntryId,
        category: StorageCategory,
        tier: Tier,
        size_bytes: u64,
        now_unix: i64,
    ) -> Self {
        Self {
            entry_id,
            category,
            tier,
            frequency: EntryAccessFrequency::new(1.0, now_unix, 0),
            size_bytes,
            gold: false,
            corrupt: false,
            fidelity_lost: false,
            created_unix: now_unix,
            updated_unix: now_unix,
            transition_log: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.entry_id.cf_name.is_empty() {
            return Err("entry_id.cf_name must be non-empty".to_string());
        }
        if self.entry_id.key.is_empty() {
            return Err("entry_id.key must be non-empty".to_string());
        }
        if StorageCategory::from_cf(&self.entry_id.cf_name) != Some(self.category) {
            return Err(format!(
                "metadata category {:?} does not own CF {}",
                self.category, self.entry_id.cf_name
            ));
        }
        if !self.frequency.score.is_finite() || !(0.0..=1.0).contains(&self.frequency.score) {
            return Err(format!(
                "frequency.score must be finite in [0,1], got {}",
                self.frequency.score
            ));
        }
        if self.updated_unix < self.created_unix {
            return Err(format!(
                "updated_unix {} is before created_unix {}",
                self.updated_unix, self.created_unix
            ));
        }
        if self.transition_log.len() > Self::MAX_TRANSITION_LOG_ENTRIES {
            return Err(format!(
                "transition_log length {} exceeds cap {}",
                self.transition_log.len(),
                Self::MAX_TRANSITION_LOG_ENTRIES
            ));
        }
        for (idx, transition) in self.transition_log.iter().enumerate() {
            if transition.reason.is_empty() {
                return Err(format!("transition_log[{idx}].reason must be non-empty"));
            }
            if transition.reason.chars().any(char::is_control) {
                return Err(format!(
                    "transition_log[{idx}].reason contains a control character"
                ));
            }
        }
        Ok(())
    }
}
