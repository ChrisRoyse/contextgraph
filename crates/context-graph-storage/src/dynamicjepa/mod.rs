//! DynamicJEPA RocksDB storage data plane.

pub mod audit;
mod audit_witness;
pub mod bridge;
pub mod column_families;
pub mod keys;

mod artifact_store;
mod binding_store;
mod common;
mod constellation_store;
mod dataset_store;
mod domain_pack_store;
mod encode;
mod event_store;
mod inspection;
mod pairwise_store;
mod panel_store;
mod planning_store;
mod prediction_store;
mod surprise_store;
mod threshold_store;
mod trajectory_store;
mod verification_store;

pub use artifact_store::*;
pub use audit::*;
pub use audit_witness::*;
pub use binding_store::*;
pub use bridge::*;
pub use constellation_store::*;
pub use dataset_store::*;
pub use domain_pack_store::*;
pub use encode::*;
pub use event_store::*;
pub use inspection::*;
pub use pairwise_store::*;
pub use panel_store::*;
pub use planning_store::*;
pub use prediction_store::*;
pub use surprise_store::*;
pub use threshold_store::*;
pub use trajectory_store::*;
pub use verification_store::*;

use rocksdb::DB;

use crate::teleological::RocksDbTeleologicalStore;

impl RocksDbTeleologicalStore {
    /// Return the physical RocksDB source of truth for DynamicJEPA stores.
    pub fn dynamicjepa_db(&self) -> &DB {
        self.db.as_ref()
    }
}

#[cfg(test)]
mod tests;
