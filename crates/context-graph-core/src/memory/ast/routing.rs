use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

use super::{EntityType, Language};

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

    pub fn slug(self) -> &'static str {
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

    pub const fn projected_dimension(self) -> usize {
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

    pub const fn is_content(self) -> bool {
        matches!(
            self,
            Self::E1
                | Self::E2
                | Self::E3
                | Self::E4
                | Self::E6
                | Self::E7
                | Self::E8
                | Self::E9
                | Self::E10
                | Self::E12
                | Self::E13
                | Self::E14
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirectInstrument {
    EAst,
    ECfg,
    EDataFlow,
    EDiff,
}

impl DirectInstrument {
    pub const fn slug(self) -> &'static str {
        match self {
            Self::EAst => "e_ast",
            Self::ECfg => "e_cfg",
            Self::EDataFlow => "e_data_flow",
            Self::EDiff => "e_diff",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingKey {
    Entity(EntityType),
    FunctionSignature,
    TestAssertion,
    DiffHunk,
    AstNodeSequence,
    CfgBasicBlock,
    DefUseEdge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingResult {
    Embedders(BTreeSet<EmbedderId>),
    HandledByInstrument(DirectInstrument),
}

impl RoutingResult {
    pub fn embedders(&self) -> Option<&BTreeSet<EmbedderId>> {
        match self {
            Self::Embedders(ids) => Some(ids),
            Self::HandledByInstrument(_) => None,
        }
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoutingError {
    #[error("embedder routing table is missing key={key:?}, language={language:?}")]
    Missing {
        key: RoutingKey,
        language: Option<Language>,
    },
    #[error("entity route resolved to an internal instrument: key={key:?}, language={language:?}, instrument={instrument:?}")]
    UnexpectedInstrument {
        key: RoutingKey,
        language: Option<Language>,
        instrument: DirectInstrument,
    },
}

impl RoutingError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Missing { .. } => "MEJEPA_EMBED_ROUTING_MISSING",
            Self::UnexpectedInstrument { .. } => "MEJEPA_EMBED_ROUTING_UNEXPECTED_INSTRUMENT",
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct RoutingTableEntry {
    key: RoutingKey,
    language: Option<Language>,
    result: RoutingTableValue,
}

#[derive(Debug, Clone, Copy)]
enum RoutingTableValue {
    Embedders(&'static [EmbedderId]),
    HandledByInstrument(DirectInstrument),
}

impl RoutingTableValue {
    fn to_result(self) -> RoutingResult {
        match self {
            Self::Embedders(ids) => RoutingResult::Embedders(ids.iter().copied().collect()),
            Self::HandledByInstrument(slot) => RoutingResult::HandledByInstrument(slot),
        }
    }
}

const E7_E14: &[EmbedderId] = &[EmbedderId::E7, EmbedderId::E14];
const E1_E10: &[EmbedderId] = &[EmbedderId::E1, EmbedderId::E10];
const E1_E7: &[EmbedderId] = &[EmbedderId::E1, EmbedderId::E7];
const E1_E8: &[EmbedderId] = &[EmbedderId::E1, EmbedderId::E8];
const E8_E13: &[EmbedderId] = &[EmbedderId::E8, EmbedderId::E13];
const E7_E12: &[EmbedderId] = &[EmbedderId::E7, EmbedderId::E12];

const ROUTING_TABLE: &[RoutingTableEntry] = &[
    RoutingTableEntry {
        key: RoutingKey::Entity(EntityType::Function),
        language: None,
        result: RoutingTableValue::Embedders(E7_E14),
    },
    RoutingTableEntry {
        key: RoutingKey::Entity(EntityType::Method),
        language: None,
        result: RoutingTableValue::Embedders(E7_E14),
    },
    RoutingTableEntry {
        key: RoutingKey::FunctionSignature,
        language: None,
        result: RoutingTableValue::Embedders(E1_E10),
    },
    RoutingTableEntry {
        key: RoutingKey::Entity(EntityType::TestFunction),
        language: None,
        result: RoutingTableValue::Embedders(E1_E7),
    },
    RoutingTableEntry {
        key: RoutingKey::Entity(EntityType::Class),
        language: None,
        result: RoutingTableValue::Embedders(E7_E14),
    },
    RoutingTableEntry {
        key: RoutingKey::Entity(EntityType::Struct),
        language: None,
        result: RoutingTableValue::Embedders(E7_E14),
    },
    RoutingTableEntry {
        key: RoutingKey::Entity(EntityType::Enum),
        language: None,
        result: RoutingTableValue::Embedders(E7_E14),
    },
    RoutingTableEntry {
        key: RoutingKey::Entity(EntityType::TraitOrInterface),
        language: None,
        result: RoutingTableValue::Embedders(E7_E14),
    },
    RoutingTableEntry {
        key: RoutingKey::Entity(EntityType::Impl),
        language: None,
        result: RoutingTableValue::Embedders(E7_E14),
    },
    RoutingTableEntry {
        key: RoutingKey::Entity(EntityType::Module),
        language: None,
        result: RoutingTableValue::Embedders(E1_E8),
    },
    RoutingTableEntry {
        key: RoutingKey::Entity(EntityType::Namespace),
        language: None,
        result: RoutingTableValue::Embedders(E1_E8),
    },
    RoutingTableEntry {
        key: RoutingKey::Entity(EntityType::Import),
        language: None,
        result: RoutingTableValue::Embedders(E8_E13),
    },
    RoutingTableEntry {
        key: RoutingKey::Entity(EntityType::CommentBlock),
        language: None,
        result: RoutingTableValue::Embedders(E1_E10),
    },
    RoutingTableEntry {
        key: RoutingKey::Entity(EntityType::Docstring),
        language: None,
        result: RoutingTableValue::Embedders(E1_E10),
    },
    RoutingTableEntry {
        key: RoutingKey::TestAssertion,
        language: None,
        result: RoutingTableValue::Embedders(E7_E12),
    },
    RoutingTableEntry {
        key: RoutingKey::DiffHunk,
        language: None,
        result: RoutingTableValue::HandledByInstrument(DirectInstrument::EDiff),
    },
    RoutingTableEntry {
        key: RoutingKey::AstNodeSequence,
        language: None,
        result: RoutingTableValue::HandledByInstrument(DirectInstrument::EAst),
    },
    RoutingTableEntry {
        key: RoutingKey::CfgBasicBlock,
        language: None,
        result: RoutingTableValue::HandledByInstrument(DirectInstrument::ECfg),
    },
    RoutingTableEntry {
        key: RoutingKey::DefUseEdge,
        language: None,
        result: RoutingTableValue::HandledByInstrument(DirectInstrument::EDataFlow),
    },
];

pub fn routing_table_entries() -> impl Iterator<Item = (RoutingKey, Option<Language>, RoutingResult)>
{
    ROUTING_TABLE
        .iter()
        .map(|entry| (entry.key, entry.language, entry.result.to_result()))
}

pub fn route_for_entity_type(
    entity_type: EntityType,
    language: Option<Language>,
) -> Result<RoutingResult, RoutingError> {
    route_for_key(RoutingKey::Entity(entity_type), language)
}

pub fn route_for_key(
    key: RoutingKey,
    language: Option<Language>,
) -> Result<RoutingResult, RoutingError> {
    find_route(key, language)
        .or_else(|| language.and_then(|_| find_route(key, None)))
        .ok_or(RoutingError::Missing { key, language })
}

fn find_route(key: RoutingKey, language: Option<Language>) -> Option<RoutingResult> {
    ROUTING_TABLE
        .iter()
        .find(|entry| entry.key == key && entry.language == language)
        .map(|entry| entry.result.to_result())
}

pub fn try_route_to_embedders(
    entity_type: EntityType,
    language: Option<Language>,
) -> Result<BTreeSet<EmbedderId>, RoutingError> {
    let key = RoutingKey::Entity(entity_type);
    match route_for_key(key, language)? {
        RoutingResult::Embedders(ids) => Ok(ids),
        RoutingResult::HandledByInstrument(instrument) => Err(RoutingError::UnexpectedInstrument {
            key,
            language,
            instrument,
        }),
    }
}

pub fn route_to_embedders(entity_type: EntityType) -> BTreeSet<EmbedderId> {
    try_route_to_embedders(entity_type, None)
        .expect("MEJEPA_EMBED_ROUTING_MISSING: static EntityType routing table is incomplete")
}
