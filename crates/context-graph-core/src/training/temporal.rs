//! Phase 5 — Temporal labels for training records.
//!
//! Pure derivation from:
//! - The memory's fingerprint `created_at` (UTC wall time)
//! - Optional `SourceMetadata.session_id` / `session_sequence`
//! - Caller-provided `export_now` (for stable age computation across the run)
//!
//! No I/O. All fields are deterministic given the inputs.

use chrono::{DateTime, Datelike, Timelike, Utc};
use serde::{Deserialize, Serialize};

use crate::types::fingerprint::TeleologicalFingerprint;
use crate::types::SourceMetadata;

/// Coarse weekday/weekend × morning/afternoon/evening/night bucket.
///
/// Boundaries (local-naive, applied directly to UTC hour):
/// - Morning:   05..12
/// - Afternoon: 12..17
/// - Evening:   17..21
/// - Night:     21..05
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeriodicBucket {
    WeekdayMorning,
    WeekdayAfternoon,
    WeekdayEvening,
    WeekdayNight,
    WeekendMorning,
    WeekendAfternoon,
    WeekendEvening,
    WeekendNight,
}

impl PeriodicBucket {
    /// Derive the bucket from a day-of-week (Monday=1, Sunday=7 per
    /// `chrono::Datelike::iso_weekday().number_from_monday()`) and an hour-of-day.
    pub fn from_dow_hour(dow_mon1_sun7: u8, hour: u8) -> Self {
        let is_weekend = matches!(dow_mon1_sun7, 6 | 7);
        match (is_weekend, hour) {
            (true, 5..=11) => PeriodicBucket::WeekendMorning,
            (true, 12..=16) => PeriodicBucket::WeekendAfternoon,
            (true, 17..=20) => PeriodicBucket::WeekendEvening,
            (true, _) => PeriodicBucket::WeekendNight,
            (false, 5..=11) => PeriodicBucket::WeekdayMorning,
            (false, 12..=16) => PeriodicBucket::WeekdayAfternoon,
            (false, 17..=20) => PeriodicBucket::WeekdayEvening,
            (false, _) => PeriodicBucket::WeekdayNight,
        }
    }
}

/// Temporal features derived for a single memory at export time.
///
/// All "stored_*" fields come from `fingerprint.created_at`.
/// `age_seconds_at_export` is `export_now - stored_at`.
/// `session_sequence`/`session_total`/`relative_position` come from the
/// caller (session bookkeeping lives outside this module).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TemporalLabels {
    /// UTC wall-clock time the memory was stored.
    pub stored_at: DateTime<Utc>,
    /// 0..=23.
    pub stored_hour_utc: u8,
    /// 1=Monday, 7=Sunday (ISO weekday).
    pub stored_day_of_week: u8,
    /// 1..=12.
    pub stored_month: u8,
    /// `export_now - stored_at` in seconds. May be negative if the caller
    /// passes an `export_now` in the past (not expected in production).
    pub age_seconds_at_export: i64,
    /// Position within the creating session, when known.
    pub session_sequence: Option<u32>,
    /// Total memories in that session, when known.
    pub session_total: Option<u32>,
    /// `session_sequence / session_total` in [0, 1], when both are known.
    pub relative_position: Option<f32>,
    pub periodic_bucket: PeriodicBucket,
    /// L2 norm of the E2 temporal-recent vector (0.0 when empty).
    pub e2_recency_norm: f32,
    /// L2 norm of the E3 temporal-periodic vector.
    pub e3_periodic_norm: f32,
    /// L2 norm of the E4 temporal-positional vector.
    pub e4_positional_norm: f32,
}

/// Extract temporal labels for a single fingerprint.
///
/// Inputs are taken by reference; the function performs no I/O.
///
/// - `stored_at` = `fingerprint.created_at`
/// - Hour/day/month are read from the UTC representation.
/// - `age_seconds_at_export = export_now - stored_at`.
/// - `relative_position = session_sequence / session_total` when both provided.
///
/// Caller responsibilities:
/// - Resolve `session_sequence`/`session_total` from the session bookkeeping
///   backing store (may pass `None` both ways if not tracked for stored memories).
/// - Pick a single `export_now` for the whole export run so every record is
///   aged against the same instant.
pub fn extract_temporal_labels(
    fingerprint: &TeleologicalFingerprint,
    _source_metadata: Option<&SourceMetadata>,
    session_sequence: Option<u32>,
    session_total: Option<u32>,
    export_now: DateTime<Utc>,
) -> TemporalLabels {
    let stored_at = fingerprint.created_at;
    let hour = stored_at.hour() as u8;
    let dow = stored_at.weekday().number_from_monday() as u8;
    let month = stored_at.month() as u8;
    let bucket = PeriodicBucket::from_dow_hour(dow, hour);
    let age_seconds = (export_now - stored_at).num_seconds();

    let relative_position = match (session_sequence, session_total) {
        (Some(seq), Some(total)) if total > 0 => Some(seq as f32 / total as f32),
        _ => None,
    };

    let e2_recency_norm = l2_norm(&fingerprint.semantic.e2_temporal_recent);
    let e3_periodic_norm = l2_norm(&fingerprint.semantic.e3_temporal_periodic);
    let e4_positional_norm = l2_norm(&fingerprint.semantic.e4_temporal_positional);

    TemporalLabels {
        stored_at,
        stored_hour_utc: hour,
        stored_day_of_week: dow,
        stored_month: month,
        age_seconds_at_export: age_seconds,
        session_sequence,
        session_total,
        relative_position,
        periodic_bucket: bucket,
        e2_recency_norm,
        e3_periodic_norm,
        e4_positional_norm,
    }
}

fn l2_norm(v: &[f32]) -> f32 {
    if v.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = v.iter().map(|x| x * x).sum();
    if !sum_sq.is_finite() || sum_sq <= 0.0 {
        0.0
    } else {
        sum_sq.sqrt()
    }
}
