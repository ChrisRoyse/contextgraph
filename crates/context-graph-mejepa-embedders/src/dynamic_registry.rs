use crate::embedder_id::EmbedderId;
use crate::error::{EmbedError, EmbedResult};
use crate::routing::route_for_entity_type;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

const MAX_DYNAMIC_NAME_LEN: usize = 64;
const MAX_DYNAMIC_DIMENSION: usize = 8192;
const MAX_DYNAMIC_EMBEDDERS: usize = 235;
const DYNAMIC_MODEL_ROOT: &str = "/zfs/archive/contextgraph/models/dynamic/";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum RuntimeEmbedderId {
    Static(EmbedderId),
    EDynamic(u32, String),
}

impl RuntimeEmbedderId {
    pub fn dynamic(sequence: u32, name: impl Into<String>) -> EmbedResult<Self> {
        let id = Self::EDynamic(sequence, name.into());
        id.validate()?;
        Ok(id)
    }

    pub fn slug(&self) -> Cow<'_, str> {
        match self {
            Self::Static(id) => Cow::Borrowed(id.slug()),
            Self::EDynamic(sequence, name) => Cow::Owned(format!("edynamic:{sequence}:{name}")),
        }
    }

    pub fn validate(&self) -> EmbedResult<()> {
        match self {
            Self::Static(_) => Ok(()),
            Self::EDynamic(sequence, name) => validate_dynamic_id(*sequence, name),
        }
    }

    pub fn is_dynamic(&self) -> bool {
        matches!(self, Self::EDynamic(_, _))
    }
}

impl fmt::Display for RuntimeEmbedderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.slug())
    }
}

impl From<EmbedderId> for RuntimeEmbedderId {
    fn from(value: EmbedderId) -> Self {
        Self::Static(value)
    }
}

impl FromStr for RuntimeEmbedderId {
    type Err = EmbedError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if let Some(rest) = value.strip_prefix("edynamic:") {
            let (sequence, name) = rest.split_once(':').ok_or_else(|| {
                EmbedError::invalid(
                    "RuntimeEmbedderId",
                    format!("dynamic embedder id {value:?} must be edynamic:<seq>:<name>"),
                    "use the persisted CF_MEJEPA_DYNAMIC_EMBEDDER_REGISTRY id",
                )
            })?;
            let sequence = sequence.parse::<u32>().map_err(|err| {
                EmbedError::invalid(
                    "RuntimeEmbedderId.sequence",
                    format!("invalid dynamic sequence {sequence:?}: {err}"),
                    "use a non-zero u32 promotion sequence number",
                )
            })?;
            return Self::dynamic(sequence, name.to_string());
        }
        EmbedderId::parse(value).map(Self::Static)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DynamicEmbedderKind {
    Algorithmic,
    LearnedHead,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicEmbedderRegistryRecord {
    pub id: RuntimeEmbedderId,
    pub registry_version: u64,
    pub kind: DynamicEmbedderKind,
    pub dimension: usize,
    pub route_languages: Vec<String>,
    pub route_entity_types: Vec<String>,
    pub forward_artifact_path: String,
    pub forward_artifact_sha256: String,
    pub required_vram_bytes: u64,
    pub active: bool,
    pub promoted_at_unix_ms: i64,
}

impl DynamicEmbedderRegistryRecord {
    pub fn validate(&self) -> EmbedResult<()> {
        self.id.validate()?;
        if !self.id.is_dynamic() {
            return Err(EmbedError::invalid(
                "DynamicEmbedderRegistryRecord.id",
                "registry rows must use RuntimeEmbedderId::EDynamic",
                "write static embedders through the existing compile-time registry",
            ));
        }
        if self.registry_version == 0 {
            return Err(EmbedError::invalid(
                "DynamicEmbedderRegistryRecord.registry_version",
                "registry_version must be non-zero",
                "increment the registry version during atomic hot-swap",
            ));
        }
        if self.dimension == 0 || self.dimension > MAX_DYNAMIC_DIMENSION {
            return Err(EmbedError::invalid(
                "DynamicEmbedderRegistryRecord.dimension",
                format!(
                    "dimension must be in [1, {MAX_DYNAMIC_DIMENSION}], got {}",
                    self.dimension
                ),
                "reject malformed candidate embedders before promotion",
            ));
        }
        validate_route_list("route_languages", &self.route_languages)?;
        validate_route_list("route_entity_types", &self.route_entity_types)?;
        if !self.forward_artifact_path.starts_with(DYNAMIC_MODEL_ROOT) {
            return Err(EmbedError::invalid(
                "DynamicEmbedderRegistryRecord.forward_artifact_path",
                format!(
                    "forward artifact path must live under {DYNAMIC_MODEL_ROOT}, got {}",
                    self.forward_artifact_path
                ),
                "persist promoted dynamic embedders under the aiwonder model root",
            ));
        }
        validate_sha256(
            "DynamicEmbedderRegistryRecord.forward_artifact_sha256",
            &self.forward_artifact_sha256,
        )?;
        if self.promoted_at_unix_ms <= 0 {
            return Err(EmbedError::invalid(
                "DynamicEmbedderRegistryRecord.promoted_at_unix_ms",
                "promotion timestamp must be positive",
                "record the wall-clock promotion timestamp from the promoting process",
            ));
        }
        Ok(())
    }

    pub fn matches_route(&self, language: &str, entity_type: &str) -> bool {
        if !self.active {
            return false;
        }
        (self.route_languages.iter().any(|item| item == "*")
            || self.route_languages.iter().any(|item| item == language))
            && (self.route_entity_types.iter().any(|item| item == "*")
                || self
                    .route_entity_types
                    .iter()
                    .any(|item| item == entity_type))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DynamicEmbedderProvenanceRecord {
    pub id: RuntimeEmbedderId,
    pub registry_version: u64,
    pub residual_signal_ref: String,
    pub architecture_generator: String,
    pub training_cert_chain_hash: String,
    pub heldout_global_delta: f32,
    pub heldout_min_cell_delta: f32,
    pub operator_approval_id: Option<String>,
    pub forward_pass_artifact_sha256: String,
    pub created_at_unix_ms: i64,
}

impl DynamicEmbedderProvenanceRecord {
    pub fn validate(&self) -> EmbedResult<()> {
        self.id.validate()?;
        if !self.id.is_dynamic() {
            return Err(EmbedError::invalid(
                "DynamicEmbedderProvenanceRecord.id",
                "provenance rows must use RuntimeEmbedderId::EDynamic",
                "write promoted static-slot evidence through the existing model registry",
            ));
        }
        if self.registry_version == 0 {
            return Err(EmbedError::invalid(
                "DynamicEmbedderProvenanceRecord.registry_version",
                "registry_version must be non-zero",
                "link provenance to the exact atomic registry version",
            ));
        }
        validate_non_empty("residual_signal_ref", &self.residual_signal_ref)?;
        validate_non_empty("architecture_generator", &self.architecture_generator)?;
        validate_sha256(
            "DynamicEmbedderProvenanceRecord.training_cert_chain_hash",
            &self.training_cert_chain_hash,
        )?;
        validate_sha256(
            "DynamicEmbedderProvenanceRecord.forward_pass_artifact_sha256",
            &self.forward_pass_artifact_sha256,
        )?;
        if !self.heldout_global_delta.is_finite() || !self.heldout_min_cell_delta.is_finite() {
            return Err(EmbedError::invalid(
                "DynamicEmbedderProvenanceRecord.heldout_delta",
                "heldout deltas must be finite",
                "reject candidate promotion evidence with NaN or Inf metrics",
            ));
        }
        if self.created_at_unix_ms <= 0 {
            return Err(EmbedError::invalid(
                "DynamicEmbedderProvenanceRecord.created_at_unix_ms",
                "created_at_unix_ms must be positive",
                "record the provenance timestamp from the promotion evaluator",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeRoutingResult {
    pub registry_version: u64,
    pub language: String,
    pub entity_type: String,
    pub embedders: BTreeSet<RuntimeEmbedderId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeRoutingTable {
    pub registry_version: u64,
    pub dynamic_records: Vec<DynamicEmbedderRegistryRecord>,
}

impl RuntimeRoutingTable {
    pub fn new(
        registry_version: u64,
        dynamic_records: Vec<DynamicEmbedderRegistryRecord>,
    ) -> EmbedResult<Self> {
        if registry_version == 0 {
            return Err(EmbedError::invalid(
                "RuntimeRoutingTable.registry_version",
                "registry_version must be non-zero",
                "read the version from CF_MEJEPA_DYNAMIC_EMBEDDER_REGISTRY before routing",
            ));
        }
        if dynamic_records.len() > MAX_DYNAMIC_EMBEDDERS {
            return Err(EmbedError::invalid(
                "RuntimeRoutingTable.dynamic_records",
                format!(
                    "dynamic record count {} exceeds max {MAX_DYNAMIC_EMBEDDERS}",
                    dynamic_records.len()
                ),
                "prune or retire dynamic embedders before loading another promotion",
            ));
        }
        let mut ids = BTreeSet::new();
        for record in &dynamic_records {
            record.validate()?;
            if record.registry_version > registry_version {
                return Err(EmbedError::invalid(
                    "RuntimeRoutingTable.dynamic_records",
                    format!(
                        "{} has registry_version {} newer than table version {registry_version}",
                        record.id, record.registry_version
                    ),
                    "build the runtime routing snapshot after the registry version is committed",
                ));
            }
            if !ids.insert(record.id.clone()) {
                return Err(EmbedError::invalid(
                    "RuntimeRoutingTable.dynamic_records",
                    format!("duplicate dynamic embedder id {}", record.id),
                    "deduplicate registry rows before atomic hot-swap",
                ));
            }
        }
        Ok(Self {
            registry_version,
            dynamic_records,
        })
    }

    pub fn active_dynamic_count(&self) -> usize {
        self.dynamic_records
            .iter()
            .filter(|record| record.active)
            .count()
    }

    pub fn active_embedder_count(&self, static_count: usize) -> usize {
        static_count + self.active_dynamic_count()
    }

    pub fn pairwise_mi_upper_triangle_len(&self, static_count: usize) -> EmbedResult<usize> {
        upper_triangle_len(self.active_embedder_count(static_count))
    }

    pub fn required_vram_bytes(&self, static_required_bytes: u64) -> EmbedResult<u64> {
        let mut total = static_required_bytes;
        for record in self.dynamic_records.iter().filter(|record| record.active) {
            total = total
                .checked_add(record.required_vram_bytes)
                .ok_or_else(|| {
                    EmbedError::invalid(
                        "RuntimeRoutingTable.required_vram_bytes",
                        "dynamic VRAM requirement overflowed u64",
                        "reject the registry snapshot before invoking the GPU budget gate",
                    )
                })?;
        }
        Ok(total)
    }

    pub fn route_for_entity_type(
        &self,
        entity_type: context_graph_core::memory::ast::EntityType,
        language: context_graph_core::memory::ast::Language,
    ) -> EmbedResult<RuntimeRoutingResult> {
        let static_route = route_for_entity_type(entity_type, language)?;
        let mut embedders = static_route
            .embedders
            .iter()
            .copied()
            .map(RuntimeEmbedderId::Static)
            .collect::<BTreeSet<_>>();
        for record in &self.dynamic_records {
            if record.matches_route(&static_route.language, &static_route.entity_type) {
                embedders.insert(record.id.clone());
            }
        }
        Ok(RuntimeRoutingResult {
            registry_version: self.registry_version,
            language: static_route.language,
            entity_type: static_route.entity_type,
            embedders,
        })
    }
}

pub fn upper_triangle_len(embedder_count: usize) -> EmbedResult<usize> {
    if embedder_count > EmbedderId::content().len() + MAX_DYNAMIC_EMBEDDERS {
        return Err(EmbedError::invalid(
            "embedder_count",
            format!("embedder_count {embedder_count} exceeds supported dynamic panel cardinality"),
            "retire unused dynamic embedders before computing DDA pairwise features",
        ));
    }
    embedder_count
        .checked_mul(embedder_count.saturating_sub(1))
        .and_then(|value| value.checked_div(2))
        .ok_or_else(|| {
            EmbedError::invalid(
                "embedder_count",
                "upper-triangle cardinality overflowed usize",
                "reject the dynamic registry snapshot before DDA computation",
            )
        })
}

pub fn dda_signal_count_for_chunks(
    chunk_count: usize,
    embedder_count: usize,
    foundationality_term: usize,
) -> EmbedResult<usize> {
    let per_chunk = embedder_count
        .checked_add(upper_triangle_len(embedder_count)?)
        .and_then(|value| value.checked_add(foundationality_term))
        .ok_or_else(|| {
            EmbedError::invalid(
                "dda_signal_count",
                "per-chunk DDA identity overflowed usize",
                "reject the dynamic panel before materializing DDA signals",
            )
        })?;
    chunk_count.checked_mul(per_chunk).ok_or_else(|| {
        EmbedError::invalid(
            "dda_signal_count",
            "DDA identity overflowed usize for chunk count",
            "split the materialization into smaller shards",
        )
    })
}

fn validate_dynamic_id(sequence: u32, name: &str) -> EmbedResult<()> {
    if sequence == 0 {
        return Err(EmbedError::invalid(
            "RuntimeEmbedderId.sequence",
            "dynamic sequence must be non-zero",
            "allocate promotion sequence numbers from the registry high-water mark",
        ));
    }
    if name.is_empty() || name.len() > MAX_DYNAMIC_NAME_LEN {
        return Err(EmbedError::invalid(
            "RuntimeEmbedderId.name",
            format!("name length must be in [1, {MAX_DYNAMIC_NAME_LEN}]"),
            "use a short operator-friendly slug such as corpus_transe_v1",
        ));
    }
    let mut chars = name.chars();
    let first = chars.next().expect("checked non-empty dynamic name");
    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return Err(EmbedError::invalid(
            "RuntimeEmbedderId.name",
            "name must start with lowercase ASCII alnum",
            "use lowercase snake_case for dynamic embedder names",
        ));
    }
    if !name
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(EmbedError::invalid(
            "RuntimeEmbedderId.name",
            "name may only contain lowercase ASCII letters, digits, or underscore",
            "normalize display text before persisting the dynamic embedder id",
        ));
    }
    Ok(())
}

fn validate_route_list(field: &'static str, values: &[String]) -> EmbedResult<()> {
    if values.is_empty() {
        return Err(EmbedError::invalid(
            field,
            "route list must not be empty",
            "scope the dynamic embedder to at least one language/entity route or use *",
        ));
    }
    for value in values {
        validate_non_empty(field, value)?;
        if value.contains('\0') {
            return Err(EmbedError::invalid(
                field,
                "route values must not contain NUL bytes",
                "reject corrupt registry rows before constructing the runtime overlay",
            ));
        }
    }
    Ok(())
}

fn validate_non_empty(field: &'static str, value: &str) -> EmbedResult<()> {
    if value.trim().is_empty() {
        return Err(EmbedError::invalid(
            field,
            "value must not be empty",
            "persist explicit audit metadata instead of blank placeholders",
        ));
    }
    Ok(())
}

fn validate_sha256(field: &'static str, value: &str) -> EmbedResult<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(EmbedError::invalid(
            field,
            "sha256 must be 64 lowercase hex characters",
            "hash the promoted artifact and persist lowercase SHA-256",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use context_graph_core::memory::ast::{EntityType, Language};

    #[test]
    fn parses_dynamic_runtime_embedder_id() {
        let id = "edynamic:1:corpus_transe_v1"
            .parse::<RuntimeEmbedderId>()
            .unwrap();
        assert_eq!(id.slug(), "edynamic:1:corpus_transe_v1");
        assert!(id.is_dynamic());
        assert_eq!(
            "bad-name".parse::<RuntimeEmbedderId>().unwrap_err().code(),
            "MEJEPA_EMBED_INVALID_INPUT"
        );
    }

    #[test]
    fn runtime_overlay_adds_dynamic_route_without_mutating_old_snapshot() {
        let record = fixture_record();
        let old = RuntimeRoutingTable::new(1, Vec::new()).unwrap();
        let new = RuntimeRoutingTable::new(2, vec![record.clone()]).unwrap();

        let old_route = old
            .route_for_entity_type(EntityType::Function, Language::Python)
            .unwrap();
        let new_route = new
            .route_for_entity_type(EntityType::Function, Language::Python)
            .unwrap();

        assert!(!old_route.embedders.contains(&record.id));
        assert!(new_route.embedders.contains(&record.id));
        assert_eq!(
            new.pairwise_mi_upper_triangle_len(EmbedderId::content().len())
                .unwrap(),
            78
        );
    }

    #[test]
    fn rejects_duplicate_dynamic_rows() {
        let record = fixture_record();
        let err = RuntimeRoutingTable::new(2, vec![record.clone(), record])
            .unwrap_err()
            .code();
        assert_eq!(err, "MEJEPA_EMBED_INVALID_INPUT");
    }

    fn fixture_record() -> DynamicEmbedderRegistryRecord {
        DynamicEmbedderRegistryRecord {
            id: RuntimeEmbedderId::dynamic(1, "corpus_transe_v1").unwrap(),
            registry_version: 2,
            kind: DynamicEmbedderKind::Algorithmic,
            dimension: 128,
            route_languages: vec!["python".to_string()],
            route_entity_types: vec!["Function".to_string()],
            forward_artifact_path:
                "/zfs/archive/contextgraph/models/dynamic/edynamic_1_corpus_transe_v1/forward.so"
                    .to_string(),
            forward_artifact_sha256:
                "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
            required_vram_bytes: 64 * 1024 * 1024,
            active: true,
            promoted_at_unix_ms: 1_779_100_000_000,
        }
    }
}
