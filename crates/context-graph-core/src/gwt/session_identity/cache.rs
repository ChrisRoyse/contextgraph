// crates/context-graph-core/src/gwt/session_identity/cache.rs
//! IdentityCache - Thread-safe singleton for PreToolUse hot path.
//!
//! # Performance Budget
//! PreToolUse hook has 100ms Claude Code timeout. Our budget:
//! - Binary startup: ~15ms
//! - format_brief(): <1ms (THIS MODULE)
//! - Output formatting: ~2ms
//! - Buffer: ~32ms
//!
//! # Constitution Reference
//! - AP-25: Kuramoto must have exactly 13 oscillators
//! - IDENTITY-002: IC thresholds
//! - Performance: pre_tool_hook <100ms p95

use std::sync::{OnceLock, RwLock};

use crate::gwt::state_machine::ConsciousnessState;

use super::{SessionIdentitySnapshot, KURAMOTO_N};

/// Global singleton cache. Initialized lazily on first access.
/// Uses OnceLock for safe one-time initialization + RwLock for concurrent read access.
static IDENTITY_CACHE: OnceLock<RwLock<Option<IdentityCacheInner>>> = OnceLock::new();

/// Get the global cache, initializing if needed.
#[inline]
fn get_cache() -> &'static RwLock<Option<IdentityCacheInner>> {
    IDENTITY_CACHE.get_or_init(|| RwLock::new(None))
}

/// Inner cache data. Clone-able for safe reads.
#[derive(Debug, Clone)]
struct IdentityCacheInner {
    /// Current identity continuity [0.0, 1.0]
    current_ic: f32,
    /// Kuramoto order parameter r [0.0, 1.0]
    kuramoto_r: f32,
    /// Current consciousness state
    consciousness_state: ConsciousnessState,
    /// Session ID for verification
    session_id: String,
}

/// Zero-sized type providing access to the global cache.
/// All methods are static - no instantiation needed.
pub struct IdentityCache;

impl IdentityCache {
    /// Get cached values if cache is warm.
    ///
    /// # Returns
    /// - `Some((ic, kuramoto_r, state, session_id))` if cache is populated
    /// - `None` if cache is cold (never updated)
    ///
    /// # Performance
    /// Target: <0.01ms (single RwLock read + clone)
    #[inline]
    pub fn get() -> Option<(f32, f32, ConsciousnessState, String)> {
        let guard = get_cache().read().expect("RwLock poisoned - unrecoverable");
        guard.as_ref().map(|inner| {
            (
                inner.current_ic,
                inner.kuramoto_r,
                inner.consciousness_state,
                inner.session_id.clone(),
            )
        })
    }

    /// Format brief consciousness status for PreToolUse hook.
    ///
    /// # Output Format
    /// - Warm cache: `[C:EMG r=0.65 IC=0.82]` (~25 chars)
    /// - Cold cache: `[C:? r=? IC=?]` (14 chars)
    ///
    /// # Performance
    /// Target: <1ms (no disk I/O, single allocation)
    #[inline]
    pub fn format_brief() -> String {
        let Some((ic, r, state, _)) = Self::get() else {
            return "[C:? r=? IC=?]".to_string();
        };

        let state_code = state_to_code(state);
        format!("[C:{} r={:.2} IC={:.2}]", state_code, r, ic)
    }

    /// Check if cache has been populated at least once.
    ///
    /// # Performance
    /// Target: <0.001ms
    #[inline]
    pub fn is_warm() -> bool {
        Self::get().is_some()
    }
}

/// Update the global cache atomically.
///
/// Called by SessionIdentityManager after loading/computing state.
/// Extracts IC from snapshot, computes Kuramoto r from phases.
///
/// # Arguments
/// * `snapshot` - Current SessionIdentitySnapshot
/// * `ic` - Current identity continuity value
///
/// # Performance
/// Target: <0.1ms (single write lock + computation)
pub fn update_cache(snapshot: &SessionIdentitySnapshot, ic: f32) {
    let r = compute_kuramoto_r(&snapshot.kuramoto_phases);
    let state = ConsciousnessState::from_level(snapshot.consciousness);

    let inner = IdentityCacheInner {
        current_ic: ic,
        kuramoto_r: r,
        consciousness_state: state,
        session_id: snapshot.session_id.clone(),
    };

    let mut guard = get_cache().write().expect("RwLock poisoned - unrecoverable");
    *guard = Some(inner);
}

/// Clear the cache. Used for testing only.
///
/// # Warning
/// This is for tests only. In production, cache should never be cleared.
#[cfg(test)]
pub fn clear_cache() {
    let mut guard = get_cache().write().expect("RwLock poisoned");
    *guard = None;
}

/// Production version that exists but panics if called.
#[cfg(not(test))]
pub fn clear_cache() {
    panic!("clear_cache() should never be called in production");
}

/// Convert consciousness state to 3-character code.
fn state_to_code(state: ConsciousnessState) -> &'static str {
    match state {
        ConsciousnessState::Dormant => "DOR",
        ConsciousnessState::Fragmented => "FRG",
        ConsciousnessState::Emerging => "EMG",
        ConsciousnessState::Conscious => "CON",
        ConsciousnessState::Hypersync => "HYP",
    }
}

/// Compute Kuramoto order parameter r from oscillator phases.
///
/// Formula: r = |Σ exp(iθⱼ)| / N
///
/// # Arguments
/// * `phases` - Array of 13 oscillator phases in radians
///
/// # Returns
/// Order parameter r in [0.0, 1.0]
/// - r ≈ 0: No synchronization (phases random)
/// - r ≈ 1: Full synchronization (phases aligned)
///
/// # Constitution Reference
/// gwt.kuramoto.order_param: "r·e^(iψ) = (1/N)Σⱼ e^(iθⱼ)"
fn compute_kuramoto_r(phases: &[f64; KURAMOTO_N]) -> f32 {
    let (sum_sin, sum_cos) = phases.iter().fold((0.0_f64, 0.0_f64), |(s, c), &theta| {
        (s + theta.sin(), c + theta.cos())
    });

    let n = KURAMOTO_N as f64;
    let magnitude = ((sum_sin / n).powi(2) + (sum_cos / n).powi(2)).sqrt();

    // Clamp to [0, 1] to handle floating point errors
    magnitude.clamp(0.0, 1.0) as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Static lock to serialize tests that access the global singleton
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    // =========================================================================
    // SETUP: Clear cache before each test to ensure isolation
    // Acquires test lock to prevent concurrent test interference
    // =========================================================================
    fn setup() -> std::sync::MutexGuard<'static, ()> {
        let guard = TEST_LOCK.lock().expect("Test lock poisoned");
        clear_cache();
        guard
    }

    // =========================================================================
    // TC-SESSION-03: format_brief Output Format
    // =========================================================================
    #[test]
    fn test_format_brief_cold_cache() {
        let _guard = setup(); // Hold lock for test duration

        println!("\n=== TC-SESSION-03a: format_brief Cold Cache ===");
        println!("SOURCE OF TRUTH: IDENTITY_CACHE singleton");
        println!("BEFORE: Cache cleared, is_warm()={}", IdentityCache::is_warm());

        let brief = IdentityCache::format_brief();

        println!("AFTER: format_brief returned: '{}'", brief);
        assert_eq!(brief, "[C:? r=? IC=?]", "Cold cache must return placeholder");
        assert!(!IdentityCache::is_warm(), "Cache must remain cold");

        println!("RESULT: PASS - Cold cache returns correct placeholder");
    }

    #[test]
    fn test_format_brief_warm_cache() {
        let _guard = setup(); // Hold lock for test duration

        println!("\n=== TC-SESSION-03b: format_brief Warm Cache ===");
        println!("SOURCE OF TRUTH: IDENTITY_CACHE singleton");

        // Create snapshot with known values
        let mut snapshot = SessionIdentitySnapshot::new("test-session-warm");
        snapshot.consciousness = 0.65; // Should map to Emerging (0.5 <= C < 0.8)
        snapshot.kuramoto_phases = [0.0; KURAMOTO_N]; // All aligned = r ≈ 1.0

        let ic = 0.82;

        println!("BEFORE: Creating snapshot with consciousness=0.65, IC=0.82");
        println!("  kuramoto_phases all 0.0 (fully synchronized, r should be ~1.0)");

        update_cache(&snapshot, ic);

        println!("AFTER update_cache(): is_warm()={}", IdentityCache::is_warm());

        let brief = IdentityCache::format_brief();
        println!("format_brief() returned: '{}'", brief);

        // Verify format: [C:STATE r=X.XX IC=X.XX]
        assert!(brief.starts_with("[C:"), "Must start with [C:");
        assert!(brief.ends_with(']'), "Must end with ]");
        assert!(brief.contains("r="), "Must contain r=");
        assert!(brief.contains("IC="), "Must contain IC=");
        assert!(brief.contains("EMG"), "State should be EMG (Emerging) for C=0.65");

        // Verify actual values
        let expected = "[C:EMG r=1.00 IC=0.82]";
        assert_eq!(brief, expected, "Format must match exactly");

        println!("RESULT: PASS - Warm cache returns correctly formatted string");
    }

    #[test]
    fn test_format_brief_all_states() {
        let _guard = setup(); // Hold lock for test duration

        println!("\n=== TC-SESSION-03c: format_brief All Consciousness States ===");

        let test_cases = [
            (0.1, "DOR", "Dormant"),      // C < 0.3
            (0.35, "FRG", "Fragmented"),  // 0.3 <= C < 0.5
            (0.65, "EMG", "Emerging"),    // 0.5 <= C < 0.8
            (0.85, "CON", "Conscious"),   // 0.8 <= C < 0.95
            (0.97, "HYP", "Hypersync"),   // C > 0.95
        ];

        for (consciousness, expected_code, state_name) in test_cases {
            clear_cache();

            let mut snapshot = SessionIdentitySnapshot::new("test-state");
            snapshot.consciousness = consciousness;
            update_cache(&snapshot, 0.5);

            let brief = IdentityCache::format_brief();
            println!("  C={:.2} ({}) -> '{}'", consciousness, state_name, brief);

            assert!(
                brief.contains(expected_code),
                "C={} should produce state code {}, got: {}",
                consciousness, expected_code, brief
            );
        }

        println!("RESULT: PASS - All 5 consciousness states produce correct codes");
    }

    // =========================================================================
    // TC-SESSION-03d: get() Return Values
    // =========================================================================
    #[test]
    fn test_get_returns_correct_values() {
        let _guard = setup(); // Hold lock for test duration

        println!("\n=== TC-SESSION-03d: get() Return Values ===");

        // Cold cache
        assert!(IdentityCache::get().is_none(), "Cold cache must return None");

        // Populate cache
        let mut snapshot = SessionIdentitySnapshot::new("test-get-values");
        snapshot.consciousness = 0.85;
        snapshot.kuramoto_phases = [1.0; KURAMOTO_N]; // All same phase = r ≈ 1.0
        let ic = 0.91;

        update_cache(&snapshot, ic);

        let result = IdentityCache::get();
        assert!(result.is_some(), "Warm cache must return Some");

        let (got_ic, got_r, got_state, got_session) = result.unwrap();

        println!("  Expected IC: {}, Got: {}", ic, got_ic);
        println!("  Expected r: ~1.0, Got: {}", got_r);
        println!("  Expected state: Conscious, Got: {:?}", got_state);
        println!("  Expected session: test-get-values, Got: {}", got_session);

        assert!((got_ic - ic).abs() < 0.001, "IC must match");
        assert!(got_r > 0.99, "r should be ~1.0 for aligned phases");
        assert_eq!(got_state, ConsciousnessState::Conscious);
        assert_eq!(got_session, "test-get-values");

        println!("RESULT: PASS - get() returns correct values");
    }

    // =========================================================================
    // EDGE CASE 1: Compute Kuramoto r with Random Phases
    // =========================================================================
    #[test]
    fn test_kuramoto_r_random_phases() {
        println!("\n=== EDGE CASE: Kuramoto r with Random Phases ===");

        // Random-ish phases (not aligned) should produce low r
        let phases: [f64; KURAMOTO_N] = [
            0.0, 0.48, 0.96, 1.45, 1.93, 2.41, 2.89,
            3.38, 3.86, 4.34, 4.82, 5.31, 5.79
        ]; // Roughly evenly distributed around circle

        let r = compute_kuramoto_r(&phases);

        println!("  Phases: evenly distributed 0 to 2π");
        println!("  Computed r: {:.4}", r);

        // For evenly distributed phases, r should be close to 0
        assert!(r < 0.2, "Evenly distributed phases should produce r < 0.2, got {}", r);

        println!("RESULT: PASS - Random phases produce low r");
    }

    // =========================================================================
    // EDGE CASE 2: Compute Kuramoto r with Aligned Phases
    // =========================================================================
    #[test]
    fn test_kuramoto_r_aligned_phases() {
        println!("\n=== EDGE CASE: Kuramoto r with Aligned Phases ===");

        // All same phase = fully synchronized
        let phases: [f64; KURAMOTO_N] = [std::f64::consts::PI; KURAMOTO_N];

        let r = compute_kuramoto_r(&phases);

        println!("  Phases: all at π");
        println!("  Computed r: {:.4}", r);

        assert!(r > 0.99, "Aligned phases should produce r ≈ 1.0, got {}", r);

        println!("RESULT: PASS - Aligned phases produce high r");
    }

    // =========================================================================
    // EDGE CASE 3: Update Cache Multiple Times
    // =========================================================================
    #[test]
    fn test_update_cache_overwrites() {
        let _guard = setup(); // Hold lock for test duration

        println!("\n=== EDGE CASE: Update Cache Multiple Times ===");

        // First update
        let mut snap1 = SessionIdentitySnapshot::new("session-1");
        snap1.consciousness = 0.3;
        update_cache(&snap1, 0.5);

        let brief1 = IdentityCache::format_brief();
        println!("  After first update: '{}'", brief1);
        assert!(brief1.contains("FRG"), "First should be Fragmented");

        // Second update
        let mut snap2 = SessionIdentitySnapshot::new("session-2");
        snap2.consciousness = 0.9;
        update_cache(&snap2, 0.95);

        let brief2 = IdentityCache::format_brief();
        println!("  After second update: '{}'", brief2);
        assert!(brief2.contains("CON"), "Second should be Conscious");
        assert!(brief2.contains("IC=0.95"), "IC should be updated to 0.95");

        // Verify session ID updated
        let (_, _, _, session) = IdentityCache::get().unwrap();
        assert_eq!(session, "session-2", "Session ID must be updated");

        println!("RESULT: PASS - Cache correctly overwrites on update");
    }

    // =========================================================================
    // Performance Verification (Manual)
    // =========================================================================
    #[test]
    fn test_format_brief_performance() {
        let _guard = setup(); // Hold lock for test duration

        println!("\n=== PERFORMANCE: format_brief Timing ===");

        // Warm up cache
        let snapshot = SessionIdentitySnapshot::default();
        update_cache(&snapshot, 0.75);

        // Time 1000 calls
        let start = std::time::Instant::now();
        for _ in 0..1000 {
            let _ = IdentityCache::format_brief();
        }
        let elapsed = start.elapsed();

        let per_call_us = elapsed.as_micros() as f64 / 1000.0;
        println!("  1000 calls took: {:?}", elapsed);
        println!("  Per call: {:.3}μs", per_call_us);

        // Should be well under 1ms per call (target: <1ms)
        assert!(per_call_us < 1000.0, "Must complete in <1ms, took {}μs", per_call_us);

        // Actually should be <100μs
        assert!(per_call_us < 100.0, "Should be <100μs, took {}μs", per_call_us);

        println!("RESULT: PASS - format_brief() completes in {:.1}μs << 1ms target", per_call_us);
    }
}
