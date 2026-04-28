//! Learner-state embedders for the UTL E15-E21 namespace.
//!
//! The content embedding pipeline owns production E14 (`BgeM3Dense`), so the
//! seven learner-state embedders are numbered after it. They live in a separate
//! namespace and do not alter the `ModelId` enum or the persisted
//! `TeleologicalFingerprint` layout.

use std::collections::BTreeMap;
use std::f32::consts::PI;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::time::Instant;

use context_graph_core::learner::{
    LearnerModality, LearnerStateComponents, LearnerStateVector, ModalityEmbedding,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::{EmbeddingError, EmbeddingResult};

/// Number of learner-state embedders in the UTL namespace.
pub const LEARNER_EMBEDDER_COUNT: usize = 7;

/// UTL planned total: production content E1-E14 plus learner-state E15-E21.
pub const UTL_PLANNED_TOTAL_EMBEDDERS: usize = 21;

/// The production content embedders counted before adding learner-state E15-E21.
pub const UTL_CONTENT_EMBEDDERS_BEFORE_LEARNER_STATE: usize = 14;

/// E15-E21 learner-state embedder slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum LearnerEmbedderSlot {
    E15AffectSpeech,
    E16AffectFace,
    E17AffectText,
    E18Ppg,
    E19Eda,
    E20Eeg,
    E21EegArtifactRobust,
}

impl LearnerEmbedderSlot {
    /// All learner-state slots in E15-E21 order.
    #[must_use]
    pub const fn all() -> &'static [Self; LEARNER_EMBEDDER_COUNT] {
        &[
            Self::E15AffectSpeech,
            Self::E16AffectFace,
            Self::E17AffectText,
            Self::E18Ppg,
            Self::E19Eda,
            Self::E20Eeg,
            Self::E21EegArtifactRobust,
        ]
    }

    #[must_use]
    pub const fn slot_number(self) -> u8 {
        match self {
            Self::E15AffectSpeech => 15,
            Self::E16AffectFace => 16,
            Self::E17AffectText => 17,
            Self::E18Ppg => 18,
            Self::E19Eda => 19,
            Self::E20Eeg => 20,
            Self::E21EegArtifactRobust => 21,
        }
    }

    #[must_use]
    pub const fn modality(self) -> LearnerModality {
        match self {
            Self::E15AffectSpeech => LearnerModality::AffectSpeech,
            Self::E16AffectFace => LearnerModality::AffectFace,
            Self::E17AffectText => LearnerModality::AffectText,
            Self::E18Ppg => LearnerModality::Ppg,
            Self::E19Eda => LearnerModality::Eda,
            Self::E20Eeg => LearnerModality::Eeg,
            Self::E21EegArtifactRobust => LearnerModality::EegArtifactRobust,
        }
    }

    #[must_use]
    pub const fn output_dimension(self) -> usize {
        match self {
            Self::E15AffectSpeech => 1024,
            Self::E16AffectFace => 512,
            Self::E17AffectText => 384,
            Self::E18Ppg => 512,
            Self::E19Eda => 64,
            Self::E20Eeg | Self::E21EegArtifactRobust => 768,
        }
    }

    #[must_use]
    pub const fn model_path(self) -> &'static str {
        match self {
            Self::E15AffectSpeech => "affect-speech",
            Self::E16AffectFace => "affect-face",
            Self::E17AffectText => "affect-text",
            Self::E18Ppg => "ppg",
            Self::E19Eda => "eda",
            Self::E20Eeg => "eeg",
            Self::E21EegArtifactRobust => "eeg-robust",
        }
    }

    #[must_use]
    pub const fn model_name(self) -> &'static str {
        match self {
            Self::E15AffectSpeech => "Wav2Vec2-large-MSP-DIM",
            Self::E16AffectFace => "OpenFace 3.0 + MTL emotion/AU/gaze head",
            Self::E17AffectText => "MiniLM-L6-v2 + EmoBank regressor",
            Self::E18Ppg => "PaPaGei PPG foundation model",
            Self::E19Eda => "WESAD chest physiology stress head",
            Self::E20Eeg => "LaBraM EEG foundation model",
            Self::E21EegArtifactRobust => "EEGPT artifact-robust EEG model",
        }
    }

    #[must_use]
    pub const fn scalar_heads(self) -> &'static [&'static str] {
        match self {
            Self::E15AffectSpeech => &["arousal", "dominance", "valence"],
            Self::E16AffectFace => &["valence", "arousal", "au_intensity"],
            Self::E17AffectText => &["valence", "arousal"],
            Self::E18Ppg => &["hrv_coherence"],
            Self::E19Eda => &["stress_floor"],
            Self::E20Eeg | Self::E21EegArtifactRobust => &["plasticity_window"],
        }
    }

    #[must_use]
    pub fn from_modality(modality: LearnerModality) -> Option<Self> {
        match modality {
            LearnerModality::AffectSpeech => Some(Self::E15AffectSpeech),
            LearnerModality::AffectFace => Some(Self::E16AffectFace),
            LearnerModality::AffectText => Some(Self::E17AffectText),
            LearnerModality::Ppg => Some(Self::E18Ppg),
            LearnerModality::Eda => Some(Self::E19Eda),
            LearnerModality::Eeg => Some(Self::E20Eeg),
            LearnerModality::EegArtifactRobust => Some(Self::E21EegArtifactRobust),
            LearnerModality::SelfReport => None,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::E15AffectSpeech => "e15_affect_speech",
            Self::E16AffectFace => "e16_affect_face",
            Self::E17AffectText => "e17_affect_text",
            Self::E18Ppg => "e18_ppg",
            Self::E19Eda => "e19_eda",
            Self::E20Eeg => "e20_eeg",
            Self::E21EegArtifactRobust => "e21_eeg_artifact_robust",
        }
    }
}

/// Static model metadata for one learner-state embedder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LearnerEmbedderSpec {
    pub slot: LearnerEmbedderSlot,
    pub slot_number: u8,
    pub modality: LearnerModality,
    pub model_name: &'static str,
    pub model_path: &'static str,
    pub output_dimension: usize,
    pub scalar_heads: &'static [&'static str],
}

/// Returns all E15-E21 learner-state embedder specs.
#[must_use]
pub fn learner_embedder_specs() -> Vec<LearnerEmbedderSpec> {
    LearnerEmbedderSlot::all()
        .iter()
        .copied()
        .map(|slot| LearnerEmbedderSpec {
            slot,
            slot_number: slot.slot_number(),
            modality: slot.modality(),
            model_name: slot.model_name(),
            model_path: slot.model_path(),
            output_dimension: slot.output_dimension(),
            scalar_heads: slot.scalar_heads(),
        })
        .collect()
}

/// Input accepted by the local Phase-0 learner extractors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LearnerEmbedderInput {
    Text {
        content: String,
    },
    Samples {
        modality: LearnerModality,
        samples: Vec<f32>,
        sample_rate_hz: u32,
        channels: u16,
    },
    Features {
        modality: LearnerModality,
        values: Vec<f32>,
    },
}

impl LearnerEmbedderInput {
    #[must_use]
    pub fn modality(&self) -> LearnerModality {
        match self {
            Self::Text { .. } => LearnerModality::AffectText,
            Self::Samples { modality, .. } | Self::Features { modality, .. } => *modality,
        }
    }
}

/// Output from one E15-E21 learner-state embedder.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LearnerEmbeddingOutput {
    pub slot: LearnerEmbedderSlot,
    pub modality: LearnerModality,
    pub vector: Vec<f32>,
    pub scalars: BTreeMap<String, f32>,
    pub embedder_version: String,
    pub latency_us: u64,
}

impl LearnerEmbeddingOutput {
    #[must_use]
    pub fn mean_scalar(&self) -> Option<f32> {
        if self.scalars.is_empty() {
            return None;
        }
        Some(self.scalars.values().sum::<f32>() / self.scalars.len() as f32)
    }

    #[must_use]
    pub fn to_modality_embedding(&self, source_observation_id: Uuid) -> ModalityEmbedding {
        ModalityEmbedding {
            modality: self.modality,
            vector: self.vector.clone(),
            scalar: self.mean_scalar(),
            source_observation_id,
        }
    }
}

/// Dataset metadata used to calibrate and validate the learner-state embedders.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct CalibrationDatasetSpec {
    pub name: &'static str,
    pub modality: &'static str,
    pub used_for: &'static str,
    pub access: &'static str,
    pub license: &'static str,
}

/// Public/research datasets from the UTL data plan.
pub const CALIBRATION_DATASET_MANIFEST: &[CalibrationDatasetSpec] = &[
    CalibrationDatasetSpec {
        name: "WESAD",
        modality: "EDA/ECG/RESP/ACC/TEMP",
        used_for: "E18 PPG alignment and E19 EDA stress-floor calibration",
        access: "research download",
        license: "research",
    },
    CalibrationDatasetSpec {
        name: "MSP-Podcast",
        modality: "speech",
        used_for: "E15 speech valence/arousal/dominance calibration",
        access: "research download",
        license: "research",
    },
    CalibrationDatasetSpec {
        name: "EmoBank",
        modality: "text",
        used_for: "E17 text valence/arousal regression calibration",
        access: "public",
        license: "open",
    },
    CalibrationDatasetSpec {
        name: "GoEmotions",
        modality: "text",
        used_for: "E17 text affect category sanity checks",
        access: "public",
        license: "open",
    },
    CalibrationDatasetSpec {
        name: "DAiSEE",
        modality: "face video",
        used_for: "E16 engagement/affect face-video calibration",
        access: "research download",
        license: "research",
    },
    CalibrationDatasetSpec {
        name: "AffWild2",
        modality: "audio/video affect",
        used_for: "E15 and E16 valence/arousal cross-modal calibration",
        access: "research download",
        license: "research",
    },
    CalibrationDatasetSpec {
        name: "DEAP",
        modality: "EEG/peripheral physiology",
        used_for: "E20 EEG plasticity and affect cross-checks",
        access: "research download",
        license: "research",
    },
    CalibrationDatasetSpec {
        name: "MAHNOB-HCI",
        modality: "EEG/PPG/face",
        used_for: "E18/E20 multimodal affect-state alignment",
        access: "research download",
        license: "research",
    },
    CalibrationDatasetSpec {
        name: "SEED/SEED-IV",
        modality: "EEG",
        used_for: "E20/E21 EEG emotion and artifact robustness checks",
        access: "research download",
        license: "research",
    },
    CalibrationDatasetSpec {
        name: "Sleep-EDF",
        modality: "polysomnography",
        used_for: "sleep-gated k(tau) calibration",
        access: "public",
        license: "open",
    },
];

/// One file group that must exist for a learner model asset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LearnerAssetFileGroup {
    pub label: &'static str,
    pub alternatives: &'static [&'static str],
}

/// One E15-E21 model asset requirement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LearnerModelAssetRequirement {
    pub slot: LearnerEmbedderSlot,
    pub model_dir: &'static str,
    pub source: &'static str,
    pub required: &'static [LearnerAssetFileGroup],
}

/// Result for one required file group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LearnerAssetFileStatus {
    pub label: String,
    pub candidates: Vec<String>,
    pub present_path: Option<String>,
    pub bytes: Option<u64>,
    pub sha256: Option<String>,
}

/// Preflight result for one E15-E21 model directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LearnerModelAssetStatus {
    pub slot: LearnerEmbedderSlot,
    pub slot_id: &'static str,
    pub model_dir: String,
    pub source: &'static str,
    pub ready: bool,
    pub files: Vec<LearnerAssetFileStatus>,
}

/// Preflight result for the minimum real calibration dataset used by FSV.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LearnerCalibrationAssetStatus {
    pub dataset_id: &'static str,
    pub path: String,
    pub source: &'static str,
    pub ready: bool,
    pub files: Vec<LearnerAssetFileStatus>,
}

/// Full learner asset preflight report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LearnerAssetPreflightReport {
    pub source_of_truth: &'static str,
    pub models_root: String,
    pub calibration_root: String,
    pub ready: bool,
    pub missing_count: usize,
    pub model_assets: Vec<LearnerModelAssetStatus>,
    pub calibration_assets: Vec<LearnerCalibrationAssetStatus>,
}

const HF_AFFECT_SPEECH: &str = "hf:audeering/wav2vec2-large-robust-12-ft-emotion-msp-dim";
const HF_AFFECT_TEXT: &str = "hf:sentence-transformers/all-MiniLM-L6-v2";
const HF_LABRAM: &str = "hf:braindecode/labram-pretrained";
const HF_EEGPT: &str = "hf:braindecode/eegpt-pretrained";
const GO_EMOTIONS_SOURCE: &str = "hf-dataset:google-research-datasets/go_emotions";
const WESAD_SOURCE: &str = "official:WESAD raw archive from University of Siegen";

const GROUP_CONFIG: LearnerAssetFileGroup = LearnerAssetFileGroup {
    label: "model config",
    alternatives: &["config.json"],
};
const GROUP_WEIGHTS: LearnerAssetFileGroup = LearnerAssetFileGroup {
    label: "model weights",
    alternatives: &[
        "model.safetensors",
        "pytorch_model.bin",
        "model.bin",
        "weights.pt",
    ],
};
const GROUP_PAPAGEI_WEIGHTS: LearnerAssetFileGroup = LearnerAssetFileGroup {
    label: "PaPaGei PPG weights",
    alternatives: &["papagei_s.pt"],
};
const GROUP_TOKENIZER: LearnerAssetFileGroup = LearnerAssetFileGroup {
    label: "tokenizer",
    alternatives: &["tokenizer.json", "vocab.txt", "sentencepiece.bpe.model"],
};
const GROUP_PREPROCESSOR: LearnerAssetFileGroup = LearnerAssetFileGroup {
    label: "signal preprocessor",
    alternatives: &["preprocessor_config.json", "processor_config.json"],
};
const GROUP_OPENFACE_INTERFACE: LearnerAssetFileGroup = LearnerAssetFileGroup {
    label: "OpenFace 3.0 interface",
    alternatives: &["OpenFace/interface.py"],
};
const GROUP_OPENFACE_MULTITASK_MODEL: LearnerAssetFileGroup = LearnerAssetFileGroup {
    label: "OpenFace multitask model definition",
    alternatives: &["OpenFace/model/MLT.py"],
};
const GROUP_OPENFACE_FACE_DETECTOR: LearnerAssetFileGroup = LearnerAssetFileGroup {
    label: "OpenFace RetinaFace weights",
    alternatives: &[
        "OpenFace/weights/Alignment_RetinaFace.pth",
        "OpenFace/weights/mobilenet0.25_Final.pth",
    ],
};
const GROUP_OPENFACE_LANDMARKS: LearnerAssetFileGroup = LearnerAssetFileGroup {
    label: "OpenFace landmark weights",
    alternatives: &[
        "OpenFace/weights/Landmark_98.pkl",
        "OpenFace/weights/Landmark_68.pkl",
    ],
};
const GROUP_OPENFACE_MULTITASK_WEIGHTS: LearnerAssetFileGroup = LearnerAssetFileGroup {
    label: "OpenFace emotion/AU/gaze weights",
    alternatives: &[
        "OpenFace/weights/MTL_backbone.pth",
        "OpenFace/weights/stage2_epoch_7_loss_1.1606_acc_0.5589.pth",
    ],
};
const GROUP_EDA_HEAD: LearnerAssetFileGroup = LearnerAssetFileGroup {
    label: "WESAD stress-floor head",
    alternatives: &[
        "wesad_stress_head.pt",
        "wesad_cnn.safetensors",
        "wesad_cnn.pt",
        "model.safetensors",
    ],
};
const GROUP_GOEMOTIONS_SAMPLE: LearnerAssetFileGroup = LearnerAssetFileGroup {
    label: "real GoEmotions calibration sample",
    alternatives: &["go_emotions_sample.jsonl"],
};
const GROUP_GOEMOTIONS_MANIFEST: LearnerAssetFileGroup = LearnerAssetFileGroup {
    label: "real GoEmotions manifest",
    alternatives: &["manifest.json"],
};
const GROUP_WESAD_ARCHIVE: LearnerAssetFileGroup = LearnerAssetFileGroup {
    label: "official raw WESAD archive",
    alternatives: &["WESAD.zip"],
};
const GROUP_WESAD_MANIFEST: LearnerAssetFileGroup = LearnerAssetFileGroup {
    label: "official WESAD manifest",
    alternatives: &["manifest.json"],
};

static REQ_E15_SPEECH: &[LearnerAssetFileGroup] =
    &[GROUP_CONFIG, GROUP_WEIGHTS, GROUP_PREPROCESSOR];
static REQ_E16_FACE: &[LearnerAssetFileGroup] = &[
    GROUP_OPENFACE_INTERFACE,
    GROUP_OPENFACE_MULTITASK_MODEL,
    GROUP_OPENFACE_FACE_DETECTOR,
    GROUP_OPENFACE_LANDMARKS,
    GROUP_OPENFACE_MULTITASK_WEIGHTS,
];
static REQ_E17_TEXT: &[LearnerAssetFileGroup] = &[GROUP_CONFIG, GROUP_WEIGHTS, GROUP_TOKENIZER];
static REQ_E18_PPG: &[LearnerAssetFileGroup] = &[GROUP_CONFIG, GROUP_PAPAGEI_WEIGHTS];
static REQ_E19_EDA: &[LearnerAssetFileGroup] = &[GROUP_CONFIG, GROUP_EDA_HEAD];
static REQ_E20_EEG: &[LearnerAssetFileGroup] = &[GROUP_CONFIG, GROUP_WEIGHTS];
static REQ_E21_EEG_ROBUST: &[LearnerAssetFileGroup] = &[GROUP_CONFIG, GROUP_WEIGHTS];
static REQ_GOEMOTIONS: &[LearnerAssetFileGroup] =
    &[GROUP_GOEMOTIONS_SAMPLE, GROUP_GOEMOTIONS_MANIFEST];
static REQ_WESAD: &[LearnerAssetFileGroup] = &[GROUP_WESAD_ARCHIVE, GROUP_WESAD_MANIFEST];

/// Model asset requirements for the learner-state E15-E21 namespace.
#[must_use]
pub fn learner_model_asset_requirements() -> Vec<LearnerModelAssetRequirement> {
    vec![
        LearnerModelAssetRequirement {
            slot: LearnerEmbedderSlot::E15AffectSpeech,
            model_dir: "affect-speech",
            source: HF_AFFECT_SPEECH,
            required: REQ_E15_SPEECH,
        },
        LearnerModelAssetRequirement {
            slot: LearnerEmbedderSlot::E16AffectFace,
            model_dir: "affect-face",
            source: "github:CMU-MultiComp-Lab/OpenFace-3.0 + hf:nutPace/openface_weights",
            required: REQ_E16_FACE,
        },
        LearnerModelAssetRequirement {
            slot: LearnerEmbedderSlot::E17AffectText,
            model_dir: "affect-text",
            source: HF_AFFECT_TEXT,
            required: REQ_E17_TEXT,
        },
        LearnerModelAssetRequirement {
            slot: LearnerEmbedderSlot::E18Ppg,
            model_dir: "ppg",
            source: "zenodo:10.5281/zenodo.13983110:papagei_s.pt",
            required: REQ_E18_PPG,
        },
        LearnerModelAssetRequirement {
            slot: LearnerEmbedderSlot::E19Eda,
            model_dir: "eda",
            source: "local-training:official WESAD raw ECG/EDA/RESP/TEMP stress-floor head",
            required: REQ_E19_EDA,
        },
        LearnerModelAssetRequirement {
            slot: LearnerEmbedderSlot::E20Eeg,
            model_dir: "eeg",
            source: HF_LABRAM,
            required: REQ_E20_EEG,
        },
        LearnerModelAssetRequirement {
            slot: LearnerEmbedderSlot::E21EegArtifactRobust,
            model_dir: "eeg-robust",
            source: HF_EEGPT,
            required: REQ_E21_EEG_ROBUST,
        },
    ]
}

/// Inspect model and real calibration dataset assets without falling back.
pub fn preflight_learner_assets(
    models_root: impl AsRef<Path>,
    calibration_root: impl AsRef<Path>,
) -> EmbeddingResult<LearnerAssetPreflightReport> {
    let models_root = models_root.as_ref();
    let calibration_root = calibration_root.as_ref();
    let mut missing_count = 0usize;

    let mut model_assets = Vec::new();
    for requirement in learner_model_asset_requirements() {
        let model_dir = models_root.join(requirement.model_dir);
        let files = requirement
            .required
            .iter()
            .map(|group| inspect_file_group(&model_dir, group))
            .collect::<EmbeddingResult<Vec<_>>>()?;
        missing_count += files
            .iter()
            .filter(|status| status.present_path.is_none())
            .count();
        model_assets.push(LearnerModelAssetStatus {
            slot: requirement.slot,
            slot_id: requirement.slot.as_str(),
            model_dir: model_dir.to_string_lossy().into_owned(),
            source: requirement.source,
            ready: files.iter().all(|status| status.present_path.is_some()),
            files,
        });
    }

    let go_emotions_dir = calibration_root.join("go_emotions");
    let go_emotions_files = REQ_GOEMOTIONS
        .iter()
        .map(|group| inspect_file_group(&go_emotions_dir, group))
        .collect::<EmbeddingResult<Vec<_>>>()?;
    missing_count += go_emotions_files
        .iter()
        .filter(|status| status.present_path.is_none())
        .count();
    let wesad_dir = calibration_root.join("wesad_official");
    let wesad_files = REQ_WESAD
        .iter()
        .map(|group| inspect_file_group(&wesad_dir, group))
        .collect::<EmbeddingResult<Vec<_>>>()?;
    missing_count += wesad_files
        .iter()
        .filter(|status| status.present_path.is_none())
        .count();

    let calibration_assets = vec![
        LearnerCalibrationAssetStatus {
            dataset_id: "go_emotions",
            path: go_emotions_dir.to_string_lossy().into_owned(),
            source: GO_EMOTIONS_SOURCE,
            ready: go_emotions_files
                .iter()
                .all(|status| status.present_path.is_some()),
            files: go_emotions_files,
        },
        LearnerCalibrationAssetStatus {
            dataset_id: "wesad_official",
            path: wesad_dir.to_string_lossy().into_owned(),
            source: WESAD_SOURCE,
            ready: wesad_files
                .iter()
                .all(|status| status.present_path.is_some()),
            files: wesad_files,
        },
    ];

    Ok(LearnerAssetPreflightReport {
        source_of_truth: "filesystem model assets + downloaded real calibration data",
        models_root: models_root.to_string_lossy().into_owned(),
        calibration_root: calibration_root.to_string_lossy().into_owned(),
        ready: missing_count == 0,
        missing_count,
        model_assets,
        calibration_assets,
    })
}

/// One deterministic fixture sample used for FSV and local calibration smoke tests.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LearnerCalibrationSample {
    pub sample_id: &'static str,
    pub label: &'static str,
    pub expected_components: LearnerStateComponents,
    pub inputs: BTreeMap<LearnerEmbedderSlot, LearnerEmbedderInput>,
}

/// Deterministic fixture with all E15-E21 inputs present.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LearnerCalibrationFixture {
    pub fixture_id: &'static str,
    pub source_of_truth: &'static str,
    pub samples: Vec<LearnerCalibrationSample>,
}

/// Build the deterministic local dataset used by tests and manual FSV.
#[must_use]
pub fn synthetic_calibration_fixture() -> LearnerCalibrationFixture {
    LearnerCalibrationFixture {
        fixture_id: "utl-phase0-synthetic-v1",
        source_of_truth: "context_graph_embeddings::learner::synthetic_calibration_fixture",
        samples: vec![regulated_sample(), dysregulated_sample(), boundary_sample()],
    }
}

/// Run one learner-state embedder over one input.
pub fn embed_learner_signal(
    slot: LearnerEmbedderSlot,
    input: &LearnerEmbedderInput,
) -> EmbeddingResult<LearnerEmbeddingOutput> {
    if slot.modality() != input.modality() {
        return Err(EmbeddingError::ConfigError {
            message: format!(
                "input modality {} does not match {}",
                input.modality().as_str(),
                slot.as_str()
            ),
        });
    }

    let start = Instant::now();
    let (features, scalars) = match slot {
        LearnerEmbedderSlot::E15AffectSpeech => speech_features(input)?,
        LearnerEmbedderSlot::E16AffectFace => face_features(input)?,
        LearnerEmbedderSlot::E17AffectText => text_affect_features(input)?,
        LearnerEmbedderSlot::E18Ppg => ppg_features(input)?,
        LearnerEmbedderSlot::E19Eda => eda_features(input)?,
        LearnerEmbedderSlot::E20Eeg => eeg_features(input, false)?,
        LearnerEmbedderSlot::E21EegArtifactRobust => eeg_features(input, true)?,
    };
    let vector = project_features(
        &features,
        slot.output_dimension(),
        slot.slot_number() as u64,
    );
    validate_vector(&vector, slot.output_dimension())?;
    Ok(LearnerEmbeddingOutput {
        slot,
        modality: slot.modality(),
        vector,
        scalars,
        embedder_version: format!("{}:phase0-deterministic-v1", slot.as_str()),
        latency_us: start.elapsed().as_micros() as u64,
    })
}

/// Run every E15-E21 learner-state embedder for a fixture sample.
pub fn embed_calibration_sample(
    sample: &LearnerCalibrationSample,
) -> EmbeddingResult<Vec<LearnerEmbeddingOutput>> {
    let mut outputs = Vec::with_capacity(LEARNER_EMBEDDER_COUNT);
    for slot in LearnerEmbedderSlot::all() {
        let input = sample
            .inputs
            .get(slot)
            .ok_or_else(|| EmbeddingError::ConfigError {
                message: format!("missing fixture input for {}", slot.as_str()),
            })?;
        outputs.push(embed_learner_signal(*slot, input)?);
    }
    Ok(outputs)
}

/// Concatenate E15-E21 outputs into a learner state vector and derive UTL components.
pub fn state_vector_from_outputs(
    learner_id: Uuid,
    session_ts: u64,
    outputs: &[LearnerEmbeddingOutput],
    mut context: BTreeMap<String, String>,
) -> EmbeddingResult<LearnerStateVector> {
    validate_all_slots_present(outputs)?;
    let components = components_from_outputs(outputs)?;
    let values = concatenate_outputs(outputs);
    context.insert("learner_embedder_count".into(), outputs.len().to_string());
    context.insert("learner_embedder_namespace".into(), "utl_e15_e21".into());
    let state = LearnerStateVector {
        learner_id,
        session_ts,
        values,
        components,
        context,
    };
    state.validate().map_err(|e| EmbeddingError::ConfigError {
        message: format!("learner state vector validation failed: {e}"),
    })?;
    Ok(state)
}

/// Derive reduced UTL Delta-E components from E15-E21 outputs.
pub fn components_from_outputs(
    outputs: &[LearnerEmbeddingOutput],
) -> EmbeddingResult<LearnerStateComponents> {
    validate_all_slots_present(outputs)?;
    let speech = output_for(outputs, LearnerEmbedderSlot::E15AffectSpeech)?;
    let face = output_for(outputs, LearnerEmbedderSlot::E16AffectFace)?;
    let text = output_for(outputs, LearnerEmbedderSlot::E17AffectText)?;
    let ppg = output_for(outputs, LearnerEmbedderSlot::E18Ppg)?;
    let eda = output_for(outputs, LearnerEmbedderSlot::E19Eda)?;
    let eeg = output_for(outputs, LearnerEmbedderSlot::E20Eeg)?;
    let eeg_robust = output_for(outputs, LearnerEmbedderSlot::E21EegArtifactRobust)?;

    let valence = mean_existing([
        speech.scalars.get("valence").copied(),
        face.scalars.get("valence").copied(),
        text.scalars.get("valence").copied(),
    ])
    .clamp(-1.0, 1.0);
    let arousal = mean_existing([
        speech.scalars.get("arousal").copied(),
        face.scalars.get("arousal").copied(),
        text.scalars.get("arousal").copied(),
    ])
    .clamp(-1.0, 1.0);
    let plasticity_window = mean_existing([
        eeg.scalars.get("plasticity_window").copied(),
        eeg_robust.scalars.get("plasticity_window").copied(),
    ])
    .clamp(0.0, 1.0);
    let hrv_coherence = ppg
        .scalars
        .get("hrv_coherence")
        .copied()
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);
    let stress_floor = eda
        .scalars
        .get("stress_floor")
        .copied()
        .unwrap_or(0.0)
        .clamp(0.0, 1.0);

    let components = LearnerStateComponents {
        plasticity_window,
        hrv_coherence,
        valence,
        arousal,
        stress_floor,
        k_sleep: 1.0,
    };
    components
        .validate()
        .map_err(|e| EmbeddingError::ConfigError {
            message: format!("derived learner components invalid: {e}"),
        })?;
    Ok(components)
}

fn regulated_sample() -> LearnerCalibrationSample {
    let mut inputs = BTreeMap::new();
    inputs.insert(
        LearnerEmbedderSlot::E15AffectSpeech,
        LearnerEmbedderInput::Samples {
            modality: LearnerModality::AffectSpeech,
            samples: sine_series(160, 4.0, 0.45),
            sample_rate_hz: 16_000,
            channels: 1,
        },
    );
    inputs.insert(
        LearnerEmbedderSlot::E16AffectFace,
        LearnerEmbedderInput::Features {
            modality: LearnerModality::AffectFace,
            values: vec![0.2, 0.1, 0.15, 0.05, 0.22, 0.18, 0.1, 0.08, 0.12, 0.16],
        },
    );
    inputs.insert(
        LearnerEmbedderSlot::E17AffectText,
        LearnerEmbedderInput::Text {
            content: "I feel calm, focused, and ready to learn the new concept.".into(),
        },
    );
    inputs.insert(
        LearnerEmbedderSlot::E18Ppg,
        LearnerEmbedderInput::Samples {
            modality: LearnerModality::Ppg,
            samples: sine_series(128, 1.2, 0.8),
            sample_rate_hz: 64,
            channels: 1,
        },
    );
    inputs.insert(
        LearnerEmbedderSlot::E19Eda,
        LearnerEmbedderInput::Samples {
            modality: LearnerModality::Eda,
            samples: low_ramp_series(64, 0.12, 0.02),
            sample_rate_hz: 4,
            channels: 1,
        },
    );
    inputs.insert(
        LearnerEmbedderSlot::E20Eeg,
        LearnerEmbedderInput::Samples {
            modality: LearnerModality::Eeg,
            samples: mixed_eeg_series(256, 256.0, 6.0, 0.7, 20.0, 0.2),
            sample_rate_hz: 256,
            channels: 4,
        },
    );
    inputs.insert(
        LearnerEmbedderSlot::E21EegArtifactRobust,
        LearnerEmbedderInput::Samples {
            modality: LearnerModality::EegArtifactRobust,
            samples: mixed_eeg_series(256, 256.0, 6.0, 0.65, 30.0, 0.08),
            sample_rate_hz: 256,
            channels: 4,
        },
    );
    LearnerCalibrationSample {
        sample_id: "regulated-baseline",
        label: "regulated",
        expected_components: LearnerStateComponents {
            plasticity_window: 0.7,
            hrv_coherence: 0.7,
            valence: 0.2,
            arousal: 0.0,
            stress_floor: 0.8,
            k_sleep: 1.0,
        },
        inputs,
    }
}

fn dysregulated_sample() -> LearnerCalibrationSample {
    let mut inputs = BTreeMap::new();
    inputs.insert(
        LearnerEmbedderSlot::E15AffectSpeech,
        LearnerEmbedderInput::Samples {
            modality: LearnerModality::AffectSpeech,
            samples: noisy_series(160, 0.9, 0.35),
            sample_rate_hz: 16_000,
            channels: 1,
        },
    );
    inputs.insert(
        LearnerEmbedderSlot::E16AffectFace,
        LearnerEmbedderInput::Features {
            modality: LearnerModality::AffectFace,
            values: vec![0.8, 0.7, 0.75, 0.65, 0.9, 0.8, 0.72, 0.67, 0.7, 0.85],
        },
    );
    inputs.insert(
        LearnerEmbedderSlot::E17AffectText,
        LearnerEmbedderInput::Text {
            content: "I am anxious, stressed, confused, and frustrated by this material.".into(),
        },
    );
    inputs.insert(
        LearnerEmbedderSlot::E18Ppg,
        LearnerEmbedderInput::Samples {
            modality: LearnerModality::Ppg,
            samples: noisy_series(128, 0.8, 0.45),
            sample_rate_hz: 64,
            channels: 1,
        },
    );
    inputs.insert(
        LearnerEmbedderSlot::E19Eda,
        LearnerEmbedderInput::Samples {
            modality: LearnerModality::Eda,
            samples: low_ramp_series(64, 0.82, 0.5),
            sample_rate_hz: 4,
            channels: 1,
        },
    );
    inputs.insert(
        LearnerEmbedderSlot::E20Eeg,
        LearnerEmbedderInput::Samples {
            modality: LearnerModality::Eeg,
            samples: mixed_eeg_series(256, 256.0, 30.0, 0.8, 6.0, 0.15),
            sample_rate_hz: 256,
            channels: 4,
        },
    );
    inputs.insert(
        LearnerEmbedderSlot::E21EegArtifactRobust,
        LearnerEmbedderInput::Samples {
            modality: LearnerModality::EegArtifactRobust,
            samples: mixed_eeg_series(256, 256.0, 30.0, 0.7, 6.0, 0.1),
            sample_rate_hz: 256,
            channels: 4,
        },
    );
    LearnerCalibrationSample {
        sample_id: "dysregulated-stress",
        label: "dysregulated",
        expected_components: LearnerStateComponents {
            plasticity_window: 0.2,
            hrv_coherence: 0.2,
            valence: -0.5,
            arousal: 0.7,
            stress_floor: 0.2,
            k_sleep: 1.0,
        },
        inputs,
    }
}

fn boundary_sample() -> LearnerCalibrationSample {
    let mut inputs = BTreeMap::new();
    inputs.insert(
        LearnerEmbedderSlot::E15AffectSpeech,
        LearnerEmbedderInput::Samples {
            modality: LearnerModality::AffectSpeech,
            samples: vec![0.0; 32],
            sample_rate_hz: 16_000,
            channels: 1,
        },
    );
    inputs.insert(
        LearnerEmbedderSlot::E16AffectFace,
        LearnerEmbedderInput::Features {
            modality: LearnerModality::AffectFace,
            values: vec![0.0; 17],
        },
    );
    inputs.insert(
        LearnerEmbedderSlot::E17AffectText,
        LearnerEmbedderInput::Text {
            content: "neutral".into(),
        },
    );
    inputs.insert(
        LearnerEmbedderSlot::E18Ppg,
        LearnerEmbedderInput::Samples {
            modality: LearnerModality::Ppg,
            samples: vec![0.5; 32],
            sample_rate_hz: 64,
            channels: 1,
        },
    );
    inputs.insert(
        LearnerEmbedderSlot::E19Eda,
        LearnerEmbedderInput::Samples {
            modality: LearnerModality::Eda,
            samples: vec![0.0; 32],
            sample_rate_hz: 4,
            channels: 1,
        },
    );
    inputs.insert(
        LearnerEmbedderSlot::E20Eeg,
        LearnerEmbedderInput::Samples {
            modality: LearnerModality::Eeg,
            samples: vec![0.0; 64],
            sample_rate_hz: 256,
            channels: 4,
        },
    );
    inputs.insert(
        LearnerEmbedderSlot::E21EegArtifactRobust,
        LearnerEmbedderInput::Samples {
            modality: LearnerModality::EegArtifactRobust,
            samples: vec![0.0; 64],
            sample_rate_hz: 256,
            channels: 4,
        },
    );
    LearnerCalibrationSample {
        sample_id: "boundary-flat-signals",
        label: "boundary",
        expected_components: LearnerStateComponents {
            plasticity_window: 0.5,
            hrv_coherence: 0.0,
            valence: 0.0,
            arousal: 0.0,
            stress_floor: 1.0,
            k_sleep: 1.0,
        },
        inputs,
    }
}

fn text_affect_features(
    input: &LearnerEmbedderInput,
) -> EmbeddingResult<(Vec<f32>, BTreeMap<String, f32>)> {
    let LearnerEmbedderInput::Text { content } = input else {
        return Err(EmbeddingError::ConfigError {
            message: "E17 affect-text requires text input".into(),
        });
    };
    if content.trim().is_empty() {
        return Err(EmbeddingError::EmptyInput);
    }

    let mut token_count = 0.0f32;
    let mut positive = 0.0f32;
    let mut negative = 0.0f32;
    let mut stress = 0.0f32;
    let mut calm = 0.0f32;
    for raw in content.split(|ch: char| !ch.is_ascii_alphanumeric()) {
        if raw.is_empty() {
            continue;
        }
        token_count += 1.0;
        let token = raw.to_ascii_lowercase();
        if matches!(
            token.as_str(),
            "calm" | "focused" | "ready" | "clear" | "good" | "confident" | "learn"
        ) {
            positive += 1.0;
        }
        if matches!(
            token.as_str(),
            "anxious" | "stressed" | "confused" | "frustrated" | "bad" | "overwhelmed"
        ) {
            negative += 1.0;
        }
        if matches!(
            token.as_str(),
            "anxious" | "stressed" | "panic" | "rushed" | "overwhelmed" | "frustrated"
        ) {
            stress += 1.0;
        }
        if matches!(
            token.as_str(),
            "calm" | "settled" | "focused" | "steady" | "relaxed"
        ) {
            calm += 1.0;
        }
    }
    let punctuation = content
        .chars()
        .filter(|ch| matches!(*ch, '!' | '?' | ';' | ':'))
        .count() as f32;
    let upper = content.chars().filter(char::is_ascii_uppercase).count() as f32;
    let chars = content.chars().count().max(1) as f32;
    let valence = ((positive - negative) / (positive + negative + 1.0)).clamp(-1.0, 1.0);
    let arousal = ((stress + punctuation / 3.0 + upper / chars) / (token_count + 1.0)
        - calm / (token_count + 1.0))
        .mul_add(2.0, 0.0)
        .clamp(-1.0, 1.0);

    let mut features = vec![
        token_count / 64.0,
        positive / (token_count + 1.0),
        negative / (token_count + 1.0),
        stress / (token_count + 1.0),
        calm / (token_count + 1.0),
        punctuation / 16.0,
        upper / chars,
        valence,
        arousal,
    ];
    features.extend(hashed_text_features(content, 32));

    let mut scalars = BTreeMap::new();
    scalars.insert("valence".into(), valence);
    scalars.insert("arousal".into(), arousal);
    Ok((features, scalars))
}

fn speech_features(
    input: &LearnerEmbedderInput,
) -> EmbeddingResult<(Vec<f32>, BTreeMap<String, f32>)> {
    let (samples, sample_rate_hz, channels) =
        require_samples(input, LearnerModality::AffectSpeech)?;
    validate_signal_metadata(sample_rate_hz, channels)?;
    let summary = numeric_summary(samples)?;
    let rms = summary[2].clamp(0.0, 1.0);
    let zcr = summary[7].clamp(0.0, 1.0);
    let arousal = ((rms * 2.0 + zcr) - 0.5).clamp(-1.0, 1.0);
    let valence = (0.4 - summary[1] - 0.2 * zcr).clamp(-1.0, 1.0);
    let dominance = (0.5 + summary[2] - 0.5 * summary[1]).clamp(0.0, 1.0);
    let mut scalars = BTreeMap::new();
    scalars.insert("arousal".into(), arousal);
    scalars.insert("dominance".into(), dominance);
    scalars.insert("valence".into(), valence);
    Ok((summary, scalars))
}

fn face_features(
    input: &LearnerEmbedderInput,
) -> EmbeddingResult<(Vec<f32>, BTreeMap<String, f32>)> {
    let values = require_features(input, LearnerModality::AffectFace)?;
    let summary = numeric_summary(values)?;
    let mean = summary[0].clamp(0.0, 1.0);
    let spread = summary[1].clamp(0.0, 1.0);
    let valence = (0.35 - mean + values.get(4).copied().unwrap_or(0.0) * 0.5).clamp(-1.0, 1.0);
    let arousal = (mean + spread * 2.0 - 0.25).clamp(-1.0, 1.0);
    let au_intensity = mean.clamp(0.0, 1.0);
    let mut scalars = BTreeMap::new();
    scalars.insert("valence".into(), valence);
    scalars.insert("arousal".into(), arousal);
    scalars.insert("au_intensity".into(), au_intensity);
    Ok((summary, scalars))
}

fn ppg_features(
    input: &LearnerEmbedderInput,
) -> EmbeddingResult<(Vec<f32>, BTreeMap<String, f32>)> {
    let (samples, sample_rate_hz, channels) = require_samples(input, LearnerModality::Ppg)?;
    validate_signal_metadata(sample_rate_hz, channels)?;
    let mut summary = numeric_summary(samples)?;
    let diff = first_difference(samples);
    let diff_summary = numeric_summary(&diff)?;
    summary.extend(diff_summary.iter().copied());
    let periodicity = periodicity_score(samples);
    let smoothness = (1.0 / (1.0 + diff_summary[1] * 8.0)).clamp(0.0, 1.0);
    let hrv_coherence = (0.65 * periodicity + 0.35 * smoothness).clamp(0.0, 1.0);
    let mut scalars = BTreeMap::new();
    scalars.insert("hrv_coherence".into(), hrv_coherence);
    Ok((summary, scalars))
}

fn eda_features(
    input: &LearnerEmbedderInput,
) -> EmbeddingResult<(Vec<f32>, BTreeMap<String, f32>)> {
    let (samples, sample_rate_hz, channels) = require_samples(input, LearnerModality::Eda)?;
    validate_signal_metadata(sample_rate_hz, channels)?;
    let mut summary = numeric_summary(samples)?;
    let diff = first_difference(samples);
    let diff_summary = numeric_summary(&diff)?;
    summary.extend(diff_summary.iter().copied());
    let tonic = summary[0].clamp(0.0, 1.0);
    let phasic = diff_summary[2].clamp(0.0, 1.0);
    let stress_floor = (1.0 - 0.75 * tonic - 0.55 * phasic).clamp(0.0, 1.0);
    let mut scalars = BTreeMap::new();
    scalars.insert("stress_floor".into(), stress_floor);
    Ok((summary, scalars))
}

fn eeg_features(
    input: &LearnerEmbedderInput,
    robust: bool,
) -> EmbeddingResult<(Vec<f32>, BTreeMap<String, f32>)> {
    let expected = if robust {
        LearnerModality::EegArtifactRobust
    } else {
        LearnerModality::Eeg
    };
    let (samples, sample_rate_hz, channels) = require_samples(input, expected)?;
    validate_signal_metadata(sample_rate_hz, channels)?;
    let mut summary = numeric_summary(samples)?;
    let theta = frequency_energy(samples, sample_rate_hz as f32, 6.0);
    let alpha = frequency_energy(samples, sample_rate_hz as f32, 10.0);
    let beta = frequency_energy(samples, sample_rate_hz as f32, 20.0);
    let high_beta = frequency_energy(samples, sample_rate_hz as f32, 30.0);
    let total = theta + alpha + beta + high_beta + 1e-6;
    let artifact_penalty = if robust {
        summary[1].min(1.0) * 0.1
    } else {
        summary[1].min(1.0) * 0.25
    };
    let plasticity_window = ((theta + 0.65 * alpha + 0.35 * beta - 0.25 * high_beta) / total
        - artifact_penalty)
        .clamp(0.0, 1.0);
    summary.extend([
        theta / total,
        alpha / total,
        beta / total,
        high_beta / total,
    ]);
    let mut scalars = BTreeMap::new();
    scalars.insert("plasticity_window".into(), plasticity_window);
    Ok((summary, scalars))
}

fn require_samples(
    input: &LearnerEmbedderInput,
    expected: LearnerModality,
) -> EmbeddingResult<(&[f32], u32, u16)> {
    match input {
        LearnerEmbedderInput::Samples {
            modality,
            samples,
            sample_rate_hz,
            channels,
        } if *modality == expected => {
            if samples.is_empty() {
                return Err(EmbeddingError::EmptyInput);
            }
            Ok((samples, *sample_rate_hz, *channels))
        }
        LearnerEmbedderInput::Samples { modality, .. } => Err(EmbeddingError::ConfigError {
            message: format!(
                "expected {} samples, got {} samples",
                expected.as_str(),
                modality.as_str()
            ),
        }),
        _ => Err(EmbeddingError::ConfigError {
            message: format!("{} requires numeric samples", expected.as_str()),
        }),
    }
}

fn require_features(
    input: &LearnerEmbedderInput,
    expected: LearnerModality,
) -> EmbeddingResult<&[f32]> {
    match input {
        LearnerEmbedderInput::Features { modality, values } if *modality == expected => {
            if values.is_empty() {
                return Err(EmbeddingError::EmptyInput);
            }
            Ok(values)
        }
        LearnerEmbedderInput::Features { modality, .. } => Err(EmbeddingError::ConfigError {
            message: format!(
                "expected {} features, got {} features",
                expected.as_str(),
                modality.as_str()
            ),
        }),
        _ => Err(EmbeddingError::ConfigError {
            message: format!("{} requires numeric features", expected.as_str()),
        }),
    }
}

fn validate_signal_metadata(sample_rate_hz: u32, channels: u16) -> EmbeddingResult<()> {
    if sample_rate_hz == 0 {
        return Err(EmbeddingError::ConfigError {
            message: "sample_rate_hz must be > 0".into(),
        });
    }
    if channels == 0 || channels > 256 {
        return Err(EmbeddingError::ConfigError {
            message: format!("channels must be in [1, 256], got {channels}"),
        });
    }
    Ok(())
}

fn inspect_file_group(
    root: &Path,
    group: &LearnerAssetFileGroup,
) -> EmbeddingResult<LearnerAssetFileStatus> {
    let candidates = group
        .alternatives
        .iter()
        .map(|relative| root.join(relative).to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    for relative in group.alternatives {
        let path = root.join(relative);
        if path.is_file() {
            let metadata = path.metadata()?;
            return Ok(LearnerAssetFileStatus {
                label: group.label.to_string(),
                candidates,
                present_path: Some(path.to_string_lossy().into_owned()),
                bytes: Some(metadata.len()),
                sha256: Some(sha256_file(&path)?),
            });
        }
    }

    Ok(LearnerAssetFileStatus {
        label: group.label.to_string(),
        candidates,
        present_path: None,
        bytes: None,
        sha256: None,
    })
}

fn sha256_file(path: &Path) -> EmbeddingResult<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    Ok(out)
}

fn validate_all_slots_present(outputs: &[LearnerEmbeddingOutput]) -> EmbeddingResult<()> {
    if outputs.len() != LEARNER_EMBEDDER_COUNT {
        return Err(EmbeddingError::ConfigError {
            message: format!(
                "expected {LEARNER_EMBEDDER_COUNT} learner embedder outputs, got {}",
                outputs.len()
            ),
        });
    }
    for slot in LearnerEmbedderSlot::all() {
        let output = output_for(outputs, *slot)?;
        validate_vector(&output.vector, slot.output_dimension())?;
    }
    Ok(())
}

fn output_for(
    outputs: &[LearnerEmbeddingOutput],
    slot: LearnerEmbedderSlot,
) -> EmbeddingResult<&LearnerEmbeddingOutput> {
    outputs
        .iter()
        .find(|output| output.slot == slot)
        .ok_or_else(|| EmbeddingError::ConfigError {
            message: format!("missing output for {}", slot.as_str()),
        })
}

fn concatenate_outputs(outputs: &[LearnerEmbeddingOutput]) -> Vec<f32> {
    let mut values = Vec::with_capacity(
        LearnerEmbedderSlot::all()
            .iter()
            .map(|slot| slot.output_dimension())
            .sum(),
    );
    for slot in LearnerEmbedderSlot::all() {
        if let Some(output) = outputs.iter().find(|output| output.slot == *slot) {
            values.extend_from_slice(&output.vector);
        }
    }
    values
}

fn numeric_summary(samples: &[f32]) -> EmbeddingResult<Vec<f32>> {
    if samples.is_empty() {
        return Err(EmbeddingError::EmptyInput);
    }
    for (idx, sample) in samples.iter().enumerate() {
        if !sample.is_finite() {
            return Err(EmbeddingError::ConfigError {
                message: format!("sample[{idx}] must be finite"),
            });
        }
    }
    let n = samples.len() as f32;
    let mean = samples.iter().sum::<f32>() / n;
    let variance = samples
        .iter()
        .map(|sample| (*sample - mean).powi(2))
        .sum::<f32>()
        / n;
    let stddev = variance.sqrt();
    let rms = (samples.iter().map(|sample| sample.powi(2)).sum::<f32>() / n).sqrt();
    let min = samples.iter().copied().fold(f32::INFINITY, f32::min);
    let max = samples.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let abs_mean = samples.iter().map(|sample| sample.abs()).sum::<f32>() / n;
    let zcr = zero_crossing_rate(samples);
    let slope = if samples.len() < 2 {
        0.0
    } else {
        (samples[samples.len() - 1] - samples[0]) / (samples.len() - 1) as f32
    };
    Ok(vec![
        mean,
        stddev,
        rms,
        min,
        max,
        max - min,
        abs_mean,
        zcr,
        slope,
        samples[0],
        samples[samples.len() - 1],
    ])
}

fn first_difference(samples: &[f32]) -> Vec<f32> {
    if samples.len() < 2 {
        return vec![0.0];
    }
    samples.windows(2).map(|pair| pair[1] - pair[0]).collect()
}

fn zero_crossing_rate(samples: &[f32]) -> f32 {
    if samples.len() < 2 {
        return 0.0;
    }
    let crossings = samples
        .windows(2)
        .filter(|pair| (pair[0] >= 0.0 && pair[1] < 0.0) || (pair[0] < 0.0 && pair[1] >= 0.0))
        .count();
    crossings as f32 / (samples.len() - 1) as f32
}

fn periodicity_score(samples: &[f32]) -> f32 {
    if samples.len() < 8 {
        return 0.0;
    }
    let mean = samples.iter().sum::<f32>() / samples.len() as f32;
    let centered: Vec<f32> = samples.iter().map(|sample| *sample - mean).collect();
    let mut best = 0.0f32;
    let max_lag = (samples.len() / 3).max(2);
    for lag in 2..=max_lag {
        let mut dot = 0.0f32;
        let mut a_norm = 0.0f32;
        let mut b_norm = 0.0f32;
        for idx in 0..(centered.len() - lag) {
            let a = centered[idx];
            let b = centered[idx + lag];
            dot += a * b;
            a_norm += a * a;
            b_norm += b * b;
        }
        let denom = (a_norm * b_norm).sqrt();
        if denom > 1e-6 {
            best = best.max((dot / denom).max(0.0));
        }
    }
    best.clamp(0.0, 1.0)
}

fn frequency_energy(samples: &[f32], sample_rate_hz: f32, frequency_hz: f32) -> f32 {
    if samples.is_empty() || sample_rate_hz <= 0.0 {
        return 0.0;
    }
    let mut re = 0.0f32;
    let mut im = 0.0f32;
    for (idx, sample) in samples.iter().enumerate() {
        let phase = 2.0 * PI * frequency_hz * idx as f32 / sample_rate_hz;
        re += sample * phase.cos();
        im -= sample * phase.sin();
    }
    (re * re + im * im).sqrt() / samples.len() as f32
}

fn hashed_text_features(content: &str, bins: usize) -> Vec<f32> {
    let mut out = vec![0.0; bins];
    for token in content.split(|ch: char| !ch.is_ascii_alphanumeric()) {
        if token.is_empty() {
            continue;
        }
        let mut hash = 0xcbf29ce484222325u64;
        for byte in token.as_bytes() {
            hash ^= u64::from(byte.to_ascii_lowercase());
            hash = hash.wrapping_mul(0x100000001b3);
        }
        let idx = (hash as usize) % bins;
        out[idx] += 1.0;
    }
    let norm = out.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 1e-6 {
        for value in &mut out {
            *value /= norm;
        }
    }
    out
}

fn project_features(features: &[f32], output_dim: usize, seed: u64) -> Vec<f32> {
    let mut safe_features = if features.is_empty() {
        vec![0.0]
    } else {
        features.to_vec()
    };
    for value in &mut safe_features {
        if !value.is_finite() {
            *value = 0.0;
        }
    }
    let denom = safe_features.len().max(1) as f32;
    let mut out = Vec::with_capacity(output_dim);
    for idx in 0..output_dim {
        let mut acc = 0.0f32;
        for (feature_idx, feature) in safe_features.iter().enumerate() {
            let phase = (seed as f32 + 1.0)
                * (idx as f32 + 1.0)
                * (feature_idx as f32 + 1.0)
                * 0.017_453_292;
            acc += feature * phase.sin() + (feature * 0.5) * phase.cos();
        }
        out.push((acc / denom).tanh());
    }
    l2_normalize(&mut out);
    out
}

fn l2_normalize(values: &mut [f32]) {
    let norm = values.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 1e-6 {
        for value in values {
            *value /= norm;
        }
    }
}

fn validate_vector(vector: &[f32], expected_dim: usize) -> EmbeddingResult<()> {
    if vector.len() != expected_dim {
        return Err(EmbeddingError::InvalidDimension {
            expected: expected_dim,
            actual: vector.len(),
        });
    }
    for (idx, value) in vector.iter().enumerate() {
        if !value.is_finite() {
            return Err(EmbeddingError::InvalidValue {
                index: idx,
                value: *value,
            });
        }
    }
    Ok(())
}

fn mean_existing<const N: usize>(values: [Option<f32>; N]) -> f32 {
    let mut sum = 0.0f32;
    let mut count = 0usize;
    for value in values.into_iter().flatten() {
        sum += value;
        count += 1;
    }
    if count == 0 {
        0.0
    } else {
        sum / count as f32
    }
}

fn sine_series(len: usize, cycles: f32, amplitude: f32) -> Vec<f32> {
    (0..len)
        .map(|idx| amplitude * (2.0 * PI * cycles * idx as f32 / len as f32).sin())
        .collect()
}

fn noisy_series(len: usize, amplitude: f32, noise: f32) -> Vec<f32> {
    (0..len)
        .map(|idx| {
            let base = amplitude * (2.0 * PI * 7.0 * idx as f32 / len as f32).sin();
            let jitter = noise * ((idx * 17 % 29) as f32 / 29.0 - 0.5);
            base + jitter
        })
        .collect()
}

fn low_ramp_series(len: usize, base: f32, ramp: f32) -> Vec<f32> {
    (0..len)
        .map(|idx| base + ramp * idx as f32 / len.max(1) as f32)
        .collect()
}

fn mixed_eeg_series(len: usize, sample_rate: f32, f1: f32, a1: f32, f2: f32, a2: f32) -> Vec<f32> {
    (0..len)
        .map(|idx| {
            let t = idx as f32 / sample_rate;
            a1 * (2.0 * PI * f1 * t).sin() + a2 * (2.0 * PI * f2 * t).sin()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ModelId;
    use context_graph_core::learner::LEARNER_MODALITY_COUNT;

    #[test]
    fn learner_embedder_registry_counts_to_utl_twenty() {
        let specs = learner_embedder_specs();
        assert_eq!(specs.len(), LEARNER_EMBEDDER_COUNT);
        assert_eq!(
            UTL_CONTENT_EMBEDDERS_BEFORE_LEARNER_STATE + specs.len(),
            UTL_PLANNED_TOTAL_EMBEDDERS
        );
        assert_eq!(ModelId::production().len(), 14);
        assert_eq!(LEARNER_MODALITY_COUNT, 7);
        for (idx, spec) in specs.iter().enumerate() {
            assert_eq!(spec.slot_number as usize, 15 + idx);
            assert_eq!(spec.output_dimension, spec.slot.output_dimension());
        }
    }

    #[test]
    fn checked_in_calibration_manifest_matches_registry() {
        let manifest: serde_json::Value = serde_json::from_str(include_str!(
            "../../fixtures/utl_calibration/dataset_manifest.json"
        ))
        .unwrap();
        assert_eq!(
            manifest["planned_total_embedders"].as_u64(),
            Some(UTL_PLANNED_TOTAL_EMBEDDERS as u64)
        );
        assert_eq!(
            manifest["learner_embedder_slots"].as_array().map(Vec::len),
            Some(LEARNER_EMBEDDER_COUNT)
        );
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../fixtures/utl_calibration/phase0_synthetic_fixture.json"
        ))
        .unwrap();
        assert_eq!(
            fixture["samples"][0]["expected"]["state_vector_len"].as_u64(),
            Some(4032)
        );
    }

    #[test]
    fn synthetic_fixture_embeds_all_e15_e21_slots() {
        let fixture = synthetic_calibration_fixture();
        let sample = &fixture.samples[0];
        let outputs = embed_calibration_sample(sample).unwrap();
        assert_eq!(outputs.len(), LEARNER_EMBEDDER_COUNT);
        for output in &outputs {
            assert_eq!(output.vector.len(), output.slot.output_dimension());
            assert!(!output.scalars.is_empty());
            assert!(output.vector.iter().all(|value| value.is_finite()));
        }
        let state = state_vector_from_outputs(
            Uuid::from_u128(0x11111111_1111_4111_8111_111111111111),
            1_700_000_000,
            &outputs,
            BTreeMap::new(),
        )
        .unwrap();
        assert_eq!(state.values.len(), 4032);
        assert_eq!(
            state
                .context
                .get("learner_embedder_count")
                .map(String::as_str),
            Some("7")
        );
    }

    #[test]
    fn empty_signal_rejected_before_embedding() {
        let input = LearnerEmbedderInput::Samples {
            modality: LearnerModality::Ppg,
            samples: Vec::new(),
            sample_rate_hz: 64,
            channels: 1,
        };
        let err = embed_learner_signal(LearnerEmbedderSlot::E18Ppg, &input).unwrap_err();
        assert!(matches!(err, EmbeddingError::EmptyInput));
    }

    #[test]
    fn modality_mismatch_rejected() {
        let input = LearnerEmbedderInput::Text {
            content: "calm text".into(),
        };
        let err = embed_learner_signal(LearnerEmbedderSlot::E15AffectSpeech, &input).unwrap_err();
        assert!(matches!(err, EmbeddingError::ConfigError { .. }));
    }

    #[test]
    fn nonfinite_signal_rejected() {
        let input = LearnerEmbedderInput::Samples {
            modality: LearnerModality::Eda,
            samples: vec![0.1, f32::NAN],
            sample_rate_hz: 4,
            channels: 1,
        };
        let err = embed_learner_signal(LearnerEmbedderSlot::E19Eda, &input).unwrap_err();
        assert!(matches!(err, EmbeddingError::ConfigError { .. }));
    }

    #[test]
    fn asset_preflight_reports_missing_files_without_fallback() {
        let models = tempfile::TempDir::new().unwrap();
        let calibration = tempfile::TempDir::new().unwrap();
        let report = preflight_learner_assets(models.path(), calibration.path()).unwrap();
        println!(
            "ASSET PREFLIGHT ready={} missing_count={} source_of_truth={}",
            report.ready, report.missing_count, report.source_of_truth
        );
        assert!(!report.ready);
        assert!(report.missing_count > 0);
        assert!(report
            .model_assets
            .iter()
            .all(|asset| asset.files.iter().any(|file| file.present_path.is_none())));
        assert!(report.calibration_assets.iter().all(|asset| !asset.ready));
    }
}
