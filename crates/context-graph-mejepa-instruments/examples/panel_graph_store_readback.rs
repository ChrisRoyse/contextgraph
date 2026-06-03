use context_graph_mejepa_instruments::{
    ChunkEdge, InstrumentSlot, PanelBuilder, PanelGraph, PanelGraphDoctrine, PanelGraphEnvelope,
    PanelGraphNode, PanelKey, PanelProvenance, PanelStore, TimeStep, CF_MEJEPA_PANEL_GRAPHS,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::error::Error;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const FORMULA_VERSION: &str = "panelgraph_phase2_store_readback_v1";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PrototypeInput {
    attempt_id: String,
    time_step: String,
    provenance: PanelProvenance,
    dependency_subgraph_sha256: String,
    root_chunk_id: String,
    nodes: Vec<PrototypeNode>,
    edges: Vec<ChunkEdge>,
    source_artifacts: BTreeMap<String, String>,
    representative_row: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PrototypeNode {
    chunk_id: String,
    e9_vector_sha256: String,
    e9_vector: Vec<f32>,
}

#[derive(Debug, Serialize)]
struct NodeReadback {
    chunk_id: String,
    e9_vector_sha256: String,
    panel_hash: String,
    filled_slot: &'static str,
    filled_slot_dim: usize,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse()?;
    let text = fs::read_to_string(&args.input)?;
    let input: PrototypeInput = serde_json::from_str(&text)?;
    validate_no_mounted_drive_paths(&input)?;

    let key = PanelKey::new(&input.attempt_id, parse_time_step(&input.time_step)?)?;
    let mut nodes = Vec::with_capacity(input.nodes.len());
    let mut node_readbacks = Vec::with_capacity(input.nodes.len());
    for node in input.nodes {
        let observed = vector_sha256(&node.e9_vector);
        if observed != node.e9_vector_sha256 {
            return Err(format!(
                "e9 vector hash mismatch for {}: expected {}, observed {}",
                node.chunk_id, node.e9_vector_sha256, observed
            )
            .into());
        }
        let mut builder = PanelBuilder::new();
        builder.set_slot_with_health_check(InstrumentSlot::EProblem, &node.e9_vector)?;
        let panel = builder.build()?;
        let graph_node = PanelGraphNode::try_new(node.chunk_id.clone(), panel)?;
        node_readbacks.push(NodeReadback {
            chunk_id: node.chunk_id,
            e9_vector_sha256: observed,
            panel_hash: graph_node.panel_hash.clone(),
            filled_slot: InstrumentSlot::EProblem.slug(),
            filled_slot_dim: InstrumentSlot::EProblem.dim(),
        });
        nodes.push(graph_node);
    }
    nodes.sort_by(|left, right| left.chunk_id.cmp(&right.chunk_id));
    node_readbacks.sort_by(|left, right| left.chunk_id.cmp(&right.chunk_id));

    let mut edges = input.edges;
    edges.sort_by(|left, right| {
        (
            left.from.as_str(),
            left.to.as_str(),
            left.edge_kind.as_str(),
            left.edge_evidence_hash.as_str(),
        )
            .cmp(&(
                right.from.as_str(),
                right.to.as_str(),
                right.edge_kind.as_str(),
                right.edge_evidence_hash.as_str(),
            ))
    });

    let graph = PanelGraph::try_new(
        nodes,
        edges,
        input.dependency_subgraph_sha256,
        input.root_chunk_id,
        PanelGraphDoctrine::preserved(),
    )?;
    let envelope = PanelGraphEnvelope::try_new(graph, input.provenance)?;

    if args.output_root.exists() {
        return Err(format!("output root already exists: {}", args.output_root.display()).into());
    }
    fs::create_dir_all(&args.output_root)?;
    fs::create_dir_all(&args.store_path)?;

    let store = PanelStore::open(&args.store_path)?;
    store.put_panel_graph(&key, &envelope)?;
    store.flush()?;
    let readback = store
        .get_panel_graph(&key)?
        .ok_or("PanelGraph row missing after put_panel_graph")?;
    if readback != envelope {
        return Err("PanelGraph store readback did not match written envelope".into());
    }
    let graph_cf_count = store.count_cf(CF_MEJEPA_PANEL_GRAPHS)?;

    let envelope_path = args.output_root.join("panel_graph_envelope.json");
    write_json(&envelope_path, &envelope)?;

    let readback_summary = json!({
        "schema_version": 1,
        "artifact_kind": "panel_graph_store_readback",
        "formula_version": FORMULA_VERSION,
        "attempt_id": input.attempt_id,
        "time_step": input.time_step,
        "storage_key": key.storage_key(),
        "store_path": args.store_path,
        "cf_name": CF_MEJEPA_PANEL_GRAPHS,
        "graph_cf_count": graph_cf_count,
        "readback_matches_written_envelope": true,
        "graph_hash": envelope.graph_hash,
        "dependency_subgraph_sha256": envelope.graph.dependency_subgraph_sha256,
        "root_chunk_id": envelope.graph.root_chunk_id,
        "node_count": envelope.graph.nodes.len(),
        "edge_count": envelope.graph.edges.len(),
        "node_vector_source": "e9_forward_cache_vector_mapped_to_e_problem_slot_for_schema_fsv",
        "node_readbacks": node_readbacks,
        "doctrine": envelope.graph.doctrine,
        "source_artifacts": input.source_artifacts,
        "representative_row": input.representative_row,
    });
    let readback_path = args.output_root.join("panel_graph_store_readback.json");
    write_json(&readback_path, &readback_summary)?;

    let manifest_path = args.output_root.join("panel_graph_phase2_manifest.json");
    let manifest = json!({
        "schema_version": 1,
        "artifact_kind": "panel_graph_phase2_manifest",
        "formula_version": FORMULA_VERSION,
        "passes": true,
        "output_root": args.output_root,
        "store_path": args.store_path,
        "graph_hash": envelope.graph_hash,
        "node_count": envelope.graph.nodes.len(),
        "edge_count": envelope.graph.edges.len(),
        "cf_name": CF_MEJEPA_PANEL_GRAPHS,
        "graph_cf_count": graph_cf_count,
        "readback_matches_written_envelope": true,
        "envelope_path": envelope_path,
        "readback_path": readback_path,
        "doctrine": envelope.graph.doctrine,
    });
    write_json(&manifest_path, &manifest)?;

    let artifact_hash_path = args.output_root.join("artifact_hashes.json");
    let artifact_hashes = json!({
        "panel_graph_envelope.json": sha256_file(&envelope_path)?,
        "panel_graph_store_readback.json": sha256_file(&readback_path)?,
        "panel_graph_phase2_manifest.json": sha256_file(&manifest_path)?,
    });
    write_json(&artifact_hash_path, &artifact_hashes)?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "passes": true,
            "output_root": args.output_root,
            "store_path": args.store_path,
            "graph_hash": envelope.graph_hash,
            "node_count": envelope.graph.nodes.len(),
            "edge_count": envelope.graph.edges.len(),
            "graph_cf_count": graph_cf_count,
        }))?
    );
    Ok(())
}

#[derive(Debug)]
struct Args {
    input: PathBuf,
    store_path: PathBuf,
    output_root: PathBuf,
}

impl Args {
    fn parse() -> Result<Self, Box<dyn Error>> {
        let mut input = None;
        let mut store_path = None;
        let mut output_root = None;
        let mut iter = env::args().skip(1);
        while let Some(arg) = iter.next() {
            let value = iter
                .next()
                .ok_or_else(|| format!("missing value for argument {arg}"))?;
            match arg.as_str() {
                "--input" => input = Some(PathBuf::from(value)),
                "--store-path" => store_path = Some(PathBuf::from(value)),
                "--output-root" => output_root = Some(PathBuf::from(value)),
                _ => return Err(format!("unknown argument {arg}").into()),
            }
        }
        Ok(Self {
            input: input.ok_or("--input is required")?,
            store_path: store_path.ok_or("--store-path is required")?,
            output_root: output_root.ok_or("--output-root is required")?,
        })
    }
}

fn parse_time_step(value: &str) -> Result<TimeStep, Box<dyn Error>> {
    match value {
        "t0" | "T0" => Ok(TimeStep::T0),
        "t1" | "T1" => Ok(TimeStep::T1),
        "t2" | "T2" => Ok(TimeStep::T2),
        _ => Err(format!("unsupported time step {value:?}; expected t0/t1/t2").into()),
    }
}

fn validate_no_mounted_drive_paths(input: &PrototypeInput) -> Result<(), Box<dyn Error>> {
    for value in input.source_artifacts.values() {
        if value.contains("/mnt/c/") || value.contains("/mnt/d/") {
            return Err(
                format!("mounted-drive path forbidden in source_artifacts: {value}").into(),
            );
        }
    }
    Ok(())
}

fn vector_sha256(vector: &[f32]) -> String {
    let mut bytes = Vec::with_capacity(vector.len() * 4);
    for value in vector {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    format!("sha256:{}", lower_hex(&Sha256::digest(bytes)))
}

fn sha256_file(path: &Path) -> Result<String, Box<dyn Error>> {
    let bytes = fs::read(path)?;
    Ok(format!("sha256:{}", lower_hex(&Sha256::digest(bytes))))
}

fn lower_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut out, "{byte:02x}").expect("writing to String cannot fail");
    }
    out
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), Box<dyn Error>> {
    let mut file = fs::File::create(path)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}
