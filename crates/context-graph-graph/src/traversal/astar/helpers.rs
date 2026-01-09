//! A* helper functions.
//!
//! Utility functions for UUID conversion, point conversion, and edge costs.

use uuid::Uuid;

use crate::hyperbolic::PoincarePoint as HyperbolicPoint;
use crate::storage::PoincarePoint as StoragePoint;

/// Convert UUID to i64 for storage key operations.
///
/// This reverses `Uuid::from_u64_pair(id as u64, 0)` used in storage.
#[inline]
pub(crate) fn uuid_to_i64(uuid: &Uuid) -> i64 {
    let bytes = uuid.as_bytes();
    i64::from_be_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}

/// Convert storage PoincarePoint to hyperbolic PoincarePoint.
///
/// Both types have the same underlying [f32; 64] coords, just in different modules.
#[inline]
pub(crate) fn to_hyperbolic_point(storage_point: StoragePoint) -> HyperbolicPoint {
    HyperbolicPoint::from_coords(storage_point.coords)
}

/// Compute edge cost from effective weight.
///
/// Higher weight = lower cost = preferred.
/// Cost = 1.0 / (weight + epsilon)
#[inline]
pub(crate) fn edge_cost(weight: f32) -> f32 {
    // Clamp weight to valid range
    let w = weight.clamp(0.0, 1.0);
    1.0 / (w + 0.001)
}
