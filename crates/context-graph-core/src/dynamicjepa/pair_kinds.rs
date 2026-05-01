use crate::dynamicjepa::error::{DynamicJepaError, DynamicJepaResult};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
#[repr(transparent)]
pub struct PairKindBitset(pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PairKindName {
    Cosine,
    RankDisagreement,
    ModalityContradiction,
    SparseDenseMismatch,
    TemporalSurprise,
    CausalDirectionDisagreement,
    SafetyProximity,
}

impl PairKindBitset {
    pub const COSINE_AGREEMENT: u8 = 0b0000_0001;
    pub const RANK_DISAGREEMENT: u8 = 0b0000_0010;
    pub const MODALITY_CONTRADICTION: u8 = 0b0000_0100;
    pub const SPARSE_DENSE_MISMATCH: u8 = 0b0000_1000;
    pub const TEMPORAL_SURPRISE: u8 = 0b0001_0000;
    pub const CAUSAL_DIRECTION: u8 = 0b0010_0000;
    pub const SAFETY_PROXIMITY: u8 = 0b0100_0000;
    pub const ALL_KIND_BITS: u8 = 0b0111_1111;

    pub fn has(self, kind: u8) -> bool {
        (self.0 & kind) != 0
    }

    pub fn set(&mut self, kind: u8) {
        self.0 |= kind;
    }

    pub fn clear(&mut self, kind: u8) {
        self.0 &= !kind;
    }

    pub fn iter_kinds(self) -> impl Iterator<Item = &'static str> {
        let mut out = Vec::new();
        if self.has(Self::COSINE_AGREEMENT) {
            out.push("cosine_agreement");
        }
        if self.has(Self::RANK_DISAGREEMENT) {
            out.push("rank_disagreement");
        }
        if self.has(Self::MODALITY_CONTRADICTION) {
            out.push("modality_contradiction");
        }
        if self.has(Self::SPARSE_DENSE_MISMATCH) {
            out.push("sparse_dense_mismatch");
        }
        if self.has(Self::TEMPORAL_SURPRISE) {
            out.push("temporal_surprise");
        }
        if self.has(Self::CAUSAL_DIRECTION) {
            out.push("causal_direction_disagreement");
        }
        if self.has(Self::SAFETY_PROXIMITY) {
            out.push("safety_proximity");
        }
        out.into_iter()
    }

    pub fn validate(self) -> DynamicJepaResult<()> {
        if self.0 & !Self::ALL_KIND_BITS != 0 {
            return Err(DynamicJepaError::validation(
                "PairKindBitset",
                format!(
                    "unknown pair-kind bits set: 0b{:08b}",
                    self.0 & !Self::ALL_KIND_BITS
                ),
                "write only the declared PairKindBitset constants",
            ));
        }
        if !self.has(Self::COSINE_AGREEMENT) {
            return Err(DynamicJepaError::validation(
                "PairKindBitset",
                "cosine_agreement bit must always be emitted",
                "set PairKindBitset::COSINE_AGREEMENT before persisting pairwise readings",
            ));
        }
        Ok(())
    }
}

impl PairKindName {
    pub fn as_toml_str(self) -> &'static str {
        match self {
            Self::Cosine => "cosine",
            Self::RankDisagreement => "rank_disagreement",
            Self::ModalityContradiction => "modality_contradiction",
            Self::SparseDenseMismatch => "sparse_dense_mismatch",
            Self::TemporalSurprise => "temporal_surprise",
            Self::CausalDirectionDisagreement => "causal_direction_disagreement",
            Self::SafetyProximity => "safety_proximity",
        }
    }

    pub fn bit(self) -> u8 {
        match self {
            Self::Cosine => PairKindBitset::COSINE_AGREEMENT,
            Self::RankDisagreement => PairKindBitset::RANK_DISAGREEMENT,
            Self::ModalityContradiction => PairKindBitset::MODALITY_CONTRADICTION,
            Self::SparseDenseMismatch => PairKindBitset::SPARSE_DENSE_MISMATCH,
            Self::TemporalSurprise => PairKindBitset::TEMPORAL_SURPRISE,
            Self::CausalDirectionDisagreement => PairKindBitset::CAUSAL_DIRECTION,
            Self::SafetyProximity => PairKindBitset::SAFETY_PROXIMITY,
        }
    }

    pub fn parse(value: &str) -> DynamicJepaResult<Self> {
        match value {
            "cosine" => Ok(Self::Cosine),
            "rank_disagreement" => Ok(Self::RankDisagreement),
            "modality_contradiction" => Ok(Self::ModalityContradiction),
            "sparse_dense_mismatch" => Ok(Self::SparseDenseMismatch),
            "temporal_surprise" => Ok(Self::TemporalSurprise),
            "causal_direction_disagreement" => Ok(Self::CausalDirectionDisagreement),
            "safety_proximity" => Ok(Self::SafetyProximity),
            other => Err(DynamicJepaError::validation(
                "InstrumentSpec.pair_kinds",
                format!("unsupported pair kind {other:?}"),
                "use one of: cosine, rank_disagreement, modality_contradiction, sparse_dense_mismatch, temporal_surprise, causal_direction_disagreement, safety_proximity",
            )),
        }
    }
}

impl Serialize for PairKindName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_toml_str())
    }
}

impl<'de> Deserialize<'de> for PairKindName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::parse(&value).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for PairKindName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_toml_str())
    }
}
