use crate::frozen_hook::hash_f32s;
use crate::panel_json::{validate_provenance, PanelProvenance};
use crate::{InstrumentError, InstrumentResult, Panel};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const PANEL_GRAPH_SCHEMA_VERSION: u8 = 1;
pub const MAX_PANEL_GRAPH_NODES: usize = 4_096;
pub const MAX_PANEL_GRAPH_EDGES: usize = 16_384;
const MAX_CHUNK_ID_BYTES: usize = 1_024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PanelGraphEnvelope {
    pub schema_version: u8,
    pub graph_hash: String,
    pub graph: PanelGraph,
    pub provenance: PanelProvenance,
}

impl PanelGraphEnvelope {
    pub fn try_new(graph: PanelGraph, provenance: PanelProvenance) -> InstrumentResult<Self> {
        graph.validate()?;
        validate_provenance(&provenance)?;
        let graph_hash = graph.canonical_hash()?;
        Ok(Self {
            schema_version: PANEL_GRAPH_SCHEMA_VERSION,
            graph_hash,
            graph,
            provenance,
        })
    }

    pub fn validate(&self) -> InstrumentResult<()> {
        if self.schema_version != PANEL_GRAPH_SCHEMA_VERSION {
            return Err(InstrumentError::invalid(
                "PanelGraphEnvelope.schema_version",
                format!(
                    "unsupported panel graph schema version {}; expected {}",
                    self.schema_version, PANEL_GRAPH_SCHEMA_VERSION
                ),
                "regenerate the PanelGraph row with the current schema; no migration fallback exists",
            ));
        }
        validate_provenance(&self.provenance)?;
        self.graph.validate()?;
        let actual = self.graph.canonical_hash()?;
        if actual != self.graph_hash {
            return Err(InstrumentError::invalid(
                "PanelGraphEnvelope.graph_hash",
                format!(
                    "graph hash mismatch: envelope has {}, graph hashes to {}",
                    self.graph_hash, actual
                ),
                "read the source graph again and reject the corrupted JSON",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PanelGraph {
    pub nodes: Vec<PanelGraphNode>,
    pub edges: Vec<ChunkEdge>,
    pub dependency_subgraph_sha256: String,
    pub root_chunk_id: String,
    pub doctrine: PanelGraphDoctrine,
}

impl PanelGraph {
    pub fn try_new(
        nodes: Vec<PanelGraphNode>,
        edges: Vec<ChunkEdge>,
        dependency_subgraph_sha256: impl Into<String>,
        root_chunk_id: impl Into<String>,
        doctrine: PanelGraphDoctrine,
    ) -> InstrumentResult<Self> {
        let graph = Self {
            nodes,
            edges,
            dependency_subgraph_sha256: dependency_subgraph_sha256.into(),
            root_chunk_id: root_chunk_id.into(),
            doctrine,
        };
        graph.validate()?;
        Ok(graph)
    }

    pub fn validate(&self) -> InstrumentResult<()> {
        if self.nodes.is_empty() {
            return Err(InstrumentError::invalid(
                "PanelGraph.nodes",
                "panel graph must contain at least one chunk node",
                "materialize one PanelGraph node for each touched AST/source chunk",
            ));
        }
        if self.nodes.len() > MAX_PANEL_GRAPH_NODES {
            return Err(InstrumentError::invalid(
                "PanelGraph.nodes",
                format!(
                    "panel graph has {} nodes; max is {}",
                    self.nodes.len(),
                    MAX_PANEL_GRAPH_NODES
                ),
                "split pathological patches before graph materialization",
            ));
        }
        if self.edges.len() > MAX_PANEL_GRAPH_EDGES {
            return Err(InstrumentError::invalid(
                "PanelGraph.edges",
                format!(
                    "panel graph has {} edges; max is {}",
                    self.edges.len(),
                    MAX_PANEL_GRAPH_EDGES
                ),
                "deduplicate dependency evidence before graph materialization",
            ));
        }
        validate_sha256_hex(
            "PanelGraph.dependency_subgraph_sha256",
            &self.dependency_subgraph_sha256,
        )?;
        validate_chunk_id("PanelGraph.root_chunk_id", &self.root_chunk_id)?;
        self.doctrine.validate()?;

        let mut node_ids = BTreeSet::new();
        let mut previous_node_id: Option<&str> = None;
        for node in &self.nodes {
            node.validate()?;
            if !node_ids.insert(node.chunk_id.clone()) {
                return Err(InstrumentError::invalid(
                    "PanelGraph.nodes",
                    format!("duplicate chunk node {:?}", node.chunk_id),
                    "deduplicate nodes by chunk_id before writing PanelGraph rows",
                ));
            }
            if let Some(previous) = previous_node_id {
                if previous >= node.chunk_id.as_str() {
                    return Err(InstrumentError::invalid(
                        "PanelGraph.nodes",
                        "nodes are not in canonical ascending chunk_id order",
                        "sort PanelGraph nodes by chunk_id before hashing or storage",
                    ));
                }
            }
            previous_node_id = Some(&node.chunk_id);
        }
        if !node_ids.contains(&self.root_chunk_id) {
            return Err(InstrumentError::invalid(
                "PanelGraph.root_chunk_id",
                format!(
                    "root chunk {:?} is not present in the graph node set",
                    self.root_chunk_id
                ),
                "write the root chunk as a node before writing the PanelGraph row",
            ));
        }

        let mut edge_keys = BTreeSet::new();
        let mut previous_edge_key: Option<String> = None;
        for edge in &self.edges {
            edge.validate()?;
            if !node_ids.contains(&edge.from) || !node_ids.contains(&edge.to) {
                return Err(InstrumentError::invalid(
                    "PanelGraph.edges",
                    format!(
                        "edge {:?}->{:?} references a chunk missing from nodes",
                        edge.from, edge.to
                    ),
                    "only write edges whose endpoints are present PanelGraph nodes",
                ));
            }
            let edge_key = edge.canonical_key();
            if !edge_keys.insert(edge_key.clone()) {
                return Err(InstrumentError::invalid(
                    "PanelGraph.edges",
                    format!("duplicate edge {edge_key}"),
                    "deduplicate edges by from/to/kind/evidence hash before storage",
                ));
            }
            if let Some(previous) = &previous_edge_key {
                if previous >= &edge_key {
                    return Err(InstrumentError::invalid(
                        "PanelGraph.edges",
                        "edges are not in canonical ascending edge-key order",
                        "sort PanelGraph edges by from/to/kind/evidence hash before hashing or storage",
                    ));
                }
            }
            previous_edge_key = Some(edge_key);
        }
        Ok(())
    }

    pub fn canonical_hash(&self) -> InstrumentResult<String> {
        self.validate()?;
        let mut hasher = Sha256::new();
        hasher.update(b"MEJEPA_PANEL_GRAPH_V1\0");
        hasher.update(self.dependency_subgraph_sha256.as_bytes());
        hasher.update(b"\0root\0");
        hasher.update(self.root_chunk_id.as_bytes());
        hasher.update(b"\0doctrine\0");
        hasher.update(self.doctrine.canonical_bytes());
        for node in &self.nodes {
            hasher.update(b"\0node\0");
            hasher.update(node.chunk_id.as_bytes());
            hasher.update(b"\0panel_hash\0");
            hasher.update(node.panel_hash.as_bytes());
            hasher.update(b"\0filled_mask\0");
            hasher.update(node.panel.filled_mask().to_le_bytes());
        }
        for edge in &self.edges {
            hasher.update(b"\0edge\0");
            hasher.update(edge.canonical_key().as_bytes());
        }
        Ok(lower_hex(&hasher.finalize()))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PanelGraphNode {
    pub chunk_id: String,
    pub panel_hash: String,
    pub panel: Panel,
}

impl PanelGraphNode {
    pub fn try_new(chunk_id: impl Into<String>, panel: Panel) -> InstrumentResult<Self> {
        let node = Self {
            chunk_id: chunk_id.into(),
            panel_hash: hash_f32s(panel.data()),
            panel,
        };
        node.validate()?;
        Ok(node)
    }

    pub fn validate(&self) -> InstrumentResult<()> {
        validate_chunk_id("PanelGraphNode.chunk_id", &self.chunk_id)?;
        validate_sha256_hex("PanelGraphNode.panel_hash", &self.panel_hash)?;
        let actual = hash_f32s(self.panel.data());
        if actual != self.panel_hash {
            return Err(InstrumentError::invalid(
                "PanelGraphNode.panel_hash",
                format!(
                    "node panel hash mismatch: node has {}, panel hashes to {}",
                    self.panel_hash, actual
                ),
                "read the source panel again and reject the corrupted graph node",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Imports,
    Calls,
    CoChanged,
    TestsCovers,
}

impl EdgeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Imports => "imports",
            Self::Calls => "calls",
            Self::CoChanged => "co_changed",
            Self::TestsCovers => "tests_covers",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChunkEdge {
    pub from: String,
    pub to: String,
    pub edge_kind: EdgeKind,
    pub edge_evidence_hash: String,
}

impl ChunkEdge {
    pub fn try_new(
        from: impl Into<String>,
        to: impl Into<String>,
        edge_kind: EdgeKind,
        edge_evidence_hash: impl Into<String>,
    ) -> InstrumentResult<Self> {
        let edge = Self {
            from: from.into(),
            to: to.into(),
            edge_kind,
            edge_evidence_hash: edge_evidence_hash.into(),
        };
        edge.validate()?;
        Ok(edge)
    }

    pub fn validate(&self) -> InstrumentResult<()> {
        validate_chunk_id("ChunkEdge.from", &self.from)?;
        validate_chunk_id("ChunkEdge.to", &self.to)?;
        if self.from == self.to {
            return Err(InstrumentError::invalid(
                "ChunkEdge",
                format!(
                    "self-edge {:?}->{:?} is not a cross-chunk dependency",
                    self.from, self.to
                ),
                "write only cross-chunk dependency edges into PanelGraph rows",
            ));
        }
        validate_sha256_hex("ChunkEdge.edge_evidence_hash", &self.edge_evidence_hash)?;
        Ok(())
    }

    fn canonical_key(&self) -> String {
        format!(
            "{}\0{}\0{}\0{}",
            self.from,
            self.to,
            self.edge_kind.as_str(),
            self.edge_evidence_hash
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PanelGraphDoctrine {
    pub slot_identity_preserved: bool,
    pub anti_compensation_preserved: bool,
    pub flat_vector_concat_used: bool,
    pub target_labels_used_as_live_inputs: bool,
    pub row_level_scores_cloned_to_chunks: bool,
    pub root_verdict_contract_preserved: bool,
}

impl PanelGraphDoctrine {
    pub const fn preserved() -> Self {
        Self {
            slot_identity_preserved: true,
            anti_compensation_preserved: true,
            flat_vector_concat_used: false,
            target_labels_used_as_live_inputs: false,
            row_level_scores_cloned_to_chunks: false,
            root_verdict_contract_preserved: true,
        }
    }

    pub fn validate(&self) -> InstrumentResult<()> {
        if !self.slot_identity_preserved {
            return doctrine_err(
                "PanelGraphDoctrine.slot_identity_preserved",
                "slot identity must remain preserved at each graph node",
            );
        }
        if !self.anti_compensation_preserved {
            return doctrine_err(
                "PanelGraphDoctrine.anti_compensation_preserved",
                "anti-compensation must remain preserved across graph attention",
            );
        }
        if self.flat_vector_concat_used {
            return doctrine_err(
                "PanelGraphDoctrine.flat_vector_concat_used",
                "flat-vector concatenation is forbidden for PanelGraph rows",
            );
        }
        if self.target_labels_used_as_live_inputs {
            return doctrine_err(
                "PanelGraphDoctrine.target_labels_used_as_live_inputs",
                "target labels must not be live PanelGraph inputs",
            );
        }
        if self.row_level_scores_cloned_to_chunks {
            return doctrine_err(
                "PanelGraphDoctrine.row_level_scores_cloned_to_chunks",
                "row-level scores must not be cloned into chunk nodes",
            );
        }
        if !self.root_verdict_contract_preserved {
            return doctrine_err(
                "PanelGraphDoctrine.root_verdict_contract_preserved",
                "PanelGraph must still produce a root Pass/Fail verdict",
            );
        }
        Ok(())
    }

    fn canonical_bytes(&self) -> &'static [u8] {
        b"slot_identity=1;anti_compensation=1;flat_concat=0;target_labels_live=0;row_scores_cloned=0;root_verdict=1"
    }
}

fn doctrine_err(field: &'static str, message: &'static str) -> InstrumentResult<()> {
    Err(InstrumentError::frozen_violation(
        field,
        message,
        "preserve the Tier-4 doctrine invariant before persisting PanelGraph rows",
    ))
}

fn validate_chunk_id(field: &'static str, value: &str) -> InstrumentResult<()> {
    if value.trim().is_empty() {
        return Err(InstrumentError::invalid(
            field,
            "chunk id is empty",
            "write a stable AST/source chunk identifier",
        ));
    }
    if value.len() > MAX_CHUNK_ID_BYTES {
        return Err(InstrumentError::invalid(
            field,
            format!(
                "chunk id has {} bytes; max is {}",
                value.len(),
                MAX_CHUNK_ID_BYTES
            ),
            "hash or compact pathological chunk identifiers before storage",
        ));
    }
    if value.contains('\0') {
        return Err(InstrumentError::invalid(
            field,
            "chunk id contains NUL",
            "use a NUL-free UTF-8 chunk identifier",
        ));
    }
    Ok(())
}

fn validate_sha256_hex(field: &'static str, value: &str) -> InstrumentResult<()> {
    if value.len() != 64 || !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(InstrumentError::invalid(
            field,
            format!("expected 64 lowercase hex chars, got {value:?}"),
            "record a real SHA-256 digest in lowercase hex",
        ));
    }
    if value.bytes().any(|b| b.is_ascii_uppercase()) {
        return Err(InstrumentError::invalid(
            field,
            format!("SHA-256 digest must be lowercase hex, got {value:?}"),
            "normalize digest bytes to lowercase hex at the writer boundary",
        ));
    }
    Ok(())
}

fn lower_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write;
        write!(&mut out, "{byte:02x}").expect("writing to String cannot fail");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InstrumentSlot, PanelBuilder};
    use std::collections::BTreeMap;

    fn panel(seed: f32) -> Panel {
        let mut builder = PanelBuilder::new();
        builder
            .set_slot(
                InstrumentSlot::EOracle,
                &vec![seed; InstrumentSlot::EOracle.dim()],
            )
            .unwrap();
        builder.build().unwrap()
    }

    fn provenance() -> PanelProvenance {
        PanelProvenance {
            code_version: "test-sha".into(),
            embedder_versions: BTreeMap::from([("e_oracle".into(), "deterministic-v1".into())]),
            corpus_sha: "a".repeat(64),
            frozen_at_unix_ms: 1,
            source_sha256: "b".repeat(64),
        }
    }

    fn hash(value: &str) -> String {
        lower_hex(&Sha256::digest(value.as_bytes()))
    }

    fn graph() -> PanelGraph {
        let nodes = vec![
            PanelGraphNode::try_new("pkg/a.py::chunk:1", panel(0.25)).unwrap(),
            PanelGraphNode::try_new("pkg/b.py::chunk:1", panel(0.75)).unwrap(),
        ];
        let edges = vec![ChunkEdge::try_new(
            "pkg/a.py::chunk:1",
            "pkg/b.py::chunk:1",
            EdgeKind::Calls,
            hash("call-edge"),
        )
        .unwrap()];
        PanelGraph::try_new(
            nodes,
            edges,
            hash("dependency-subgraph"),
            "pkg/a.py::chunk:1",
            PanelGraphDoctrine::preserved(),
        )
        .unwrap()
    }

    #[test]
    fn panel_graph_envelope_validates_canonical_hash() {
        let envelope = PanelGraphEnvelope::try_new(graph(), provenance()).unwrap();
        envelope.validate().unwrap();
        let mut bad = envelope.clone();
        bad.graph_hash = "c".repeat(64);
        assert_eq!(
            bad.validate().unwrap_err().code(),
            "MEJEPA_INSTRUMENTS_INVALID_INPUT"
        );
    }

    #[test]
    fn panel_graph_rejects_duplicate_nodes_and_missing_root() {
        let node = PanelGraphNode::try_new("pkg/a.py::chunk:1", panel(0.25)).unwrap();
        let err = PanelGraph::try_new(
            vec![node.clone(), node],
            Vec::new(),
            hash("dependency-subgraph"),
            "pkg/a.py::chunk:1",
            PanelGraphDoctrine::preserved(),
        )
        .unwrap_err();
        assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");

        let err = PanelGraph::try_new(
            vec![PanelGraphNode::try_new("pkg/a.py::chunk:1", panel(0.25)).unwrap()],
            Vec::new(),
            hash("dependency-subgraph"),
            "pkg/missing.py::chunk:1",
            PanelGraphDoctrine::preserved(),
        )
        .unwrap_err();
        assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");
    }

    #[test]
    fn panel_graph_rejects_flat_vector_policy_violation() {
        let mut doctrine = PanelGraphDoctrine::preserved();
        doctrine.flat_vector_concat_used = true;
        let err = PanelGraph::try_new(
            vec![PanelGraphNode::try_new("pkg/a.py::chunk:1", panel(0.25)).unwrap()],
            Vec::new(),
            hash("dependency-subgraph"),
            "pkg/a.py::chunk:1",
            doctrine,
        )
        .unwrap_err();
        assert_eq!(err.code(), "MEJEPA_INSTR_FROZEN_VIOLATION");
    }

    #[test]
    fn panel_graph_requires_canonical_node_and_edge_order() {
        let nodes = vec![
            PanelGraphNode::try_new("pkg/b.py::chunk:1", panel(0.75)).unwrap(),
            PanelGraphNode::try_new("pkg/a.py::chunk:1", panel(0.25)).unwrap(),
        ];
        let err = PanelGraph::try_new(
            nodes,
            Vec::new(),
            hash("dependency-subgraph"),
            "pkg/a.py::chunk:1",
            PanelGraphDoctrine::preserved(),
        )
        .unwrap_err();
        assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");

        let nodes = vec![
            PanelGraphNode::try_new("pkg/a.py::chunk:1", panel(0.25)).unwrap(),
            PanelGraphNode::try_new("pkg/b.py::chunk:1", panel(0.75)).unwrap(),
        ];
        let edges = vec![
            ChunkEdge::try_new(
                "pkg/b.py::chunk:1",
                "pkg/a.py::chunk:1",
                EdgeKind::Calls,
                hash("z-edge"),
            )
            .unwrap(),
            ChunkEdge::try_new(
                "pkg/a.py::chunk:1",
                "pkg/b.py::chunk:1",
                EdgeKind::Calls,
                hash("a-edge"),
            )
            .unwrap(),
        ];
        let err = PanelGraph::try_new(
            nodes,
            edges,
            hash("dependency-subgraph"),
            "pkg/a.py::chunk:1",
            PanelGraphDoctrine::preserved(),
        )
        .unwrap_err();
        assert_eq!(err.code(), "MEJEPA_INSTRUMENTS_INVALID_INPUT");
    }
}
