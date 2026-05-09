use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use super::EntityType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbedderId {
    E1,
    E7,
    E8,
    E10,
    E11,
    E13,
    E14,
}

impl EmbedderId {
    pub fn slug(self) -> &'static str {
        match self {
            Self::E1 => "e1",
            Self::E7 => "e7",
            Self::E8 => "e8",
            Self::E10 => "e10",
            Self::E11 => "e11",
            Self::E13 => "e13",
            Self::E14 => "e14",
        }
    }
}

pub fn route_to_embedders(entity_type: EntityType) -> BTreeSet<EmbedderId> {
    use EmbedderId::*;
    use EntityType::*;

    match entity_type {
        Function | Method => BTreeSet::from([E7, E14]),
        Class | Struct | Enum | TraitOrInterface | Impl => BTreeSet::from([E7, E11]),
        Module | Namespace => BTreeSet::from([E1, E8]),
        TestFunction => BTreeSet::from([E1, E7]),
        Import => BTreeSet::from([E11, E13]),
        CommentBlock | Docstring => BTreeSet::from([E1, E10]),
    }
}
