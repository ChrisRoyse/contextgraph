use crate::error::{EmbedError, EmbedResult};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbedderKind {
    ContentPretrained,
    ContentDeterministic,
    LearnerState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbedderId {
    E1,
    E2,
    E3,
    E4,
    E5,
    E6,
    E7,
    E8,
    E9,
    E10,
    E11,
    E12,
    E13,
    E14,
    E15,
    E16,
    E17,
    E18,
    E19,
    E20,
    E21,
}

impl EmbedderId {
    pub const fn all() -> [Self; 21] {
        [
            Self::E1,
            Self::E2,
            Self::E3,
            Self::E4,
            Self::E5,
            Self::E6,
            Self::E7,
            Self::E8,
            Self::E9,
            Self::E10,
            Self::E11,
            Self::E12,
            Self::E13,
            Self::E14,
            Self::E15,
            Self::E16,
            Self::E17,
            Self::E18,
            Self::E19,
            Self::E20,
            Self::E21,
        ]
    }

    pub const fn required_registrations() -> [Self; 12] {
        Self::content()
    }

    pub const fn all_non_retired_registrations() -> [Self; 19] {
        [
            Self::E1,
            Self::E2,
            Self::E3,
            Self::E4,
            Self::E6,
            Self::E7,
            Self::E8,
            Self::E9,
            Self::E10,
            Self::E12,
            Self::E13,
            Self::E14,
            Self::E15,
            Self::E16,
            Self::E17,
            Self::E18,
            Self::E19,
            Self::E20,
            Self::E21,
        ]
    }

    pub const fn content() -> [Self; 12] {
        [
            Self::E1,
            Self::E2,
            Self::E3,
            Self::E4,
            Self::E6,
            Self::E7,
            Self::E8,
            Self::E9,
            Self::E10,
            Self::E12,
            Self::E13,
            Self::E14,
        ]
    }

    pub const fn is_retired(self) -> bool {
        matches!(self, Self::E5 | Self::E11)
    }

    pub const fn learner_state() -> [Self; 7] {
        [
            Self::E15,
            Self::E16,
            Self::E17,
            Self::E18,
            Self::E19,
            Self::E20,
            Self::E21,
        ]
    }

    pub const fn slug(self) -> &'static str {
        match self {
            Self::E1 => "e1",
            Self::E2 => "e2",
            Self::E3 => "e3",
            Self::E4 => "e4",
            Self::E5 => "e5",
            Self::E6 => "e6",
            Self::E7 => "e7",
            Self::E8 => "e8",
            Self::E9 => "e9",
            Self::E10 => "e10",
            Self::E11 => "e11",
            Self::E12 => "e12",
            Self::E13 => "e13",
            Self::E14 => "e14",
            Self::E15 => "e15",
            Self::E16 => "e16",
            Self::E17 => "e17",
            Self::E18 => "e18",
            Self::E19 => "e19",
            Self::E20 => "e20",
            Self::E21 => "e21",
        }
    }

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::E1 => "Semantic",
            Self::E2 => "TemporalRecent",
            Self::E3 => "TemporalPeriodic",
            Self::E4 => "TemporalPositional",
            Self::E5 => "Causal",
            Self::E6 => "Sparse",
            Self::E7 => "Code",
            Self::E8 => "Graph",
            Self::E9 => "HDC",
            Self::E10 => "Contextual",
            Self::E11 => "Kepler",
            Self::E12 => "LateInteraction",
            Self::E13 => "SpladeV3",
            Self::E14 => "BgeM3Dense",
            Self::E15 => "AffectSpeech",
            Self::E16 => "AffectFace",
            Self::E17 => "AffectText",
            Self::E18 => "PPG",
            Self::E19 => "EDA",
            Self::E20 => "EEG",
            Self::E21 => "EegArtifactRobust",
        }
    }

    pub const fn kind(self) -> EmbedderKind {
        match self {
            Self::E2 | Self::E3 | Self::E4 | Self::E9 => EmbedderKind::ContentDeterministic,
            Self::E15 | Self::E16 | Self::E17 | Self::E18 | Self::E19 | Self::E20 | Self::E21 => {
                EmbedderKind::LearnerState
            }
            _ => EmbedderKind::ContentPretrained,
        }
    }

    pub const fn dimension(self) -> usize {
        match self {
            Self::E1 => 1024,
            Self::E2 | Self::E3 | Self::E4 => 512,
            Self::E5 => 768,
            Self::E6 | Self::E7 | Self::E13 => 1536,
            Self::E8 | Self::E9 | Self::E14 | Self::E15 => 1024,
            Self::E10 | Self::E11 | Self::E20 | Self::E21 => 768,
            Self::E12 => 128,
            Self::E16 | Self::E18 => 512,
            Self::E17 => 384,
            Self::E19 => 64,
        }
    }

    pub const fn default_model_dir(self) -> &'static str {
        match self {
            Self::E1 => "semantic",
            Self::E2 | Self::E3 | Self::E4 => "temporal",
            Self::E5 => "causal",
            Self::E6 => "sparse",
            Self::E7 => "code-1536",
            Self::E8 => "graph",
            Self::E9 => "hdc",
            Self::E10 => "contextual",
            Self::E11 => "kepler",
            Self::E12 => "late-interaction",
            Self::E13 => "splade-v3",
            Self::E14 => "bge-m3-dense",
            Self::E15 => "learner-state/affect-speech",
            Self::E16 => "learner-state/affect-face",
            Self::E17 => "learner-state/affect-text",
            Self::E18 => "learner-state/ppg",
            Self::E19 => "learner-state/eda",
            Self::E20 => "learner-state/eeg",
            Self::E21 => "learner-state/eeg-robust",
        }
    }

    pub const fn default_repo(self) -> Option<&'static str> {
        match self {
            Self::E1 => Some("intfloat/e5-large-v2"),
            Self::E5 => Some("nomic-ai/nomic-embed-text-v1.5"),
            Self::E6 => Some("naver/splade-cocondenser-ensembledistil"),
            Self::E7 => Some("Qodo/Qodo-Embed-1-1.5B"),
            Self::E8 => Some("intfloat/e5-large-v2"),
            Self::E10 => Some("intfloat/e5-base-v2"),
            Self::E11 => Some("THU-KEG/KEPLER-Wiki5M-KE"),
            Self::E12 => Some("colbert-ir/colbertv2.0"),
            Self::E13 => Some("prithivida/Splade_PP_en_v1"),
            Self::E14 => Some("BAAI/bge-m3"),
            Self::E15 => Some("msp-dim/wav2vec2-large"),
            Self::E16 => Some("OpenFace-3.0+MTL-local"),
            Self::E17 => Some("MiniLM-L6-v2+EmoBank-local"),
            Self::E18 => Some("PaPaGei/ppg-foundation"),
            Self::E19 => Some("WESAD/eda-stress-head"),
            Self::E20 => Some("LaBraM/eeg-foundation"),
            Self::E21 => Some("EEGPT/eeg-artifact-robust"),
            Self::E2 | Self::E3 | Self::E4 | Self::E9 => None,
        }
    }

    pub fn parse(value: &str) -> EmbedResult<Self> {
        value.parse()
    }
}

impl fmt::Display for EmbedderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.slug())
    }
}

impl FromStr for EmbedderId {
    type Err = EmbedError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().replace(['_', '-'], "").as_str() {
            "e1" | "semantic" => Ok(Self::E1),
            "e2" | "temporalrecent" => Ok(Self::E2),
            "e3" | "temporalperiodic" => Ok(Self::E3),
            "e4" | "temporalpositional" => Ok(Self::E4),
            "e5" | "causal" => Ok(Self::E5),
            "e6" | "sparse" => Ok(Self::E6),
            "e7" | "code" => Ok(Self::E7),
            "e8" | "graph" => Ok(Self::E8),
            "e9" | "hdc" => Ok(Self::E9),
            "e10" | "contextual" => Ok(Self::E10),
            "e11" | "kepler" => Ok(Self::E11),
            "e12" | "lateinteraction" => Ok(Self::E12),
            "e13" | "spladev3" | "splade" => Ok(Self::E13),
            "e14" | "bgem3dense" | "bgem3" => Ok(Self::E14),
            "e15" | "affectspeech" => Ok(Self::E15),
            "e16" | "affectface" => Ok(Self::E16),
            "e17" | "affecttext" => Ok(Self::E17),
            "e18" | "ppg" => Ok(Self::E18),
            "e19" | "eda" => Ok(Self::E19),
            "e20" | "eeg" => Ok(Self::E20),
            "e21" | "eegartifactrobust" => Ok(Self::E21),
            _ => Err(EmbedError::invalid(
                "EmbedderId",
                format!("unknown embedder id {value:?}"),
                "use canonical ids e1 through e21",
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_cover_all_phase1b_slots() {
        assert_eq!(EmbedderId::all().len(), 21);
        assert_eq!(EmbedderId::required_registrations().len(), 12);
        assert_eq!(EmbedderId::all_non_retired_registrations().len(), 19);
        assert!(!EmbedderId::required_registrations().contains(&EmbedderId::E5));
        assert!(!EmbedderId::required_registrations().contains(&EmbedderId::E11));
        assert!(!EmbedderId::required_registrations().contains(&EmbedderId::E15));
        assert_eq!(EmbedderId::content().len(), 12);
        assert!(!EmbedderId::content().contains(&EmbedderId::E5));
        assert!(!EmbedderId::content().contains(&EmbedderId::E11));
        assert_eq!(EmbedderId::learner_state().len(), 7);
        assert_eq!("bge_m3".parse::<EmbedderId>().unwrap(), EmbedderId::E14);
    }
}
