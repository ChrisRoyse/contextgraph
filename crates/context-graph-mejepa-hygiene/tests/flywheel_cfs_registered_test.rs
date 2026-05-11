//! REQ-FLYWHEEL-10 integration FSV: a fresh hygiene RocksDB root created by
//! `open_hygiene_rocksdb` MUST land all three new flywheel column families on
//! disk. Daemon startup uses this exact opener, so if this regresses, the
//! daemon will panic on the first put_cf call into a missing CF.
//!
//! The test is `#[cfg(unix)]`-free; it uses `tempfile` (a regular dev-dep)
//! and asserts post-state by inspecting the live `DB::cf_handle` map.

use context_graph_mejepa_cf::{
    CF_MEJEPA_AGENT_FEEDBACK, CF_MEJEPA_DDA_SIGNALS, CF_MEJEPA_FAILURE_EXEMPLARS, FLYWHEEL_CFS,
};
use context_graph_mejepa_hygiene::storage::open_hygiene_rocksdb;

#[test]
fn fresh_hygiene_root_creates_all_three_flywheel_cfs() {
    let tmp = tempfile::tempdir().expect("create temp dir");
    let db_path = tmp.path().join("hygiene-rocks");

    let db = open_hygiene_rocksdb(&db_path).expect("open hygiene rocksdb");

    for cf_name in FLYWHEEL_CFS {
        assert!(
            db.cf_handle(cf_name).is_some(),
            "FLYWHEEL CF {cf_name} missing after open_hygiene_rocksdb; \
             daemon startup would fail on the first put_cf call",
        );
    }
    assert!(db.cf_handle(CF_MEJEPA_DDA_SIGNALS).is_some());
    assert!(db.cf_handle(CF_MEJEPA_FAILURE_EXEMPLARS).is_some());
    assert!(db.cf_handle(CF_MEJEPA_AGENT_FEEDBACK).is_some());
}

#[test]
fn reopen_existing_hygiene_root_retains_flywheel_cfs() {
    // Crash-recovery scenario: daemon writes, restarts, must still see the
    // CFs without recreating them (RocksDB would error on duplicate creation
    // unless `create_missing_column_families` is set + CF didn't exist).
    let tmp = tempfile::tempdir().expect("create temp dir");
    let db_path = tmp.path().join("hygiene-rocks");

    {
        let _db = open_hygiene_rocksdb(&db_path).expect("first open");
        // first DB goes out of scope here -> rocksdb closes
    }

    let db = open_hygiene_rocksdb(&db_path).expect("second open after close");
    for cf_name in FLYWHEEL_CFS {
        assert!(
            db.cf_handle(cf_name).is_some(),
            "FLYWHEEL CF {cf_name} lost across reopen",
        );
    }
}
