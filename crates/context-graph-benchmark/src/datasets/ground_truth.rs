//! Ground truth management for benchmark evaluation.
//!
//! This module provides utilities for managing relevance labels and
//! ground truth data for retrieval and clustering evaluation.

use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use serde::{Deserialize, Serialize};

/// Ground truth data for benchmark evaluation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GroundTruth {
    /// Document to topic mapping.
    pub document_topics: HashMap<Uuid, usize>,

    /// Query to relevant documents mapping.
    pub query_relevance: HashMap<Uuid, RelevanceLabels>,

    /// Divergence labels for queries.
    pub divergence_labels: HashMap<Uuid, bool>,

    /// Topic names (for reporting).
    pub topic_names: Vec<String>,
}

/// Relevance labels for a single query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelevanceLabels {
    /// Binary relevance: set of relevant document IDs.
    pub relevant_docs: HashSet<Uuid>,

    /// Graded relevance (optional): document ID to relevance grade (0-3).
    pub graded_relevance: HashMap<Uuid, u8>,

    /// Topic this query is associated with.
    pub topic: usize,
}

impl RelevanceLabels {
    /// Create with binary relevance only.
    pub fn binary(relevant_docs: HashSet<Uuid>, topic: usize) -> Self {
        Self {
            relevant_docs,
            graded_relevance: HashMap::new(),
            topic,
        }
    }

    /// Create with graded relevance.
    pub fn graded(graded_relevance: HashMap<Uuid, u8>, topic: usize) -> Self {
        // Binary relevance is derived from graded (any grade > 0 is relevant)
        let relevant_docs: HashSet<Uuid> = graded_relevance
            .iter()
            .filter(|(_, &grade)| grade > 0)
            .map(|(id, _)| *id)
            .collect();

        Self {
            relevant_docs,
            graded_relevance,
            topic,
        }
    }

    /// Get relevance grade for a document (0 if not labeled).
    pub fn get_grade(&self, doc_id: &Uuid) -> u8 {
        self.graded_relevance.get(doc_id).copied().unwrap_or(0)
    }

    /// Check if a document is relevant (binary).
    pub fn is_relevant(&self, doc_id: &Uuid) -> bool {
        self.relevant_docs.contains(doc_id)
    }

    /// Get number of relevant documents.
    pub fn num_relevant(&self) -> usize {
        self.relevant_docs.len()
    }
}

impl GroundTruth {
    /// Create new ground truth.
    pub fn new(document_topics: HashMap<Uuid, usize>, topic_names: Vec<String>) -> Self {
        Self {
            document_topics,
            query_relevance: HashMap::new(),
            divergence_labels: HashMap::new(),
            topic_names,
        }
    }

    /// Add query relevance labels.
    pub fn add_query_relevance(&mut self, query_id: Uuid, labels: RelevanceLabels) {
        self.query_relevance.insert(query_id, labels);
    }

    /// Add divergence label for a query.
    pub fn add_divergence_label(&mut self, query_id: Uuid, is_divergent: bool) {
        self.divergence_labels.insert(query_id, is_divergent);
    }

    /// Get topic for a document.
    pub fn get_document_topic(&self, doc_id: &Uuid) -> Option<usize> {
        self.document_topics.get(doc_id).copied()
    }

    /// Get all documents in a topic.
    pub fn get_topic_documents(&self, topic: usize) -> HashSet<Uuid> {
        self.document_topics
            .iter()
            .filter(|(_, &t)| t == topic)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get relevance labels for a query.
    pub fn get_query_relevance(&self, query_id: &Uuid) -> Option<&RelevanceLabels> {
        self.query_relevance.get(query_id)
    }

    /// Check if a query is divergent.
    pub fn is_divergent(&self, query_id: &Uuid) -> bool {
        self.divergence_labels
            .get(query_id)
            .copied()
            .unwrap_or(false)
    }

    /// Get number of documents.
    pub fn num_documents(&self) -> usize {
        self.document_topics.len()
    }

    /// Get number of topics.
    pub fn num_topics(&self) -> usize {
        self.topic_names.len()
    }

    /// Get number of queries.
    pub fn num_queries(&self) -> usize {
        self.query_relevance.len()
    }

    /// Get cluster labels for all documents (for clustering evaluation).
    ///
    /// Returns (doc_ids, labels) where labels[i] is the topic for doc_ids[i].
    pub fn get_cluster_labels(&self) -> (Vec<Uuid>, Vec<usize>) {
        let doc_ids: Vec<Uuid> = self.document_topics.keys().copied().collect();
        let labels: Vec<usize> = doc_ids.iter().map(|id| self.document_topics[id]).collect();

        (doc_ids, labels)
    }

    /// Convert query relevance to format suitable for metric computation.
    pub fn get_retrieval_ground_truth(&self) -> Vec<(Uuid, HashSet<Uuid>)> {
        self.query_relevance
            .iter()
            .map(|(query_id, labels)| (*query_id, labels.relevant_docs.clone()))
            .collect()
    }

    /// Get divergence labels as Vec<bool> matching query order.
    pub fn get_divergence_labels(&self, query_ids: &[Uuid]) -> Vec<bool> {
        query_ids
            .iter()
            .map(|id| self.divergence_labels.get(id).copied().unwrap_or(false))
            .collect()
    }

    /// Validate consistency of ground truth.
    pub fn validate(&self) -> Result<(), String> {
        // Check all queries reference valid documents
        for (query_id, labels) in &self.query_relevance {
            for doc_id in &labels.relevant_docs {
                if !self.document_topics.contains_key(doc_id) {
                    return Err(format!(
                        "Query {} references unknown document {}",
                        query_id, doc_id
                    ));
                }
            }
        }

        // Check topic indices are valid
        let max_topic = self.topic_names.len();
        for (doc_id, &topic) in &self.document_topics {
            if topic >= max_topic {
                return Err(format!(
                    "Document {} has invalid topic {} (max {})",
                    doc_id,
                    topic,
                    max_topic - 1
                ));
            }
        }

        Ok(())
    }
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;

    #[test]
    fn test_ground_truth_creation() {
        let mut gt = GroundTruth::new(
            HashMap::new(),
            vec!["Topic0".to_string(), "Topic1".to_string()],
        );

        let doc1 = Uuid::new_v4();
        let doc2 = Uuid::new_v4();

        gt.document_topics.insert(doc1, 0);
        gt.document_topics.insert(doc2, 1);

        assert_eq!(gt.get_document_topic(&doc1), Some(0));
        assert_eq!(gt.get_document_topic(&doc2), Some(1));
    }

    #[test]
    fn test_relevance_labels() {
        let doc1 = Uuid::new_v4();
        let doc2 = Uuid::new_v4();

        let mut relevant = HashSet::new();
        relevant.insert(doc1);
        relevant.insert(doc2);

        let labels = RelevanceLabels::binary(relevant, 0);

        assert!(labels.is_relevant(&doc1));
        assert!(labels.is_relevant(&doc2));
        assert_eq!(labels.num_relevant(), 2);
    }

    #[test]
    fn test_graded_relevance() {
        let doc1 = Uuid::new_v4();
        let doc2 = Uuid::new_v4();
        let doc3 = Uuid::new_v4();

        let mut graded = HashMap::new();
        graded.insert(doc1, 3); // Highly relevant
        graded.insert(doc2, 1); // Marginally relevant
        graded.insert(doc3, 0); // Not relevant

        let labels = RelevanceLabels::graded(graded, 0);

        assert!(labels.is_relevant(&doc1));
        assert!(labels.is_relevant(&doc2));
        assert!(!labels.is_relevant(&doc3));
        assert_eq!(labels.get_grade(&doc1), 3);
        assert_eq!(labels.get_grade(&doc2), 1);
        assert_eq!(labels.get_grade(&doc3), 0);
    }
}
