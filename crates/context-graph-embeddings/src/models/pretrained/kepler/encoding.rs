//! Entity and relation encoding utilities for KEPLER.
//!
//! Provides methods to format entity names and relations for embedding.

use super::KeplerModel;

impl KeplerModel {
    /// Encode an entity with optional type context.
    ///
    /// Creates a text representation suitable for embedding with KEPLER.
    /// The entity type is uppercased and wrapped in brackets.
    ///
    /// # Arguments
    /// * `name` - The entity name (e.g., "Paris", "Anthropic")
    /// * `entity_type` - Optional entity type (e.g., "LOCATION", "ORG")
    ///
    /// # Returns
    /// A string formatted as "[TYPE] name" if type provided, otherwise just "name".
    ///
    /// # Examples
    /// ```rust
    /// use context_graph_embeddings::models::kepler::KeplerModel;
    ///
    /// let text = KeplerModel::encode_entity("Paris", Some("location"));
    /// assert_eq!(text, "[LOCATION] Paris");
    ///
    /// let text = KeplerModel::encode_entity("France", None);
    /// assert_eq!(text, "France");
    /// ```
    pub fn encode_entity(name: &str, entity_type: Option<&str>) -> String {
        match entity_type {
            Some(etype) => format!("[{}] {}", etype.to_uppercase(), name),
            None => name.to_string(),
        }
    }

    /// Encode a relation for TransE-style operations.
    ///
    /// Converts relation predicates into natural language form by replacing
    /// underscores with spaces.
    ///
    /// # Arguments
    /// * `relation` - The relation predicate (e.g., "capital_of", "is_friend_of")
    ///
    /// # Returns
    /// A string with underscores replaced by spaces.
    ///
    /// # Examples
    /// ```rust
    /// use context_graph_embeddings::models::kepler::KeplerModel;
    ///
    /// let text = KeplerModel::encode_relation("capital_of");
    /// assert_eq!(text, "capital of");
    ///
    /// let text = KeplerModel::encode_relation("is_friend_of");
    /// assert_eq!(text, "is friend of");
    /// ```
    pub fn encode_relation(relation: &str) -> String {
        relation.replace('_', " ")
    }

    /// Encode a Wikidata-style relation.
    ///
    /// KEPLER was trained on Wikidata5M, so it understands Wikidata relation
    /// patterns. This method formats relations in the style KEPLER expects.
    ///
    /// # Arguments
    /// * `relation` - The relation predicate (e.g., "P36" for capital, "P17" for country)
    /// * `label` - Optional human-readable label for the relation
    ///
    /// # Returns
    /// A string suitable for embedding as a relation.
    ///
    /// # Examples
    /// ```rust
    /// use context_graph_embeddings::models::kepler::KeplerModel;
    ///
    /// // With label
    /// let text = KeplerModel::encode_wikidata_relation("P36", Some("capital"));
    /// assert_eq!(text, "capital");
    ///
    /// // Without label (uses property ID)
    /// let text = KeplerModel::encode_wikidata_relation("P36", None);
    /// assert_eq!(text, "P36");
    /// ```
    pub fn encode_wikidata_relation(relation: &str, label: Option<&str>) -> String {
        match label {
            Some(lbl) => lbl.to_string(),
            None => relation.to_string(),
        }
    }

    /// Encode a knowledge triple as text for embedding.
    ///
    /// Creates a natural language representation of a (head, relation, tail) triple.
    /// Useful for embedding entire facts.
    ///
    /// # Arguments
    /// * `head` - Head entity name
    /// * `relation` - Relation predicate
    /// * `tail` - Tail entity name
    ///
    /// # Returns
    /// A natural language sentence representing the triple.
    ///
    /// # Examples
    /// ```rust
    /// use context_graph_embeddings::models::kepler::KeplerModel;
    ///
    /// let text = KeplerModel::encode_triple("Paris", "capital_of", "France");
    /// assert_eq!(text, "Paris capital of France");
    /// ```
    pub fn encode_triple(head: &str, relation: &str, tail: &str) -> String {
        format!("{} {} {}", head, Self::encode_relation(relation), tail)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_entity_with_type() {
        let text = KeplerModel::encode_entity("Paris", Some("location"));
        assert_eq!(text, "[LOCATION] Paris");
    }

    #[test]
    fn test_encode_entity_without_type() {
        let text = KeplerModel::encode_entity("France", None);
        assert_eq!(text, "France");
    }

    #[test]
    fn test_encode_relation() {
        assert_eq!(KeplerModel::encode_relation("capital_of"), "capital of");
        assert_eq!(KeplerModel::encode_relation("is_friend_of"), "is friend of");
        assert_eq!(KeplerModel::encode_relation("located_in"), "located in");
    }

    #[test]
    fn test_encode_triple() {
        let text = KeplerModel::encode_triple("Paris", "capital_of", "France");
        assert_eq!(text, "Paris capital of France");
    }

    #[test]
    fn test_encode_wikidata_relation() {
        assert_eq!(
            KeplerModel::encode_wikidata_relation("P36", Some("capital")),
            "capital"
        );
        assert_eq!(KeplerModel::encode_wikidata_relation("P36", None), "P36");
    }
}
