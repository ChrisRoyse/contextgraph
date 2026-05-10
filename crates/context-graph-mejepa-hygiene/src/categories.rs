// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use serde::{Deserialize, Serialize};

pub const BYTES_PER_GB: u64 = 1024 * 1024 * 1024;
pub const DEFAULT_TOTAL_QUOTA_BYTES: u64 = 115 * BYTES_PER_GB;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageCategory {
    MutationCorpus,
    LiveAttemptCorpus,
    ModelCheckpoints,
    WitnessChains,
    LivePanelCacheRam,
    LivePanelDiskOverflow,
    CalibrationSnapshots,
    AuxSignalsOverrides,
    ShiftLogSubscriberState,
}

impl StorageCategory {
    pub fn all() -> [Self; 9] {
        [
            Self::MutationCorpus,
            Self::LiveAttemptCorpus,
            Self::ModelCheckpoints,
            Self::WitnessChains,
            Self::LivePanelCacheRam,
            Self::LivePanelDiskOverflow,
            Self::CalibrationSnapshots,
            Self::AuxSignalsOverrides,
            Self::ShiftLogSubscriberState,
        ]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::MutationCorpus => "mutation_corpus",
            Self::LiveAttemptCorpus => "live_attempt_corpus",
            Self::ModelCheckpoints => "model_checkpoints",
            Self::WitnessChains => "witness_chains",
            Self::LivePanelCacheRam => "live_panel_cache_ram",
            Self::LivePanelDiskOverflow => "live_panel_disk_overflow",
            Self::CalibrationSnapshots => "calibration_snapshots",
            Self::AuxSignalsOverrides => "aux_signals_overrides",
            Self::ShiftLogSubscriberState => "shift_log_subscriber_state",
        }
    }

    pub fn budget_bytes_default(self) -> u64 {
        match self {
            Self::MutationCorpus => 5 * BYTES_PER_GB,
            Self::LiveAttemptCorpus => 50 * BYTES_PER_GB,
            Self::ModelCheckpoints => 10 * BYTES_PER_GB,
            Self::WitnessChains => 20 * BYTES_PER_GB,
            Self::LivePanelCacheRam => 4 * BYTES_PER_GB,
            Self::LivePanelDiskOverflow => 20 * BYTES_PER_GB,
            Self::CalibrationSnapshots => 5 * BYTES_PER_GB,
            Self::AuxSignalsOverrides => 2 * BYTES_PER_GB,
            Self::ShiftLogSubscriberState => BYTES_PER_GB / 2,
        }
    }

    pub fn budget_bytes(self, total_quota_bytes: u64) -> u64 {
        let numerator = self.budget_bytes_default() as u128 * total_quota_bytes as u128;
        (numerator / DEFAULT_TOTAL_QUOTA_BYTES as u128).max(1) as u64
    }

    pub fn cf_names(self) -> &'static [&'static str] {
        use context_graph_mejepa_cf::*;
        match self {
            Self::MutationCorpus => &[CF_MEJEPA_MUTATION_CORPUS],
            Self::LiveAttemptCorpus => &[CF_MEJEPA_LIVE_ATTEMPT_CORPUS],
            Self::ModelCheckpoints => &[CF_MEJEPA_WEIGHT_BLOBS],
            Self::WitnessChains => &[CF_MEJEPA_WITNESS_CHAIN, CF_MEJEPA_WITNESS_SEGMENT_META],
            Self::LivePanelCacheRam => &[CF_MEJEPA_PANEL_CACHE],
            Self::LivePanelDiskOverflow => &[CF_MEJEPA_PANEL_DISK_OVERFLOW],
            Self::CalibrationSnapshots => &[CF_MEJEPA_CALIBRATION_HISTORY],
            Self::AuxSignalsOverrides => &[CF_MEJEPA_AUX_SIGNALS_OVERRIDES],
            Self::ShiftLogSubscriberState => &[CF_MEJEPA_SHIFT_WATERMARK],
        }
    }

    pub fn from_cf(cf_name: &str) -> Option<Self> {
        Self::all()
            .into_iter()
            .find(|category| category.cf_names().contains(&cf_name))
    }
}
