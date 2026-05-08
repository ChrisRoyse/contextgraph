//! DynamicJEPA RocksDB column-family definitions.

use rocksdb::{BlockBasedOptions, Cache, ColumnFamilyDescriptor, Options, SliceTransform};

use crate::column_families::apply_write_buffer_limits;

pub const CF_DJ_DOMAIN_PACKS: &str = "dj_domain_packs";
pub const CF_DJ_DOMAIN_PACK_BY_NAME_VERSION: &str = "dj_domain_pack_by_nv";
pub const CF_DJ_INSTRUMENT_REGISTRY: &str = "dj_instrument_registry";
pub const CF_DJ_ADAPTER_REGISTRY: &str = "dj_adapter_registry";

pub const CF_DJ_RAW_EVENTS: &str = "dj_raw_events";
pub const CF_DJ_NORMALIZED_STATES: &str = "dj_normalized_states";
pub const CF_DJ_ACTIONS: &str = "dj_actions";
pub const CF_DJ_OUTCOMES: &str = "dj_outcomes";
pub const CF_DJ_TRANSITIONS: &str = "dj_transitions";
pub const CF_DJ_ADAPTER_RUNS: &str = "dj_adapter_runs";

pub const CF_DJ_INSTRUMENT_READINGS: &str = "dj_instrument_readings";
pub const CF_DJ_LATENT_PANELS: &str = "dj_latent_panels";
pub const CF_DJ_PAIRWISE_READINGS: &str = "dj_pairwise_readings";
pub const CF_DJ_CONSTELLATIONS: &str = "dj_constellations";
pub const CF_DJ_THRESHOLD_CALIBRATIONS: &str = "dj_threshold_calibrations";

pub const CF_DJ_BINDINGS: &str = "dj_bindings";
pub const CF_DJ_BINDINGS_BY_ENTITY: &str = "dj_bindings_by_entity";

pub const CF_DJ_TRAJECTORIES: &str = "dj_trajectories";
pub const CF_DJ_DATASET_SHARDS: &str = "dj_dataset_shards";

pub const CF_DJ_TRAINING_RUNS: &str = "dj_training_runs";
pub const CF_DJ_MODEL_ARTIFACTS: &str = "dj_model_artifacts";

pub const CF_DJ_PREDICTIONS: &str = "dj_predictions";
pub const CF_DJ_SKILL_POLICIES: &str = "dj_skill_policies";
pub const CF_DJ_PLAN_TRACES: &str = "dj_plan_traces";
pub const CF_DJ_GUARD_DECISIONS: &str = "dj_guard_decisions";
pub const CF_DJ_SURPRISE_EVENTS: &str = "dj_surprise_events";

pub const CF_DJ_VERIFICATION_RUNS: &str = "dj_verification_runs";
pub const CF_DJ_AUDIT_LOG: &str = "dj_audit_log";
pub const CF_DJ_AUDIT_WITNESS_CHAIN: &str = "dj_audit_witness_chain";

pub const DJ_CF_NAMES: &[&str] = &[
    CF_DJ_DOMAIN_PACKS,
    CF_DJ_DOMAIN_PACK_BY_NAME_VERSION,
    CF_DJ_INSTRUMENT_REGISTRY,
    CF_DJ_ADAPTER_REGISTRY,
    CF_DJ_RAW_EVENTS,
    CF_DJ_NORMALIZED_STATES,
    CF_DJ_ACTIONS,
    CF_DJ_OUTCOMES,
    CF_DJ_TRANSITIONS,
    CF_DJ_ADAPTER_RUNS,
    CF_DJ_INSTRUMENT_READINGS,
    CF_DJ_LATENT_PANELS,
    CF_DJ_PAIRWISE_READINGS,
    CF_DJ_CONSTELLATIONS,
    CF_DJ_THRESHOLD_CALIBRATIONS,
    CF_DJ_BINDINGS,
    CF_DJ_BINDINGS_BY_ENTITY,
    CF_DJ_TRAJECTORIES,
    CF_DJ_DATASET_SHARDS,
    CF_DJ_TRAINING_RUNS,
    CF_DJ_MODEL_ARTIFACTS,
    CF_DJ_PREDICTIONS,
    CF_DJ_SKILL_POLICIES,
    CF_DJ_PLAN_TRACES,
    CF_DJ_GUARD_DECISIONS,
    CF_DJ_SURPRISE_EVENTS,
    CF_DJ_VERIFICATION_RUNS,
    CF_DJ_AUDIT_LOG,
    CF_DJ_AUDIT_WITNESS_CHAIN,
];

pub const DJ_CF_COUNT: usize = DJ_CF_NAMES.len();

fn block_options(cache: &Cache, block_size_kb: usize, bloom: bool) -> BlockBasedOptions {
    let mut block_opts = BlockBasedOptions::default();
    block_opts.set_block_cache(cache);
    block_opts.set_cache_index_and_filter_blocks(true);
    block_opts.set_block_size(block_size_kb * 1024);
    if bloom {
        block_opts.set_bloom_filter(10.0, false);
    }
    block_opts
}

fn options_with_prefix(
    cache: &Cache,
    block_size_kb: usize,
    write_buffer_mb: usize,
    prefix_len: Option<usize>,
    bloom: bool,
) -> Options {
    let block_opts = block_options(cache, block_size_kb, bloom);
    let mut opts = Options::default();
    opts.set_block_based_table_factory(&block_opts);
    opts.set_compression_type(rocksdb::DBCompressionType::Lz4);
    opts.set_compaction_style(rocksdb::DBCompactionStyle::Level);
    if let Some(prefix_len) = prefix_len {
        opts.set_prefix_extractor(SliceTransform::create_fixed_prefix(prefix_len));
    }
    apply_write_buffer_limits(&mut opts, write_buffer_mb);
    opts.create_if_missing(true);
    opts
}

pub fn dj_uuid_point_cf_options(cache: &Cache) -> Options {
    options_with_prefix(cache, 8, 4, Some(16), true)
}

pub fn dj_composite_index_cf_options(cache: &Cache) -> Options {
    options_with_prefix(cache, 8, 4, Some(16), true)
}

pub fn dj_secondary_index_cf_options(cache: &Cache) -> Options {
    options_with_prefix(cache, 8, 1, Some(16), false)
}

pub fn dj_append_only_cf_options(cache: &Cache, prefix_len: Option<usize>) -> Options {
    options_with_prefix(cache, 8, 4, prefix_len, true)
}

pub fn dj_large_blob_cf_options(cache: &Cache) -> Options {
    options_with_prefix(cache, 64, 8, Some(16), true)
}

pub fn dj_cf_descriptors(cache: &Cache) -> Vec<ColumnFamilyDescriptor> {
    vec![
        ColumnFamilyDescriptor::new(CF_DJ_DOMAIN_PACKS, dj_uuid_point_cf_options(cache)),
        ColumnFamilyDescriptor::new(
            CF_DJ_DOMAIN_PACK_BY_NAME_VERSION,
            dj_composite_index_cf_options(cache),
        ),
        ColumnFamilyDescriptor::new(
            CF_DJ_INSTRUMENT_REGISTRY,
            dj_composite_index_cf_options(cache),
        ),
        ColumnFamilyDescriptor::new(CF_DJ_ADAPTER_REGISTRY, dj_composite_index_cf_options(cache)),
        ColumnFamilyDescriptor::new(CF_DJ_RAW_EVENTS, dj_uuid_point_cf_options(cache)),
        ColumnFamilyDescriptor::new(CF_DJ_NORMALIZED_STATES, dj_uuid_point_cf_options(cache)),
        ColumnFamilyDescriptor::new(CF_DJ_ACTIONS, dj_uuid_point_cf_options(cache)),
        ColumnFamilyDescriptor::new(CF_DJ_OUTCOMES, dj_uuid_point_cf_options(cache)),
        ColumnFamilyDescriptor::new(CF_DJ_TRANSITIONS, dj_uuid_point_cf_options(cache)),
        ColumnFamilyDescriptor::new(
            CF_DJ_ADAPTER_RUNS,
            dj_append_only_cf_options(cache, Some(16)),
        ),
        ColumnFamilyDescriptor::new(
            CF_DJ_INSTRUMENT_READINGS,
            dj_composite_index_cf_options(cache),
        ),
        ColumnFamilyDescriptor::new(CF_DJ_LATENT_PANELS, dj_large_blob_cf_options(cache)),
        ColumnFamilyDescriptor::new(
            CF_DJ_PAIRWISE_READINGS,
            dj_append_only_cf_options(cache, Some(16)),
        ),
        ColumnFamilyDescriptor::new(CF_DJ_CONSTELLATIONS, dj_composite_index_cf_options(cache)),
        ColumnFamilyDescriptor::new(
            CF_DJ_THRESHOLD_CALIBRATIONS,
            dj_composite_index_cf_options(cache),
        ),
        ColumnFamilyDescriptor::new(CF_DJ_BINDINGS, dj_uuid_point_cf_options(cache)),
        ColumnFamilyDescriptor::new(
            CF_DJ_BINDINGS_BY_ENTITY,
            dj_secondary_index_cf_options(cache),
        ),
        ColumnFamilyDescriptor::new(CF_DJ_TRAJECTORIES, dj_uuid_point_cf_options(cache)),
        ColumnFamilyDescriptor::new(CF_DJ_DATASET_SHARDS, dj_large_blob_cf_options(cache)),
        ColumnFamilyDescriptor::new(
            CF_DJ_TRAINING_RUNS,
            dj_append_only_cf_options(cache, Some(16)),
        ),
        ColumnFamilyDescriptor::new(CF_DJ_MODEL_ARTIFACTS, dj_uuid_point_cf_options(cache)),
        ColumnFamilyDescriptor::new(
            CF_DJ_PREDICTIONS,
            dj_append_only_cf_options(cache, Some(16)),
        ),
        ColumnFamilyDescriptor::new(CF_DJ_SKILL_POLICIES, dj_uuid_point_cf_options(cache)),
        ColumnFamilyDescriptor::new(
            CF_DJ_PLAN_TRACES,
            dj_append_only_cf_options(cache, Some(16)),
        ),
        ColumnFamilyDescriptor::new(
            CF_DJ_GUARD_DECISIONS,
            dj_append_only_cf_options(cache, Some(16)),
        ),
        ColumnFamilyDescriptor::new(
            CF_DJ_SURPRISE_EVENTS,
            dj_append_only_cf_options(cache, Some(16)),
        ),
        ColumnFamilyDescriptor::new(
            CF_DJ_VERIFICATION_RUNS,
            dj_append_only_cf_options(cache, Some(16)),
        ),
        ColumnFamilyDescriptor::new(CF_DJ_AUDIT_LOG, dj_append_only_cf_options(cache, Some(8))),
        ColumnFamilyDescriptor::new(
            CF_DJ_AUDIT_WITNESS_CHAIN,
            dj_append_only_cf_options(cache, Some(8)),
        ),
    ]
}
