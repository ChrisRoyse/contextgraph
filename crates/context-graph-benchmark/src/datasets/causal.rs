//! Causal dataset generation for benchmarking E5 embedder.
//!
//! This module generates synthetic datasets with known causal ground truth for
//! evaluating direction detection, asymmetric retrieval, and causal reasoning.
//!
//! ## Dataset Types
//!
//! - **Direction Detection Dataset**: Queries with known cause/effect directions
//! - **Causal Pairs Dataset**: Cause-effect pairs with intervention contexts
//! - **COPA Dataset**: Choice of Plausible Alternatives questions
//! - **Chain Dataset**: Multi-hop causal chains for traversal tests
//!
//! ## Ground Truth
//!
//! Each dataset includes:
//! - Direction labels for queries (Cause, Effect, Unknown)
//! - Causal pairs with strength and domain
//! - COPA questions with correct answers
//! - Ordered chains with intervention contexts

use rand::prelude::*;
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

use context_graph_core::causal::asymmetric::{CausalDirection, InterventionContext};

/// Configuration for causal dataset generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalDatasetConfig {
    /// Number of causal pairs to generate.
    pub num_causal_pairs: usize,

    /// Number of direction detection queries to generate.
    pub num_direction_queries: usize,

    /// Number of COPA questions to generate.
    pub num_copa_questions: usize,

    /// Number of causal chains to generate.
    pub num_chains: usize,

    /// Average chain length.
    pub avg_chain_length: usize,

    /// Random seed for reproducibility.
    pub seed: u64,

    /// Domains to include.
    pub domains: Vec<CausalDomain>,

    /// Ratio of cause vs effect queries.
    pub cause_effect_ratio: f64,
}

impl Default for CausalDatasetConfig {
    fn default() -> Self {
        Self {
            num_causal_pairs: 500,
            num_direction_queries: 200,
            num_copa_questions: 100,
            num_chains: 30,
            avg_chain_length: 4,
            seed: 42,
            domains: vec![
                CausalDomain::Programming,
                CausalDomain::Physics,
                CausalDomain::Economics,
                CausalDomain::Biology,
                CausalDomain::Psychology,
                CausalDomain::Engineering,
            ],
            cause_effect_ratio: 0.5,
        }
    }
}

/// Domain enum for multi-domain coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CausalDomain {
    /// Programming: Memory leaks, bugs, crashes.
    Programming,
    /// Physics: Force, temperature, pressure.
    Physics,
    /// Economics: Supply/demand, interest rates.
    Economics,
    /// Biology: Genes, disease, treatments.
    Biology,
    /// Psychology: Behavior, motivation.
    Psychology,
    /// Engineering: Failures, stress, fatigue.
    Engineering,
}

impl CausalDomain {
    /// Get all domains.
    pub fn all() -> Vec<Self> {
        vec![
            Self::Programming,
            Self::Physics,
            Self::Economics,
            Self::Biology,
            Self::Psychology,
            Self::Engineering,
        ]
    }

    /// Get cause templates for this domain.
    pub fn cause_templates(&self) -> Vec<&'static str> {
        match self {
            Self::Programming => vec![
                "Memory leak in {component}",
                "Race condition in {module}",
                "Null pointer dereference in {function}",
                "Buffer overflow in {buffer}",
                "Deadlock between {thread_a} and {thread_b}",
                "Uncaught exception in {handler}",
                "Integer overflow in {calculation}",
                "Use-after-free in {allocator}",
                "Stack overflow in {recursive_function}",
                "SQL injection in {endpoint}",
            ],
            Self::Physics => vec![
                "Increase in temperature by {degrees}",
                "Applied force of {newtons}N",
                "Pressure increase to {pascals}Pa",
                "Electric current of {amps}A",
                "Magnetic field of {tesla}T",
                "Acceleration of {mps2}m/s²",
                "Heat transfer of {watts}W",
                "Friction coefficient of {coeff}",
                "Voltage of {volts}V applied",
                "Gravitational pull of {force}N",
            ],
            Self::Economics => vec![
                "Interest rate increase by {rate}%",
                "Inflation rise to {inflation}%",
                "Supply shortage of {commodity}",
                "Demand surge for {product}",
                "Tax increase of {tax_rate}%",
                "Currency devaluation by {percent}%",
                "Trade tariff on {goods}",
                "Government spending increase of ${amount}",
                "Labor shortage in {sector}",
                "Raw material price spike",
            ],
            Self::Biology => vec![
                "Gene mutation in {gene}",
                "Pathogen exposure to {virus}",
                "Hormone {hormone} imbalance",
                "Nutrient deficiency of {nutrient}",
                "Antibiotic {antibiotic} administration",
                "Environmental toxin {toxin} exposure",
                "Cellular stress from {stressor}",
                "Immune response to {antigen}",
                "Enzyme {enzyme} inhibition",
                "DNA damage from {radiation}",
            ],
            Self::Psychology => vec![
                "Stress from {stressor}",
                "Sleep deprivation of {hours} hours",
                "Traumatic experience of {event}",
                "Social isolation for {duration}",
                "Cognitive overload from {task}",
                "Positive reinforcement of {behavior}",
                "Negative feedback on {performance}",
                "Environmental change to {setting}",
                "Motivational speech about {topic}",
                "Fear conditioning with {stimulus}",
            ],
            Self::Engineering => vec![
                "Material fatigue in {component}",
                "Structural stress of {force}N",
                "Thermal cycling of {cycles} cycles",
                "Corrosion in {environment}",
                "Vibration at {frequency}Hz",
                "Overload of {percent}% capacity",
                "Design flaw in {subsystem}",
                "Manufacturing defect in {part}",
                "Improper installation of {component}",
                "Environmental exposure to {condition}",
            ],
        }
    }

    /// Get effect templates for this domain.
    pub fn effect_templates(&self) -> Vec<&'static str> {
        match self {
            Self::Programming => vec![
                "Application crash after {duration}",
                "Data corruption in {structure}",
                "System hang for {seconds} seconds",
                "Memory exhaustion after {operations} ops",
                "Performance degradation of {percent}%",
                "Security vulnerability in {layer}",
                "Service unavailability for {minutes} minutes",
                "Incorrect output in {module}",
                "Resource contention in {resource}",
                "Cascading failure in {system}",
            ],
            Self::Physics => vec![
                "Thermal expansion of {material}",
                "Acceleration of {object}",
                "Phase transition to {state}",
                "Electromagnetic induction in {conductor}",
                "Wave propagation through {medium}",
                "Energy dissipation as {form}",
                "Structural deformation of {body}",
                "Resonance in {system}",
                "Heat flow to {sink}",
                "Pressure equilibrium at {value}Pa",
            ],
            Self::Economics => vec![
                "Reduced borrowing in {sector}",
                "Price increase of {percent}%",
                "Unemployment rise to {rate}%",
                "Market contraction in {industry}",
                "Consumer spending decrease of {amount}%",
                "Investment decline in {sector}",
                "Trade deficit widening to ${deficit}",
                "Stock market drop of {points} points",
                "Business closures in {region}",
                "GDP growth slowdown to {growth}%",
            ],
            Self::Biology => vec![
                "Disease manifestation of {condition}",
                "Cellular apoptosis in {tissue}",
                "Immune response activation",
                "Metabolic rate change of {percent}%",
                "Tissue damage in {organ}",
                "Behavioral changes in {behavior}",
                "Protein misfolding of {protein}",
                "Inflammation in {area}",
                "Developmental abnormality in {stage}",
                "Neurological symptoms of {type}",
            ],
            Self::Psychology => vec![
                "Anxiety increase to {level}",
                "Performance decrease of {percent}%",
                "Behavioral change towards {behavior}",
                "Mood alteration to {state}",
                "Memory impairment of {type}",
                "Decision-making bias towards {bias}",
                "Attention deficit in {area}",
                "Emotional response of {emotion}",
                "Habit formation of {habit}",
                "Motivation change to {direction}",
            ],
            Self::Engineering => vec![
                "Structural failure of {component}",
                "System malfunction in {mode}",
                "Efficiency loss of {percent}%",
                "Safety hazard of {type}",
                "Operational downtime of {hours} hours",
                "Component replacement required for {part}",
                "Maintenance cost increase of ${amount}",
                "Performance degradation of {metric}",
                "Reliability reduction to {mtbf} MTBF",
                "Warranty claim for {issue}",
            ],
        }
    }

    /// Get intervention variables for this domain.
    pub fn intervention_variables(&self) -> Vec<&'static str> {
        match self {
            Self::Programming => vec![
                "memory_allocation",
                "thread_count",
                "buffer_size",
                "timeout_value",
                "retry_count",
                "cache_size",
                "connection_pool",
                "log_level",
                "error_handling",
                "input_validation",
            ],
            Self::Physics => vec![
                "temperature",
                "pressure",
                "force",
                "velocity",
                "mass",
                "voltage",
                "current",
                "frequency",
                "amplitude",
                "density",
            ],
            Self::Economics => vec![
                "interest_rate",
                "tax_rate",
                "inflation",
                "exchange_rate",
                "supply",
                "demand",
                "price",
                "wages",
                "employment",
                "investment",
            ],
            Self::Biology => vec![
                "gene_expression",
                "hormone_level",
                "nutrient_intake",
                "drug_dosage",
                "pathogen_load",
                "immune_response",
                "metabolic_rate",
                "cell_division",
                "enzyme_activity",
                "receptor_binding",
            ],
            Self::Psychology => vec![
                "stress_level",
                "sleep_quality",
                "social_support",
                "cognitive_load",
                "motivation",
                "reward_magnitude",
                "punishment_intensity",
                "attention_focus",
                "emotional_state",
                "learning_rate",
            ],
            Self::Engineering => vec![
                "load_factor",
                "temperature_cycle",
                "vibration_level",
                "material_grade",
                "safety_margin",
                "maintenance_interval",
                "operating_pressure",
                "duty_cycle",
                "tolerance_level",
                "environmental_rating",
            ],
        }
    }

    /// Get mechanism for this domain.
    pub fn mechanisms(&self) -> Vec<&'static str> {
        match self {
            Self::Programming => vec![
                "memory_management",
                "concurrency_control",
                "resource_allocation",
                "error_propagation",
                "state_management",
            ],
            Self::Physics => vec![
                "heat_transfer",
                "force_transmission",
                "energy_conversion",
                "wave_mechanics",
                "electromagnetic_induction",
            ],
            Self::Economics => vec![
                "price_mechanism",
                "market_equilibrium",
                "fiscal_policy",
                "monetary_transmission",
                "supply_chain",
            ],
            Self::Biology => vec![
                "gene_regulation",
                "signal_transduction",
                "metabolic_pathway",
                "immune_cascade",
                "neural_transmission",
            ],
            Self::Psychology => vec![
                "conditioning",
                "cognitive_processing",
                "emotional_regulation",
                "memory_consolidation",
                "attention_mechanism",
            ],
            Self::Engineering => vec![
                "stress_concentration",
                "fatigue_mechanism",
                "wear_mechanism",
                "thermal_cycling",
                "load_distribution",
            ],
        }
    }
}

impl std::fmt::Display for CausalDomain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Programming => write!(f, "programming"),
            Self::Physics => write!(f, "physics"),
            Self::Economics => write!(f, "economics"),
            Self::Biology => write!(f, "biology"),
            Self::Psychology => write!(f, "psychology"),
            Self::Engineering => write!(f, "engineering"),
        }
    }
}

/// A causal pair with cause, effect, and metadata.
#[derive(Debug, Clone)]
pub struct CausalPair {
    /// Unique ID.
    pub id: Uuid,

    /// Cause content text.
    pub cause_content: String,

    /// Effect content text.
    pub effect_content: String,

    /// Causal strength [0.0-1.0].
    pub strength: f32,

    /// Domain of the causal relationship.
    pub domain: CausalDomain,

    /// Intervention context for the cause.
    pub cause_context: InterventionContext,

    /// Intervention context for the effect.
    pub effect_context: InterventionContext,

    /// Chain ID (for multi-hop chains).
    pub chain_id: Option<usize>,

    /// Position in chain.
    pub chain_position: Option<usize>,
}

/// A query for direction detection testing.
#[derive(Debug, Clone)]
pub struct DirectionQuery {
    /// Unique ID.
    pub id: Uuid,

    /// Query text.
    pub query_text: String,

    /// Expected direction (ground truth).
    pub expected_direction: CausalDirection,

    /// Domain of the query.
    pub domain: CausalDomain,

    /// Template pattern used.
    pub pattern: String,
}

/// A COPA (Choice of Plausible Alternatives) question.
#[derive(Debug, Clone)]
pub struct CopaQuestion {
    /// Unique ID.
    pub id: Uuid,

    /// Premise statement.
    pub premise: String,

    /// Question type: "cause" or "effect".
    pub question_type: String,

    /// First alternative.
    pub alternative1: String,

    /// Second alternative.
    pub alternative2: String,

    /// Correct answer: 1 or 2.
    pub correct_answer: u8,

    /// Domain of the question.
    pub domain: CausalDomain,
}

/// A multi-hop causal chain.
#[derive(Debug, Clone)]
pub struct CausalChain {
    /// Chain ID.
    pub id: usize,

    /// Ordered list of pair IDs in the chain.
    pub pair_ids: Vec<Uuid>,

    /// Domain of the chain.
    pub domain: CausalDomain,

    /// Total chain length (number of hops).
    pub length: usize,
}

/// Complete causal benchmark dataset.
#[derive(Debug)]
pub struct CausalBenchmarkDataset {
    /// All causal pairs.
    pub pairs: Vec<CausalPair>,

    /// Direction detection queries.
    pub direction_queries: Vec<DirectionQuery>,

    /// COPA questions.
    pub copa_questions: Vec<CopaQuestion>,

    /// Causal chains.
    pub chains: Vec<CausalChain>,

    /// Configuration used.
    pub config: CausalDatasetConfig,
}

/// Generator for causal benchmark datasets.
pub struct CausalDatasetGenerator {
    config: CausalDatasetConfig,
    rng: ChaCha8Rng,
}

impl CausalDatasetGenerator {
    /// Create a new generator with the given configuration.
    pub fn new(config: CausalDatasetConfig) -> Self {
        let rng = ChaCha8Rng::seed_from_u64(config.seed);
        Self { config, rng }
    }

    /// Generate a deterministic UUID from the seeded RNG.
    ///
    /// This ensures reproducibility when using the same seed.
    fn next_uuid(&mut self) -> Uuid {
        let mut bytes = [0u8; 16];
        self.rng.fill_bytes(&mut bytes);
        // Set version 4 (random) and variant bits per RFC 4122
        bytes[6] = (bytes[6] & 0x0f) | 0x40; // Version 4
        bytes[8] = (bytes[8] & 0x3f) | 0x80; // Variant 1
        Uuid::from_bytes(bytes)
    }

    /// Generate a complete causal benchmark dataset.
    pub fn generate(&mut self) -> CausalBenchmarkDataset {
        // Generate causal pairs
        let pairs = self.generate_causal_pairs();

        // Generate chains from pairs
        let chains = self.generate_chains(&pairs);

        // Generate direction detection queries
        let direction_queries = self.generate_direction_queries();

        // Generate COPA questions
        let copa_questions = self.generate_copa_questions(&pairs);

        CausalBenchmarkDataset {
            pairs,
            direction_queries,
            copa_questions,
            chains,
            config: self.config.clone(),
        }
    }

    fn generate_causal_pairs(&mut self) -> Vec<CausalPair> {
        let mut pairs = Vec::with_capacity(self.config.num_causal_pairs);

        for _ in 0..self.config.num_causal_pairs {
            let domain = self.random_domain();
            let cause_templates = domain.cause_templates();
            let effect_templates = domain.effect_templates();

            let cause_template = cause_templates[self.rng.gen_range(0..cause_templates.len())];
            let effect_template = effect_templates[self.rng.gen_range(0..effect_templates.len())];

            let cause_content = self.fill_template(cause_template, &domain);
            let effect_content = self.fill_template(effect_template, &domain);

            let intervention_vars = domain.intervention_variables();
            let mechanisms = domain.mechanisms();

            // Create intervention contexts
            let num_vars = self.rng.gen_range(1..=3.min(intervention_vars.len()));
            let mut cause_vars: Vec<String> = Vec::new();
            for _ in 0..num_vars {
                cause_vars.push(
                    intervention_vars[self.rng.gen_range(0..intervention_vars.len())].to_string(),
                );
            }

            let cause_context = InterventionContext {
                intervened_variables: cause_vars.clone(),
                domain: Some(domain.to_string()),
                mechanism: Some(mechanisms[self.rng.gen_range(0..mechanisms.len())].to_string()),
            };

            // Effect context often shares some variables with cause
            let mut effect_vars = cause_vars.clone();
            if self.rng.gen_bool(0.5) && !intervention_vars.is_empty() {
                effect_vars.push(
                    intervention_vars[self.rng.gen_range(0..intervention_vars.len())].to_string(),
                );
            }

            let effect_context = InterventionContext {
                intervened_variables: effect_vars,
                domain: Some(domain.to_string()),
                mechanism: Some(mechanisms[self.rng.gen_range(0..mechanisms.len())].to_string()),
            };

            pairs.push(CausalPair {
                id: self.next_uuid(),
                cause_content,
                effect_content,
                strength: self.rng.gen_range(0.5..1.0),
                domain,
                cause_context,
                effect_context,
                chain_id: None,
                chain_position: None,
            });
        }

        pairs
    }

    fn generate_chains(&mut self, pairs: &[CausalPair]) -> Vec<CausalChain> {
        let mut chains = Vec::with_capacity(self.config.num_chains);
        let mut used_pairs: HashSet<Uuid> = HashSet::new();

        // Group pairs by domain
        let mut pairs_by_domain: std::collections::HashMap<CausalDomain, Vec<&CausalPair>> =
            std::collections::HashMap::new();
        for pair in pairs {
            pairs_by_domain.entry(pair.domain).or_default().push(pair);
        }

        for chain_id in 0..self.config.num_chains {
            let domain = self.random_domain();
            let domain_pairs = match pairs_by_domain.get(&domain) {
                Some(p) if !p.is_empty() => p,
                _ => continue,
            };

            let chain_length = self.rng.gen_range(2..=self.config.avg_chain_length + 2);
            let mut chain_pair_ids = Vec::new();

            // Select pairs for this chain
            let available: Vec<_> = domain_pairs
                .iter()
                .filter(|p| !used_pairs.contains(&p.id))
                .collect();

            for pair in available.iter().take(chain_length) {
                chain_pair_ids.push(pair.id);
                used_pairs.insert(pair.id);
            }

            if chain_pair_ids.len() >= 2 {
                chains.push(CausalChain {
                    id: chain_id,
                    pair_ids: chain_pair_ids.clone(),
                    domain,
                    length: chain_pair_ids.len(),
                });
            }
        }

        chains
    }

    fn generate_direction_queries(&mut self) -> Vec<DirectionQuery> {
        let mut queries = Vec::with_capacity(self.config.num_direction_queries);

        // Cause-seeking query patterns
        // ~70% easy (match indicators), ~30% hard (don't match)
        let cause_patterns = vec![
            // EASY: These match detection indicators (7 patterns)
            ("Why does {effect} happen?", CausalDirection::Cause),
            ("What causes {effect}?", CausalDirection::Cause),
            ("What caused {effect}?", CausalDirection::Cause),
            ("What led to {effect}?", CausalDirection::Cause),
            ("Explain why {effect} occurs", CausalDirection::Cause),
            ("What is the reason for {effect}?", CausalDirection::Cause),
            ("How come {effect}?", CausalDirection::Cause),
            // HARD: These don't match indicators (3 patterns) - reveals detection gaps
            ("Diagnose {effect} for me", CausalDirection::Cause),
            ("What triggers {effect}?", CausalDirection::Cause),
            ("Root cause analysis for {effect}", CausalDirection::Cause),
        ];

        // Effect-seeking query patterns
        // ~70% easy (match indicators), ~30% hard (don't match)
        let effect_patterns = vec![
            // EASY: These match detection indicators (7 patterns)
            ("What happens if {cause}?", CausalDirection::Effect),
            ("What will happen when {cause}?", CausalDirection::Effect),
            ("What are the effects of {cause}?", CausalDirection::Effect),
            (
                "What is the consequence of {cause}?",
                CausalDirection::Effect,
            ),
            ("What is the result of {cause}?", CausalDirection::Effect),
            ("What is the impact of {cause}?", CausalDirection::Effect),
            ("If I {cause}, then what?", CausalDirection::Effect),
            // HARD: These don't match indicators (3 patterns) - reveals detection gaps
            ("Predict the outcome after {cause}", CausalDirection::Effect),
            ("What follows from {cause}?", CausalDirection::Effect),
            (
                "Downstream implications of {cause}",
                CausalDirection::Effect,
            ),
        ];

        // Unknown/neutral patterns
        // Include some ambiguous patterns that might trigger false positives
        let unknown_patterns = [
            // Clear neutral patterns
            ("Tell me about {topic}", CausalDirection::Unknown),
            ("Describe {topic}", CausalDirection::Unknown),
            ("Show me {topic}", CausalDirection::Unknown),
            ("List all {topic}", CausalDirection::Unknown),
            ("Find {topic}", CausalDirection::Unknown),
            ("Search for {topic}", CausalDirection::Unknown),
            // Ambiguous patterns that might trigger false positives
            ("Details on {topic}", CausalDirection::Unknown),
            ("Information about {topic}", CausalDirection::Unknown),
        ];

        let num_cause =
            (self.config.num_direction_queries as f64 * self.config.cause_effect_ratio) as usize;
        let num_effect = (self.config.num_direction_queries as f64
            * (1.0 - self.config.cause_effect_ratio)
            / 2.0) as usize;
        let num_unknown = self.config.num_direction_queries - num_cause - num_effect;

        // Generate cause queries
        for _ in 0..num_cause {
            let domain = self.random_domain();
            let pattern_idx = self.rng.gen_range(0..cause_patterns.len());
            let (pattern, direction) = cause_patterns[pattern_idx];
            let effect_templates = domain.effect_templates();
            let template_idx = self.rng.gen_range(0..effect_templates.len());
            let effect = self.fill_template(effect_templates[template_idx], &domain);
            let query_text = pattern.replace("{effect}", &effect);

            queries.push(DirectionQuery {
                id: self.next_uuid(),
                query_text,
                expected_direction: direction,
                domain,
                pattern: pattern.to_string(),
            });
        }

        // Generate effect queries
        for _ in 0..num_effect {
            let domain = self.random_domain();
            let pattern_idx = self.rng.gen_range(0..effect_patterns.len());
            let (pattern, direction) = effect_patterns[pattern_idx];
            let cause_templates = domain.cause_templates();
            let template_idx = self.rng.gen_range(0..cause_templates.len());
            let cause = self.fill_template(cause_templates[template_idx], &domain);
            let query_text = pattern.replace("{cause}", &cause);

            queries.push(DirectionQuery {
                id: self.next_uuid(),
                query_text,
                expected_direction: direction,
                domain,
                pattern: pattern.to_string(),
            });
        }

        // Generate unknown queries
        for _ in 0..num_unknown {
            let domain = self.random_domain();
            let (pattern, direction) =
                unknown_patterns[self.rng.gen_range(0..unknown_patterns.len())];
            let topics = domain.intervention_variables();
            let topic = topics[self.rng.gen_range(0..topics.len())];
            let query_text = pattern.replace("{topic}", topic);

            queries.push(DirectionQuery {
                id: self.next_uuid(),
                query_text,
                expected_direction: direction,
                domain,
                pattern: pattern.to_string(),
            });
        }

        // Shuffle queries
        queries.shuffle(&mut self.rng);
        queries
    }

    fn generate_copa_questions(&mut self, pairs: &[CausalPair]) -> Vec<CopaQuestion> {
        let mut questions = Vec::with_capacity(self.config.num_copa_questions);

        // For each COPA question, we need:
        // - A premise (either cause or effect)
        // - A question asking for the other direction
        // - Two alternatives (one correct, one incorrect)

        for _ in 0..self.config.num_copa_questions {
            if pairs.len() < 2 {
                continue;
            }

            // Select a correct pair
            let correct_pair = &pairs[self.rng.gen_range(0..pairs.len())];

            // Select a distractor from the same domain
            let distractors: Vec<_> = pairs
                .iter()
                .filter(|p| p.domain == correct_pair.domain && p.id != correct_pair.id)
                .collect();

            if distractors.is_empty() {
                continue;
            }

            let distractor = distractors[self.rng.gen_range(0..distractors.len())];

            // Decide question type
            let is_cause_question = self.rng.gen_bool(0.5);

            let (premise, question_type, alternative1, alternative2, correct_answer) =
                if is_cause_question {
                    // Premise is the effect, asking for the cause
                    (
                        correct_pair.effect_content.clone(),
                        "cause".to_string(),
                        correct_pair.cause_content.clone(),
                        distractor.cause_content.clone(),
                        1u8,
                    )
                } else {
                    // Premise is the cause, asking for the effect
                    (
                        correct_pair.cause_content.clone(),
                        "effect".to_string(),
                        correct_pair.effect_content.clone(),
                        distractor.effect_content.clone(),
                        1u8,
                    )
                };

            // Randomly swap alternatives
            let (alt1, alt2, answer) = if self.rng.gen_bool(0.5) {
                (alternative2, alternative1, 2u8)
            } else {
                (alternative1, alternative2, correct_answer)
            };

            questions.push(CopaQuestion {
                id: self.next_uuid(),
                premise,
                question_type,
                alternative1: alt1,
                alternative2: alt2,
                correct_answer: answer,
                domain: correct_pair.domain,
            });
        }

        questions
    }

    fn random_domain(&mut self) -> CausalDomain {
        self.config.domains[self.rng.gen_range(0..self.config.domains.len())]
    }

    fn fill_template(&mut self, template: &str, domain: &CausalDomain) -> String {
        let mut result = template.to_string();

        // Replace common placeholders with domain-appropriate values
        let replacements: Vec<(&str, Vec<&str>)> = match domain {
            CausalDomain::Programming => vec![
                (
                    "{component}",
                    vec!["UserService", "AuthModule", "DataCache", "EventHandler"],
                ),
                (
                    "{module}",
                    vec!["authentication", "database", "networking", "storage"],
                ),
                (
                    "{function}",
                    vec![
                        "processRequest",
                        "handleEvent",
                        "validateInput",
                        "serializeData",
                    ],
                ),
                (
                    "{buffer}",
                    vec!["inputBuffer", "outputBuffer", "packetBuffer", "frameBuffer"],
                ),
                (
                    "{thread_a}",
                    vec!["WorkerThread-1", "IOThread", "MainThread", "GCThread"],
                ),
                (
                    "{thread_b}",
                    vec!["WorkerThread-2", "TimerThread", "EventThread", "PoolThread"],
                ),
                (
                    "{handler}",
                    vec![
                        "RequestHandler",
                        "ErrorHandler",
                        "EventListener",
                        "Callback",
                    ],
                ),
                (
                    "{calculation}",
                    vec![
                        "array indexing",
                        "hash computation",
                        "size calculation",
                        "offset",
                    ],
                ),
                (
                    "{allocator}",
                    vec![
                        "heap allocator",
                        "pool allocator",
                        "arena allocator",
                        "stack",
                    ],
                ),
                (
                    "{recursive_function}",
                    vec!["parseTree", "traverseGraph", "deepCopy", "factorial"],
                ),
                (
                    "{endpoint}",
                    vec!["/api/users", "/login", "/data/export", "/admin/settings"],
                ),
                (
                    "{duration}",
                    vec!["3 seconds", "1 minute", "5 minutes", "prolonged use"],
                ),
                (
                    "{structure}",
                    vec![
                        "user records",
                        "transaction log",
                        "cache entries",
                        "session data",
                    ],
                ),
                ("{seconds}", vec!["5", "30", "60", "120"]),
                ("{operations}", vec!["1000", "10000", "100000", "1 million"]),
                ("{percent}", vec!["20", "50", "75", "90"]),
                ("{layer}", vec!["API", "service", "data", "presentation"]),
                ("{minutes}", vec!["5", "15", "30", "60"]),
                (
                    "{resource}",
                    vec![
                        "database connection",
                        "file handle",
                        "network socket",
                        "thread pool",
                    ],
                ),
                (
                    "{system}",
                    vec![
                        "microservices",
                        "distributed cache",
                        "message queue",
                        "load balancer",
                    ],
                ),
            ],
            CausalDomain::Physics => vec![
                ("{degrees}", vec!["10", "50", "100", "500"]),
                ("{newtons}", vec!["10", "100", "1000", "10000"]),
                ("{pascals}", vec!["1000", "100000", "1000000", "10 million"]),
                ("{amps}", vec!["0.1", "1", "10", "100"]),
                ("{tesla}", vec!["0.01", "0.1", "1", "10"]),
                ("{mps2}", vec!["1", "5", "10", "100"]),
                ("{watts}", vec!["10", "100", "1000", "10000"]),
                ("{coeff}", vec!["0.1", "0.3", "0.5", "0.8"]),
                ("{volts}", vec!["5", "12", "120", "240"]),
                ("{force}", vec!["1", "10", "100", "1000"]),
                ("{material}", vec!["metal", "glass", "plastic", "ceramic"]),
                ("{object}", vec!["particle", "ball", "block", "projectile"]),
                ("{state}", vec!["liquid", "gas", "plasma", "solid"]),
                (
                    "{conductor}",
                    vec!["copper wire", "aluminum rod", "silver plate", "coil"],
                ),
                ("{medium}", vec!["air", "water", "glass", "vacuum"]),
                ("{form}", vec!["heat", "sound", "light", "vibration"]),
                ("{body}", vec!["beam", "plate", "rod", "cylinder"]),
                ("{sink}", vec!["ambient", "coolant", "radiator", "ground"]),
                ("{value}", vec!["1000", "10000", "100000", "1000000"]),
            ],
            CausalDomain::Economics => vec![
                ("{rate}", vec!["0.25", "0.5", "1.0", "2.0"]),
                ("{inflation}", vec!["2", "5", "10", "15"]),
                (
                    "{commodity}",
                    vec!["oil", "wheat", "copper", "semiconductors"],
                ),
                (
                    "{product}",
                    vec!["housing", "electric vehicles", "smartphones", "software"],
                ),
                ("{tax_rate}", vec!["5", "10", "15", "20"]),
                ("{percent}", vec!["10", "20", "30", "50"]),
                (
                    "{goods}",
                    vec!["steel", "electronics", "textiles", "automobiles"],
                ),
                (
                    "{amount}",
                    vec!["1 billion", "10 billion", "100 billion", "1 trillion"],
                ),
                (
                    "{sector}",
                    vec!["technology", "healthcare", "manufacturing", "finance"],
                ),
                (
                    "{industry}",
                    vec!["retail", "construction", "hospitality", "transportation"],
                ),
                (
                    "{region}",
                    vec!["urban areas", "midwest", "coastal regions", "rural areas"],
                ),
                ("{growth}", vec!["1", "2", "3", "4"]),
                (
                    "{deficit}",
                    vec!["50 billion", "100 billion", "200 billion", "500 billion"],
                ),
                ("{points}", vec!["100", "500", "1000", "2000"]),
            ],
            CausalDomain::Biology => vec![
                ("{gene}", vec!["BRCA1", "p53", "CFTR", "HTT"]),
                (
                    "{virus}",
                    vec!["influenza", "coronavirus", "rhinovirus", "hepatitis"],
                ),
                (
                    "{hormone}",
                    vec!["cortisol", "insulin", "thyroid", "estrogen"],
                ),
                ("{nutrient}", vec!["vitamin D", "iron", "calcium", "B12"]),
                (
                    "{antibiotic}",
                    vec!["penicillin", "amoxicillin", "ciprofloxacin", "azithromycin"],
                ),
                ("{toxin}", vec!["lead", "mercury", "arsenic", "benzene"]),
                (
                    "{stressor}",
                    vec!["oxidative stress", "heat shock", "hypoxia", "UV radiation"],
                ),
                (
                    "{antigen}",
                    vec![
                        "bacterial protein",
                        "viral coat",
                        "allergen",
                        "foreign cell",
                    ],
                ),
                (
                    "{enzyme}",
                    vec!["kinase", "protease", "lipase", "polymerase"],
                ),
                ("{radiation}", vec!["UV", "ionizing", "X-ray", "gamma"]),
                (
                    "{condition}",
                    vec!["diabetes", "hypertension", "cancer", "autoimmune disease"],
                ),
                ("{tissue}", vec!["liver", "kidney", "brain", "muscle"]),
                ("{organ}", vec!["heart", "lungs", "liver", "kidney"]),
                (
                    "{behavior}",
                    vec!["feeding", "sleep", "locomotion", "social"],
                ),
                (
                    "{protein}",
                    vec!["hemoglobin", "collagen", "keratin", "amyloid"],
                ),
                ("{area}", vec!["joints", "airways", "skin", "gut"]),
                (
                    "{stage}",
                    vec!["embryonic", "fetal", "neonatal", "pubertal"],
                ),
                ("{type}", vec!["cognitive", "motor", "sensory", "autonomic"]),
            ],
            CausalDomain::Psychology => vec![
                (
                    "{stressor}",
                    vec![
                        "work deadline",
                        "financial pressure",
                        "relationship conflict",
                        "health concern",
                    ],
                ),
                ("{hours}", vec!["24", "48", "72", "96"]),
                ("{event}", vec!["accident", "loss", "violence", "disaster"]),
                (
                    "{duration}",
                    vec!["1 week", "1 month", "3 months", "1 year"],
                ),
                (
                    "{task}",
                    vec![
                        "multitasking",
                        "complex problem",
                        "information overload",
                        "decision making",
                    ],
                ),
                (
                    "{behavior}",
                    vec!["healthy eating", "exercise", "studying", "socializing"],
                ),
                (
                    "{performance}",
                    vec!["academic", "athletic", "professional", "creative"],
                ),
                ("{setting}", vec!["classroom", "office", "home", "outdoor"]),
                (
                    "{topic}",
                    vec!["achievement", "resilience", "growth", "purpose"],
                ),
                (
                    "{stimulus}",
                    vec![
                        "loud noise",
                        "flashing light",
                        "unexpected touch",
                        "sudden movement",
                    ],
                ),
                ("{level}", vec!["mild", "moderate", "severe", "extreme"]),
                ("{state}", vec!["euphoric", "depressed", "anxious", "calm"]),
                (
                    "{bias}",
                    vec!["risk-averse", "risk-seeking", "confirmation", "anchoring"],
                ),
                ("{emotion}", vec!["fear", "anger", "sadness", "joy"]),
                (
                    "{habit}",
                    vec![
                        "nail-biting",
                        "procrastination",
                        "checking phone",
                        "overeating",
                    ],
                ),
                (
                    "{direction}",
                    vec!["intrinsic", "extrinsic", "achievement", "avoidance"],
                ),
            ],
            CausalDomain::Engineering => vec![
                ("{component}", vec!["bearing", "gear", "shaft", "seal"]),
                ("{force}", vec!["1000", "5000", "10000", "50000"]),
                ("{cycles}", vec!["1000", "10000", "100000", "1 million"]),
                (
                    "{environment}",
                    vec!["marine", "industrial", "chemical", "outdoor"],
                ),
                ("{frequency}", vec!["10", "100", "1000", "10000"]),
                ("{percent}", vec!["110", "125", "150", "200"]),
                ("{subsystem}", vec!["control", "power", "cooling", "safety"]),
                ("{part}", vec!["bracket", "fitting", "connector", "housing"]),
                (
                    "{condition}",
                    vec!["humidity", "temperature extremes", "dust", "vibration"],
                ),
                (
                    "{mode}",
                    vec!["intermittent", "complete", "partial", "progressive"],
                ),
                ("{hours}", vec!["8", "24", "48", "168"]),
                (
                    "{metric}",
                    vec!["throughput", "efficiency", "accuracy", "response time"],
                ),
                ("{mtbf}", vec!["1000", "5000", "10000", "50000"]),
                (
                    "{issue}",
                    vec![
                        "premature failure",
                        "manufacturing defect",
                        "design flaw",
                        "wear",
                    ],
                ),
            ],
        };

        for (placeholder, values) in replacements {
            if result.contains(placeholder) {
                let value = values[self.rng.gen_range(0..values.len())];
                result = result.replace(placeholder, value);
            }
        }

        result
    }
}

impl CausalBenchmarkDataset {
    /// Get a causal pair by ID.
    pub fn get_pair(&self, id: &Uuid) -> Option<&CausalPair> {
        self.pairs.iter().find(|p| &p.id == id)
    }

    /// Get pairs for a specific chain.
    pub fn get_chain_pairs(&self, chain_id: usize) -> Vec<&CausalPair> {
        let chain = self.chains.iter().find(|c| c.id == chain_id);
        match chain {
            Some(c) => c
                .pair_ids
                .iter()
                .filter_map(|id| self.get_pair(id))
                .collect(),
            None => Vec::new(),
        }
    }

    /// Validate dataset consistency.
    pub fn validate(&self) -> Result<(), String> {
        // Check all chains reference valid pairs
        for chain in &self.chains {
            for pair_id in &chain.pair_ids {
                if self.get_pair(pair_id).is_none() {
                    return Err(format!(
                        "Chain {} references unknown pair {}",
                        chain.id, pair_id
                    ));
                }
            }
            if chain.pair_ids.len() < 2 {
                return Err(format!(
                    "Chain {} has only {} pairs (minimum 2)",
                    chain.id,
                    chain.pair_ids.len()
                ));
            }
        }

        // Check COPA questions have valid alternatives
        for question in &self.copa_questions {
            if question.correct_answer != 1 && question.correct_answer != 2 {
                return Err(format!(
                    "COPA question {} has invalid correct_answer {}",
                    question.id, question.correct_answer
                ));
            }
            if question.alternative1.is_empty() || question.alternative2.is_empty() {
                return Err(format!(
                    "COPA question {} has empty alternatives",
                    question.id
                ));
            }
        }

        // Check direction queries have expected directions
        for query in &self.direction_queries {
            if query.query_text.is_empty() {
                return Err(format!("Direction query {} has empty text", query.id));
            }
        }

        Ok(())
    }

    /// Get dataset statistics.
    pub fn stats(&self) -> CausalDatasetStats {
        let mut domain_counts: std::collections::HashMap<CausalDomain, usize> =
            std::collections::HashMap::new();

        for pair in &self.pairs {
            *domain_counts.entry(pair.domain).or_default() += 1;
        }

        let cause_queries = self
            .direction_queries
            .iter()
            .filter(|q| q.expected_direction == CausalDirection::Cause)
            .count();
        let effect_queries = self
            .direction_queries
            .iter()
            .filter(|q| q.expected_direction == CausalDirection::Effect)
            .count();
        let unknown_queries = self
            .direction_queries
            .iter()
            .filter(|q| q.expected_direction == CausalDirection::Unknown)
            .count();

        let copa_cause_questions = self
            .copa_questions
            .iter()
            .filter(|q| q.question_type == "cause")
            .count();
        let copa_effect_questions = self
            .copa_questions
            .iter()
            .filter(|q| q.question_type == "effect")
            .count();

        CausalDatasetStats {
            total_pairs: self.pairs.len(),
            total_direction_queries: self.direction_queries.len(),
            total_copa_questions: self.copa_questions.len(),
            total_chains: self.chains.len(),
            pairs_by_domain: domain_counts,
            cause_queries,
            effect_queries,
            unknown_queries,
            copa_cause_questions,
            copa_effect_questions,
            avg_chain_length: if self.chains.is_empty() {
                0.0
            } else {
                self.chains.iter().map(|c| c.length as f64).sum::<f64>() / self.chains.len() as f64
            },
        }
    }
}

/// Statistics about a causal dataset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalDatasetStats {
    pub total_pairs: usize,
    pub total_direction_queries: usize,
    pub total_copa_questions: usize,
    pub total_chains: usize,
    pub pairs_by_domain: std::collections::HashMap<CausalDomain, usize>,
    pub cause_queries: usize,
    pub effect_queries: usize,
    pub unknown_queries: usize,
    pub copa_cause_questions: usize,
    pub copa_effect_questions: usize,
    pub avg_chain_length: f64,
}

#[cfg(all(test, feature = "benchmark-tests"))]
mod tests {
    use super::*;

    #[test]
    fn test_default_config_generation() {
        let config = CausalDatasetConfig {
            num_causal_pairs: 50,
            num_direction_queries: 30,
            num_copa_questions: 20,
            num_chains: 5,
            seed: 42,
            ..Default::default()
        };

        let mut generator = CausalDatasetGenerator::new(config);
        let dataset = generator.generate();

        assert_eq!(dataset.pairs.len(), 50);
        assert_eq!(dataset.direction_queries.len(), 30);
        // COPA might have fewer due to distractor requirements
        assert!(dataset.copa_questions.len() <= 20);
        assert!(dataset.chains.len() <= 5);

        println!("[VERIFIED] Dataset generation produces expected counts");
        println!("  Pairs: {}", dataset.pairs.len());
        println!("  Direction queries: {}", dataset.direction_queries.len());
        println!("  COPA questions: {}", dataset.copa_questions.len());
        println!("  Chains: {}", dataset.chains.len());
    }

    #[test]
    fn test_dataset_validation() {
        let config = CausalDatasetConfig {
            num_causal_pairs: 100,
            num_direction_queries: 50,
            num_copa_questions: 30,
            num_chains: 10,
            seed: 42,
            ..Default::default()
        };

        let mut generator = CausalDatasetGenerator::new(config);
        let dataset = generator.generate();

        let result = dataset.validate();
        assert!(result.is_ok(), "Validation failed: {:?}", result);

        println!("[VERIFIED] Generated dataset passes validation");
    }

    #[test]
    fn test_direction_query_distribution() {
        let config = CausalDatasetConfig {
            num_direction_queries: 100,
            cause_effect_ratio: 0.5,
            seed: 42,
            ..Default::default()
        };

        let mut generator = CausalDatasetGenerator::new(config);
        let dataset = generator.generate();
        let stats = dataset.stats();

        // Should be roughly 50% cause queries
        assert!(
            stats.cause_queries >= 40 && stats.cause_queries <= 60,
            "Cause queries: {} (expected ~50)",
            stats.cause_queries
        );

        println!("[VERIFIED] Direction query distribution is balanced");
        println!("  Cause: {}", stats.cause_queries);
        println!("  Effect: {}", stats.effect_queries);
        println!("  Unknown: {}", stats.unknown_queries);
    }

    #[test]
    fn test_domain_coverage() {
        let config = CausalDatasetConfig {
            num_causal_pairs: 120, // Enough to cover all domains
            domains: CausalDomain::all(),
            seed: 42,
            ..Default::default()
        };

        let mut generator = CausalDatasetGenerator::new(config);
        let dataset = generator.generate();
        let stats = dataset.stats();

        // Each domain should have some pairs
        for domain in CausalDomain::all() {
            let count = stats.pairs_by_domain.get(&domain).copied().unwrap_or(0);
            assert!(count > 0, "Domain {:?} has no pairs", domain);
        }

        println!("[VERIFIED] All domains have coverage");
        for (domain, count) in &stats.pairs_by_domain {
            println!("  {:?}: {}", domain, count);
        }
    }

    #[test]
    fn test_chain_structure() {
        let config = CausalDatasetConfig {
            num_causal_pairs: 100,
            num_chains: 10,
            avg_chain_length: 4,
            seed: 42,
            ..Default::default()
        };

        let mut generator = CausalDatasetGenerator::new(config);
        let dataset = generator.generate();

        for chain in &dataset.chains {
            assert!(
                chain.pair_ids.len() >= 2,
                "Chain {} has only {} pairs",
                chain.id,
                chain.pair_ids.len()
            );

            // Check all pairs in chain are retrievable
            for pair_id in &chain.pair_ids {
                assert!(
                    dataset.get_pair(pair_id).is_some(),
                    "Chain {} references missing pair {}",
                    chain.id,
                    pair_id
                );
            }
        }

        println!("[VERIFIED] Chain structure is valid");
        println!("  Chains created: {}", dataset.chains.len());
        println!(
            "  Avg chain length: {:.1}",
            dataset.stats().avg_chain_length
        );
    }

    #[test]
    fn test_copa_question_validity() {
        let config = CausalDatasetConfig {
            num_causal_pairs: 100,
            num_copa_questions: 30,
            seed: 42,
            ..Default::default()
        };

        let mut generator = CausalDatasetGenerator::new(config);
        let dataset = generator.generate();

        for question in &dataset.copa_questions {
            assert!(
                question.correct_answer == 1 || question.correct_answer == 2,
                "Invalid correct_answer: {}",
                question.correct_answer
            );
            assert!(
                !question.premise.is_empty(),
                "Empty premise in question {}",
                question.id
            );
            assert!(
                !question.alternative1.is_empty() && !question.alternative2.is_empty(),
                "Empty alternatives in question {}",
                question.id
            );
            assert!(
                question.question_type == "cause" || question.question_type == "effect",
                "Invalid question_type: {}",
                question.question_type
            );
        }

        println!("[VERIFIED] COPA questions are valid");
        let stats = dataset.stats();
        println!("  Total COPA: {}", stats.total_copa_questions);
        println!("  Cause questions: {}", stats.copa_cause_questions);
        println!("  Effect questions: {}", stats.copa_effect_questions);
    }

    #[test]
    fn test_intervention_context_generation() {
        let config = CausalDatasetConfig {
            num_causal_pairs: 50,
            seed: 42,
            ..Default::default()
        };

        let mut generator = CausalDatasetGenerator::new(config);
        let dataset = generator.generate();

        let mut has_domain = 0;
        let mut has_mechanism = 0;
        let mut has_variables = 0;

        for pair in &dataset.pairs {
            if pair.cause_context.domain.is_some() {
                has_domain += 1;
            }
            if pair.cause_context.mechanism.is_some() {
                has_mechanism += 1;
            }
            if !pair.cause_context.intervened_variables.is_empty() {
                has_variables += 1;
            }
        }

        assert_eq!(
            has_domain,
            dataset.pairs.len(),
            "All pairs should have domain"
        );
        assert_eq!(
            has_mechanism,
            dataset.pairs.len(),
            "All pairs should have mechanism"
        );
        assert_eq!(
            has_variables,
            dataset.pairs.len(),
            "All pairs should have variables"
        );

        println!("[VERIFIED] Intervention contexts are fully populated");
    }

    #[test]
    fn test_reproducibility() {
        let config = CausalDatasetConfig {
            num_causal_pairs: 20,
            num_direction_queries: 10,
            seed: 12345,
            ..Default::default()
        };

        let mut gen1 = CausalDatasetGenerator::new(config.clone());
        let dataset1 = gen1.generate();

        let mut gen2 = CausalDatasetGenerator::new(config);
        let dataset2 = gen2.generate();

        // Same seed should produce same IDs
        assert_eq!(dataset1.pairs.len(), dataset2.pairs.len());
        for (p1, p2) in dataset1.pairs.iter().zip(dataset2.pairs.iter()) {
            assert_eq!(p1.id, p2.id, "Pair IDs should match with same seed");
            assert_eq!(p1.cause_content, p2.cause_content);
        }

        println!("[VERIFIED] Dataset generation is reproducible with same seed");
    }
}
