//! Tests for ModelHandle VRAM pointer tracking.

use super::helpers::{GB, MB};
use crate::warm::handle::ModelHandle;

#[test]
fn test_handle_creation() {
    let handle = ModelHandle::new(0x1000_0000, 512 * MB, 0, 0xDEAD_BEEF);

    assert_eq!(handle.vram_address(), 0x1000_0000);
    assert_eq!(handle.allocation_bytes(), 512 * MB);
    assert_eq!(handle.device_ordinal(), 0);
    assert_eq!(handle.weight_checksum(), 0xDEAD_BEEF);
}

#[test]
fn test_handle_vram_address_hex() {
    let handle = ModelHandle::new(0x1000_0000, 512 * MB, 0, 0xDEAD_BEEF);
    let hex = handle.vram_address_hex();
    assert!(hex.contains("10000000"));
}

#[test]
fn test_handle_different_devices() {
    let handle0 = ModelHandle::new(0x1000, 1024, 0, 0);
    let handle1 = ModelHandle::new(0x2000, 2048, 1, 0);

    assert_eq!(handle0.device_ordinal(), 0);
    assert_eq!(handle1.device_ordinal(), 1);
}

#[test]
fn test_handle_checksum_verification() {
    let checksum = 0xCAFE_BABE_DEAD_BEEF;
    let handle = ModelHandle::new(0x1000, 1024, 0, checksum);
    assert_eq!(handle.weight_checksum(), checksum);
}

#[test]
fn test_handle_large_allocation() {
    let handle = ModelHandle::new(0x1000_0000, 24 * GB, 0, 0);
    assert_eq!(handle.allocation_bytes(), 24 * GB);
}

#[test]
fn test_handle_zero_allocation() {
    let handle = ModelHandle::new(0x1000, 0, 0, 0);
    assert_eq!(handle.allocation_bytes(), 0);
}

#[test]
fn test_handle_not_clone_by_design() {
    // ModelHandle is intentionally NOT Clone/Copy to prevent VRAM ownership duplication
    // This test verifies we can create multiple handles with same data
    let handle1 = ModelHandle::new(0x1000_0000, 512 * MB, 0, 0xDEAD_BEEF);
    let handle2 = ModelHandle::new(0x1000_0000, 512 * MB, 0, 0xDEAD_BEEF);

    assert_eq!(handle1.vram_address(), handle2.vram_address());
    assert_eq!(handle1.allocation_bytes(), handle2.allocation_bytes());
    assert_eq!(handle1.device_ordinal(), handle2.device_ordinal());
    assert_eq!(handle1.weight_checksum(), handle2.weight_checksum());
}
