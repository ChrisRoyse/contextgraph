use crate::embedder_id::EmbedderId;
use crate::error::{EmbedError, EmbedResult};
use crate::types::RoutingResult;
use context_graph_core::memory::ast::{
    route_for_entity_type as core_route_for_entity_type, EntityType, Language,
    RoutingResult as CoreRoutingResult,
};
use std::collections::BTreeSet;

pub fn route_for_entity_type(
    entity_type: EntityType,
    language: Language,
) -> EmbedResult<RoutingResult> {
    let core = core_route_for_entity_type(entity_type, Some(language)).map_err(|err| {
        EmbedError::RoutingMissing {
            language: language.slug().to_string(),
            entity_type: format!("{entity_type:?}"),
            message: err.to_string(),
            remediation: "extend the AST routing table so every Phase 1 language/entity pair has a deterministic route",
        }
    })?;
    match core {
        CoreRoutingResult::Embedders(ids) => {
            let mut embedders = BTreeSet::new();
            for id in ids {
                let embedder = convert_core_embedder(id);
                if embedder.is_retired()
                    || (matches!(embedder, EmbedderId::E11)
                        && !context_graph_core::weights::E11_ENTITY_ENABLED)
                {
                    return Err(EmbedError::RoutingMissing {
                        language: language.slug().to_string(),
                        entity_type: format!("{entity_type:?}"),
                        message: format!(
                            "core AST route attempted to use retired/disabled embedder {embedder}"
                        ),
                        remediation: "replace the route with active ME-JEPA embedders before loading the content set",
                    });
                }
                embedders.insert(embedder);
            }
            Ok(RoutingResult {
                language: language.slug().to_string(),
                entity_type: format!("{entity_type:?}"),
                embedders,
            })
        }
        CoreRoutingResult::HandledByInstrument(instrument) => Err(EmbedError::RoutingMissing {
            language: language.slug().to_string(),
            entity_type: format!("{entity_type:?}"),
            message: format!(
                "entity route unexpectedly resolved to direct instrument {instrument:?}"
            ),
            remediation: "only direct AST/CFG/DataFlow keys may route to direct instruments",
        }),
    }
}

pub fn routing_coverage() -> EmbedResult<Vec<RoutingResult>> {
    let mut rows = Vec::new();
    for language in Language::all() {
        for entity_type in EntityType::all() {
            rows.push(route_for_entity_type(entity_type, language)?);
        }
    }
    Ok(rows)
}

fn convert_core_embedder(id: context_graph_core::memory::ast::EmbedderId) -> EmbedderId {
    match id {
        context_graph_core::memory::ast::EmbedderId::E1 => EmbedderId::E1,
        context_graph_core::memory::ast::EmbedderId::E2 => EmbedderId::E2,
        context_graph_core::memory::ast::EmbedderId::E3 => EmbedderId::E3,
        context_graph_core::memory::ast::EmbedderId::E4 => EmbedderId::E4,
        context_graph_core::memory::ast::EmbedderId::E5 => EmbedderId::E5,
        context_graph_core::memory::ast::EmbedderId::E6 => EmbedderId::E6,
        context_graph_core::memory::ast::EmbedderId::E7 => EmbedderId::E7,
        context_graph_core::memory::ast::EmbedderId::E8 => EmbedderId::E8,
        context_graph_core::memory::ast::EmbedderId::E9 => EmbedderId::E9,
        context_graph_core::memory::ast::EmbedderId::E10 => EmbedderId::E10,
        context_graph_core::memory::ast::EmbedderId::E11 => EmbedderId::E11,
        context_graph_core::memory::ast::EmbedderId::E12 => EmbedderId::E12,
        context_graph_core::memory::ast::EmbedderId::E13 => EmbedderId::E13,
        context_graph_core::memory::ast::EmbedderId::E14 => EmbedderId::E14,
        context_graph_core::memory::ast::EmbedderId::E15 => EmbedderId::E15,
        context_graph_core::memory::ast::EmbedderId::E16 => EmbedderId::E16,
        context_graph_core::memory::ast::EmbedderId::E17 => EmbedderId::E17,
        context_graph_core::memory::ast::EmbedderId::E18 => EmbedderId::E18,
        context_graph_core::memory::ast::EmbedderId::E19 => EmbedderId::E19,
        context_graph_core::memory::ast::EmbedderId::E20 => EmbedderId::E20,
        context_graph_core::memory::ast::EmbedderId::E21 => EmbedderId::E21,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase1_routing_covers_13_entities_x_11_languages() {
        let rows = routing_coverage().unwrap();
        assert_eq!(rows.len(), 13 * 11);
        assert!(rows.iter().all(|row| !row.embedders.is_empty()));
    }
}
