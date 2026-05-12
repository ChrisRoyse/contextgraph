use std::collections::BTreeMap;
use std::time::SystemTime;

pub use context_graph_mejepa_embedders::EmbedderId;
use serde::{Deserialize, Serialize};

use crate::error::TctError;
use crate::shrinkage::ShrinkageOrigin;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationCategory {
    KnownGood,
    SubtleFlip,
    OffByOne,
    SwapVariable,
    DeleteTestCall,
    WrongFile,
    OverEngineer,
    CompileError,
}

impl MutationCategory {
    pub const fn all() -> [Self; 8] {
        [
            Self::KnownGood,
            Self::SubtleFlip,
            Self::OffByOne,
            Self::SwapVariable,
            Self::DeleteTestCall,
            Self::WrongFile,
            Self::OverEngineer,
            Self::CompileError,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OracleOutcome {
    Pass,
    Fail,
    OutOfDistribution,
    Abstain,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    Rust,
    Python,
    Javascript,
    Typescript,
    Go,
    Java,
    C,
    Cpp,
    CSharp,
    Ruby,
    Php,
}

impl Language {
    pub const fn all() -> [Self; 11] {
        [
            Self::Rust,
            Self::Python,
            Self::Javascript,
            Self::Typescript,
            Self::Go,
            Self::Java,
            Self::C,
            Self::Cpp,
            Self::CSharp,
            Self::Ruby,
            Self::Php,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityType {
    Function,
    Method,
    Class,
    Struct,
    Enum,
    TraitOrInterface,
    Impl,
    Module,
    Namespace,
    TestFunction,
    Import,
    CommentBlock,
    Docstring,
}

impl EntityType {
    pub const fn all() -> [Self; 13] {
        [
            Self::Function,
            Self::Method,
            Self::Class,
            Self::Struct,
            Self::Enum,
            Self::TraitOrInterface,
            Self::Impl,
            Self::Module,
            Self::Namespace,
            Self::TestFunction,
            Self::Import,
            Self::CommentBlock,
            Self::Docstring,
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChunkId {
    pub file_sha: [u8; 32],
    pub language: Language,
    pub entity_type: EntityType,
    pub line_start: u32,
    pub line_end: u32,
    pub parent_chain_hash: [u8; 16],
}

impl ChunkId {
    pub fn try_new(
        file_sha: [u8; 32],
        language: Language,
        entity_type: EntityType,
        line_start: u32,
        line_end: u32,
        parent_chain_hash: [u8; 16],
    ) -> Result<Self, TctError> {
        if line_end < line_start {
            return Err(TctError::invalid(
                "ChunkId.line_end",
                format!("line_end {line_end} is before line_start {line_start}"),
            ));
        }
        Ok(Self {
            file_sha,
            language,
            entity_type,
            line_start,
            line_end,
            parent_chain_hash,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GtauViolation {
    pub embedder: EmbedderId,
    pub observed_cos: f32,
    pub threshold: f32,
    pub margin: f32,
    pub violating_chunk: Option<ChunkId>,
    pub centroid_origin: ShrinkageOrigin,
}

impl GtauViolation {
    pub fn try_new(
        embedder: EmbedderId,
        observed_cos: f32,
        threshold: f32,
        violating_chunk: Option<ChunkId>,
        centroid_origin: ShrinkageOrigin,
    ) -> Result<Self, TctError> {
        validate_cos("GtauViolation.observed_cos", observed_cos)?;
        validate_cos("GtauViolation.threshold", threshold)?;
        let margin = observed_cos - threshold;
        if !margin.is_finite() {
            return Err(TctError::nan(
                "GtauViolation.margin",
                format!("margin is non-finite for {embedder}"),
            ));
        }
        Ok(Self {
            embedder,
            observed_cos,
            threshold,
            margin,
            violating_chunk,
            centroid_origin,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GtauOutput {
    pub gtau_satisfied: bool,
    pub violations: Vec<GtauViolation>,
    pub centroid_origin: BTreeMap<EmbedderId, ShrinkageOrigin>,
    pub elapsed_ms: f32,
    pub evaluated_embedder_count: usize,
    pub min_margin: f32,
}

impl GtauOutput {
    pub fn try_new(
        violations: Vec<GtauViolation>,
        centroid_origin: BTreeMap<EmbedderId, ShrinkageOrigin>,
        elapsed_ms: f32,
        evaluated_embedder_count: usize,
        min_margin: f32,
    ) -> Result<Self, TctError> {
        if !elapsed_ms.is_finite() || elapsed_ms < 0.0 {
            return Err(TctError::nan(
                "GtauOutput.elapsed_ms",
                format!("elapsed_ms must be finite and non-negative, got {elapsed_ms}"),
            ));
        }
        if evaluated_embedder_count != EmbedderId::all().len() {
            return Err(TctError::dim(
                EmbedderId::all().len(),
                evaluated_embedder_count,
                "GtauOutput must evaluate all embedders",
            ));
        }
        if centroid_origin.len() != EmbedderId::all().len() {
            return Err(TctError::dim(
                EmbedderId::all().len(),
                centroid_origin.len(),
                "GtauOutput centroid_origin coverage",
            ));
        }
        if !min_margin.is_finite() {
            return Err(TctError::nan(
                "GtauOutput.min_margin",
                format!("min_margin is {min_margin}"),
            ));
        }
        Ok(Self {
            gtau_satisfied: violations.is_empty(),
            violations,
            centroid_origin,
            elapsed_ms,
            evaluated_embedder_count,
            min_margin,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CorpusProvenance {
    pub corpus_sha: [u8; 32],
    pub embedder_versions: BTreeMap<EmbedderId, [u8; 32]>,
    pub frozen_at: SystemTime,
    pub code_version: String,
}

impl CorpusProvenance {
    pub fn try_new(
        corpus_sha: [u8; 32],
        embedder_versions: BTreeMap<EmbedderId, [u8; 32]>,
        frozen_at: SystemTime,
        code_version: String,
    ) -> Result<Self, TctError> {
        if embedder_versions.len() != EmbedderId::all().len() {
            return Err(TctError::dim(
                EmbedderId::all().len(),
                embedder_versions.len(),
                "CorpusProvenance.embedder_versions must cover E1-E21",
            ));
        }
        for embedder in EmbedderId::all() {
            if !embedder_versions.contains_key(&embedder) {
                return Err(TctError::invalid(
                    "CorpusProvenance.embedder_versions",
                    format!("missing digest for {embedder}"),
                ));
            }
        }
        validate_code_version(&code_version)?;
        Ok(Self {
            corpus_sha,
            embedder_versions,
            frozen_at,
            code_version,
        })
    }
}

pub(crate) fn validate_code_version(value: &str) -> Result<(), TctError> {
    if value.len() != 40
        || !value.bytes().all(|b| b.is_ascii_hexdigit())
        || value.bytes().any(|b| b.is_ascii_uppercase())
    {
        return Err(TctError::invalid(
            "code_version",
            format!("expected 40 lowercase hex chars, got {value:?}"),
        ));
    }
    Ok(())
}

pub(crate) fn validate_cos(field: &str, value: f32) -> Result<(), TctError> {
    if !value.is_finite() || !(-1.0..=1.0).contains(&value) {
        return Err(TctError::nan(
            field,
            format!("expected finite cosine in [-1, 1], got {value}"),
        ));
    }
    Ok(())
}
