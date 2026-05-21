use crate::error::TrainerError;
use context_graph_mejepa_cf::{
    CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP, CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS, CF_MEJEPA_MISTAKE_LOG,
    CF_MEJEPA_ONLINE_HEAD_STATE, CF_MEJEPA_REPLAY_BUFFER, CF_MEJEPA_SKILL_LIFECYCLE_AUDIT,
    CF_MEJEPA_SKILL_REVERSE_INDEX,
};
use rocksdb::{ColumnFamilyDescriptor, Options, DB};
use std::path::Path;

use super::invalid;

pub fn open_ability_resolver_rocksdb(
    path: &Path,
    create_if_missing: bool,
) -> Result<DB, TrainerError> {
    let mut opts = Options::default();
    opts.create_if_missing(create_if_missing);
    opts.create_missing_column_families(create_if_missing);
    opts.set_paranoid_checks(true);
    let descriptors = ability_resolver_cfs()
        .iter()
        .map(|name| ColumnFamilyDescriptor::new(name.as_str(), Options::default()))
        .collect::<Vec<_>>();
    let db = DB::open_cf_descriptors(&opts, path, descriptors)?;
    for cf_name in ability_resolver_cfs() {
        if db.cf_handle(&cf_name).is_none() {
            return Err(invalid(
                "rocksdb.column_family",
                format!("missing {cf_name} after open"),
            ));
        }
    }
    Ok(db)
}

pub fn ability_resolver_cfs() -> Vec<String> {
    vec![
        CF_MEJEPA_FAILURE_MODE_LEVEL2_SKILLS.to_string(),
        CF_MEJEPA_CHUNK_SKILL_MEMBERSHIP.to_string(),
        CF_MEJEPA_SKILL_REVERSE_INDEX.to_string(),
        CF_MEJEPA_SKILL_LIFECYCLE_AUDIT.to_string(),
        CF_MEJEPA_MISTAKE_LOG.to_string(),
        CF_MEJEPA_REPLAY_BUFFER.to_string(),
        CF_MEJEPA_ONLINE_HEAD_STATE.to_string(),
    ]
}
