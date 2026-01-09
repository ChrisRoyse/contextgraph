//! Tests for Domain enum.

use super::domain::Domain;

#[test]
fn test_default_is_general() {
    let domain = Domain::default();
    assert_eq!(domain, Domain::General, "Default domain must be General");
}

#[test]
fn test_description_non_empty_for_all_variants() {
    for domain in Domain::all() {
        let desc = domain.description();
        assert!(
            !desc.is_empty(),
            "Description for {:?} must not be empty",
            domain
        );
        assert!(
            desc.len() > 10,
            "Description for {:?} should be meaningful",
            domain
        );
    }
}

#[test]
fn test_code_description_mentions_precision() {
    assert!(Domain::Code
        .description()
        .to_lowercase()
        .contains("precision"));
}

#[test]
fn test_legal_description_mentions_reasoning() {
    assert!(Domain::Legal
        .description()
        .to_lowercase()
        .contains("reasoning"));
}

#[test]
fn test_medical_description_mentions_causal() {
    assert!(Domain::Medical
        .description()
        .to_lowercase()
        .contains("causal"));
}

#[test]
fn test_creative_description_mentions_exploration() {
    assert!(Domain::Creative
        .description()
        .to_lowercase()
        .contains("exploration"));
}

#[test]
fn test_research_description_mentions_balanced() {
    assert!(Domain::Research
        .description()
        .to_lowercase()
        .contains("balanced"));
}

#[test]
fn test_general_description_mentions_default() {
    assert!(Domain::General
        .description()
        .to_lowercase()
        .contains("default"));
}

#[test]
fn test_all_returns_6_variants() {
    let all = Domain::all();
    assert_eq!(all.len(), 6, "Domain::all() must return exactly 6 variants");
}

#[test]
fn test_all_contains_all_variants() {
    let all = Domain::all();
    assert!(all.contains(&Domain::Code));
    assert!(all.contains(&Domain::Legal));
    assert!(all.contains(&Domain::Medical));
    assert!(all.contains(&Domain::Creative));
    assert!(all.contains(&Domain::Research));
    assert!(all.contains(&Domain::General));
}

#[test]
fn test_all_order_matches_definition() {
    let all = Domain::all();
    assert_eq!(all[0], Domain::Code);
    assert_eq!(all[1], Domain::Legal);
    assert_eq!(all[2], Domain::Medical);
    assert_eq!(all[3], Domain::Creative);
    assert_eq!(all[4], Domain::Research);
    assert_eq!(all[5], Domain::General);
}

#[test]
fn test_display_code() {
    assert_eq!(Domain::Code.to_string(), "code");
}

#[test]
fn test_display_legal() {
    assert_eq!(Domain::Legal.to_string(), "legal");
}

#[test]
fn test_display_medical() {
    assert_eq!(Domain::Medical.to_string(), "medical");
}

#[test]
fn test_display_creative() {
    assert_eq!(Domain::Creative.to_string(), "creative");
}

#[test]
fn test_display_research() {
    assert_eq!(Domain::Research.to_string(), "research");
}

#[test]
fn test_display_general() {
    assert_eq!(Domain::General.to_string(), "general");
}

#[test]
fn test_display_all_lowercase() {
    for domain in Domain::all() {
        let s = domain.to_string();
        assert_eq!(
            s,
            s.to_lowercase(),
            "Display for {:?} must be lowercase",
            domain
        );
    }
}

#[test]
fn test_serde_serializes_to_lowercase() {
    let domain = Domain::Code;
    let json = serde_json::to_string(&domain).expect("serialize failed");
    assert_eq!(json, r#""code""#, "Serde must serialize to lowercase");
}

#[test]
fn test_serde_deserializes_from_lowercase() {
    let domain: Domain = serde_json::from_str(r#""legal""#).expect("deserialize failed");
    assert_eq!(domain, Domain::Legal);
}

#[test]
fn test_serde_roundtrip_all_variants() {
    for domain in Domain::all() {
        let json = serde_json::to_string(&domain).expect("serialize failed");
        let restored: Domain = serde_json::from_str(&json).expect("deserialize failed");
        assert_eq!(restored, domain, "Serde roundtrip failed for {:?}", domain);
    }
}

#[test]
fn test_serde_snake_case_format() {
    for domain in Domain::all() {
        let json = serde_json::to_string(&domain).unwrap();
        let value = json.trim_matches('"');
        assert!(
            value.chars().all(|c| c.is_lowercase() || c == '_'),
            "Serde output for {:?} must be snake_case: {}",
            domain,
            value
        );
    }
}

#[test]
fn test_clone() {
    let domain = Domain::Medical;
    let cloned = domain;
    assert_eq!(domain, cloned);
}

#[test]
fn test_copy() {
    let domain = Domain::Creative;
    let copied = domain;
    assert_eq!(domain, copied);
    let _still_valid = domain;
}

#[test]
fn test_debug_format() {
    let debug = format!("{:?}", Domain::Research);
    assert!(debug.contains("Research"), "Debug should show variant name");
}

#[test]
fn test_partial_eq() {
    assert_eq!(Domain::Code, Domain::Code);
    assert_ne!(Domain::Code, Domain::Legal);
}

#[test]
fn test_hash_in_collection() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(Domain::Code);
    set.insert(Domain::Legal);
    set.insert(Domain::Code);
    assert_eq!(set.len(), 2, "Hash must properly deduplicate");
}

#[test]
fn test_all_variants_unique() {
    use std::collections::HashSet;
    let all = Domain::all();
    let unique: HashSet<_> = all.iter().collect();
    assert_eq!(unique.len(), 6, "All variants must be unique");
}

#[test]
fn test_default_is_in_all() {
    let default = Domain::default();
    assert!(Domain::all().contains(&default), "Default must be in all()");
}
