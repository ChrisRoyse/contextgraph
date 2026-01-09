//! Tests for PoincarePoint.
//!
//! Comprehensive test suite covering construction, norm calculations,
//! projection, validity, memory layout, and edge cases.
//!
//! # Test Modules
//!
//! - [`construction`]: Tests for point creation methods
//! - [`norm`]: Tests for norm and norm_squared calculations
//! - [`projection`]: Tests for projection to stay inside the ball
//! - [`validity`]: Tests for is_valid and is_valid_for_config
//! - [`memory_layout`]: Tests for size and alignment
//! - [`equality_clone`]: Tests for PartialEq and Clone
//! - [`edge_cases`]: Tests for boundary conditions and special values

mod construction;
mod edge_cases;
mod equality_clone;
mod memory_layout;
mod norm;
mod projection;
mod validity;
