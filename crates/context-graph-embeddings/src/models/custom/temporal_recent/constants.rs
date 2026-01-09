//! Constants and configuration for the Temporal-Recent embedding model.

/// Native dimension for TemporalRecent model (E2).
pub const TEMPORAL_RECENT_DIMENSION: usize = 512;

/// Default decay rates for different time scales (in reciprocal seconds).
/// - Index 0: 1 hour scale (1/3600)
/// - Index 1: 1 day scale (1/86400)
/// - Index 2: 1 week scale (1/604800)
/// - Index 3: 1 month scale (1/2592000)
pub const DEFAULT_DECAY_RATES: [f32; 4] = [
    1.0 / 3600.0,    // 1 hour scale
    1.0 / 86400.0,   // 1 day scale
    1.0 / 604800.0,  // 1 week scale
    1.0 / 2592000.0, // 1 month scale (~30 days)
];

/// Features per time scale (4 scales x 128 = 512 total).
pub const FEATURES_PER_SCALE: usize = 128;

/// Maximum time delta to prevent numerical overflow (1 year in seconds).
pub const MAX_TIME_DELTA_SECS: f32 = 31536000.0;

/// Number of time scales.
pub const NUM_TIME_SCALES: usize = 4;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants_are_correct() {
        assert_eq!(TEMPORAL_RECENT_DIMENSION, 512);
        assert_eq!(DEFAULT_DECAY_RATES.len(), 4);
        assert_eq!(FEATURES_PER_SCALE, 128);
        assert_eq!(MAX_TIME_DELTA_SECS, 31536000.0); // 1 year
        assert_eq!(
            NUM_TIME_SCALES * FEATURES_PER_SCALE,
            TEMPORAL_RECENT_DIMENSION
        );
    }

    #[test]
    fn test_decay_rates_are_positive() {
        for rate in DEFAULT_DECAY_RATES {
            assert!(rate > 0.0, "Decay rate must be positive");
            assert!(rate.is_finite(), "Decay rate must be finite");
        }
    }
}
