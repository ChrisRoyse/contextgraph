//! Training data structures for causal embedder fine-tuning.
//!
//! Provides pair-based training data with LLM-generated labels, hard negatives,
//! and soft confidence scores for contrastive learning.

use rand::seq::SliceRandom;
use serde::{Deserialize, Serialize};

/// Direction of a causal relationship in training data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrainingDirection {
    /// A causes B (forward).
    Forward,
    /// B causes A (backward).
    Backward,
    /// Both directions (feedback loop).
    Bidirectional,
    /// No causal relationship.
    None,
}

impl TrainingDirection {
    /// Parse from LLM output string.
    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "forward" | "a_causes_b" | "cause" => Self::Forward,
            "backward" | "b_causes_a" | "effect" => Self::Backward,
            "bidirectional" | "both" => Self::Bidirectional,
            _ => Self::None,
        }
    }
}

/// A single training pair for contrastive causal learning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CausalTrainingPair {
    /// Text describing the cause.
    pub cause_text: String,
    /// Text describing the effect.
    pub effect_text: String,
    /// Direction of the causal relationship.
    pub direction: TrainingDirection,
    /// LLM confidence score [0.0, 1.0] — used as soft label.
    pub confidence: f32,
    /// Causal mechanism domain (e.g., "biological", "economic").
    pub mechanism: String,
    /// Hard negative: semantically similar but non-causal text.
    pub hard_negative: String,
    /// Optional rationale explaining WHY this is causal (training signal).
    pub rationale: Option<String>,
    /// Domain category for curriculum learning.
    pub domain: String,
}

impl CausalTrainingPair {
    /// Create a new training pair.
    pub fn new(
        cause_text: String,
        effect_text: String,
        direction: TrainingDirection,
        confidence: f32,
    ) -> Self {
        Self {
            cause_text,
            effect_text,
            direction,
            confidence: confidence.clamp(0.0, 1.0),
            mechanism: String::new(),
            hard_negative: String::new(),
            rationale: None,
            domain: "general".to_string(),
        }
    }

    /// Set the mechanism description.
    pub fn with_mechanism(mut self, mechanism: impl Into<String>) -> Self {
        self.mechanism = mechanism.into();
        self
    }

    /// Set the hard negative text.
    pub fn with_hard_negative(mut self, neg: impl Into<String>) -> Self {
        self.hard_negative = neg.into();
        self
    }

    /// Set the rationale.
    pub fn with_rationale(mut self, rationale: impl Into<String>) -> Self {
        self.rationale = Some(rationale.into());
        self
    }

    /// Set the domain.
    pub fn with_domain(mut self, domain: impl Into<String>) -> Self {
        self.domain = domain.into();
        self
    }

    /// Whether this pair has a valid causal relationship.
    pub fn is_causal(&self) -> bool {
        !matches!(self.direction, TrainingDirection::None) && self.confidence >= 0.5
    }

    /// Difficulty level for curriculum learning (0.0 = easy, 1.0 = hard).
    pub fn difficulty(&self) -> f32 {
        let has_markers = self.cause_text.to_lowercase().contains("because")
            || self.cause_text.to_lowercase().contains("causes")
            || self.effect_text.to_lowercase().contains("therefore")
            || self.effect_text.to_lowercase().contains("results");

        if !self.is_causal() {
            return 0.0; // Non-causal pairs are easy negatives
        }

        if has_markers {
            0.2 // Explicit markers = easy
        } else if self.hard_negative.is_empty() {
            0.5 // Implicit causation = medium
        } else {
            0.8 // Hard negatives present = hard
        }
    }
}

/// A training batch with in-batch negatives.
#[derive(Debug, Clone)]
pub struct TrainingBatch {
    /// Pairs in this batch.
    pub pairs: Vec<CausalTrainingPair>,
    /// Batch index (for logging).
    pub batch_idx: usize,
}

impl TrainingBatch {
    /// Number of pairs in the batch.
    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    /// Whether the batch is empty.
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    /// Get all cause texts.
    pub fn cause_texts(&self) -> Vec<&str> {
        self.pairs.iter().map(|p| p.cause_text.as_str()).collect()
    }

    /// Get all effect texts.
    pub fn effect_texts(&self) -> Vec<&str> {
        self.pairs.iter().map(|p| p.effect_text.as_str()).collect()
    }

    /// Get all hard negative texts (non-empty only).
    pub fn hard_negatives(&self) -> Vec<&str> {
        self.pairs
            .iter()
            .filter(|p| !p.hard_negative.is_empty())
            .map(|p| p.hard_negative.as_str())
            .collect()
    }

    /// Get soft label targets (LLM confidence scores).
    pub fn soft_labels(&self) -> Vec<f32> {
        self.pairs.iter().map(|p| p.confidence).collect()
    }
}

/// Data loader for causal training with shuffling and batching.
pub struct CausalDataLoader {
    /// All training pairs.
    pairs: Vec<CausalTrainingPair>,
    /// Batch size.
    batch_size: usize,
    /// Current epoch's shuffled indices.
    indices: Vec<usize>,
    /// Current position in indices.
    position: usize,
    /// RNG for shuffling.
    rng: rand::rngs::StdRng,
}

impl CausalDataLoader {
    /// Create a new data loader.
    pub fn new(pairs: Vec<CausalTrainingPair>, batch_size: usize, seed: u64) -> Self {
        use rand::SeedableRng;
        let indices: Vec<usize> = (0..pairs.len()).collect();
        Self {
            pairs,
            batch_size,
            indices,
            position: 0,
            rng: rand::rngs::StdRng::seed_from_u64(seed),
        }
    }

    /// Total number of pairs.
    pub fn len(&self) -> usize {
        self.pairs.len()
    }

    /// Whether the loader has no pairs.
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    /// Number of batches per epoch.
    pub fn num_batches(&self) -> usize {
        (self.pairs.len() + self.batch_size - 1) / self.batch_size
    }

    /// Shuffle indices for a new epoch.
    pub fn shuffle_epoch(&mut self) {
        self.indices.shuffle(&mut self.rng);
        self.position = 0;
    }

    /// Get the next batch, or None if epoch is complete.
    pub fn next_batch(&mut self, batch_idx: usize) -> Option<TrainingBatch> {
        if self.position >= self.indices.len() {
            return None;
        }

        let end = (self.position + self.batch_size).min(self.indices.len());
        let batch_indices = &self.indices[self.position..end];
        self.position = end;

        let pairs: Vec<CausalTrainingPair> = batch_indices
            .iter()
            .map(|&idx| self.pairs[idx].clone())
            .collect();

        Some(TrainingBatch { pairs, batch_idx })
    }

    /// Filter pairs by maximum difficulty level (for curriculum learning).
    pub fn filter_by_difficulty(&self, max_difficulty: f32) -> Vec<CausalTrainingPair> {
        self.pairs
            .iter()
            .filter(|p| p.difficulty() <= max_difficulty)
            .cloned()
            .collect()
    }

    /// Split into train and eval sets.
    pub fn train_eval_split(
        mut self,
        eval_fraction: f32,
        seed: u64,
    ) -> (CausalDataLoader, CausalDataLoader) {
        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        self.pairs.shuffle(&mut rng);

        let eval_count = (self.pairs.len() as f32 * eval_fraction).ceil() as usize;
        let eval_pairs: Vec<CausalTrainingPair> =
            self.pairs.drain(self.pairs.len() - eval_count..).collect();
        let train_pairs = self.pairs;

        let train_loader = CausalDataLoader::new(train_pairs, self.batch_size, seed);
        let eval_loader = CausalDataLoader::new(eval_pairs, self.batch_size, seed + 1);

        (train_loader, eval_loader)
    }

    /// Add a new pair to the dataset (for online distillation).
    pub fn add_pair(&mut self, pair: CausalTrainingPair) {
        let idx = self.pairs.len();
        self.pairs.push(pair);
        self.indices.push(idx);
    }

    /// Get all pairs (immutable reference).
    pub fn pairs(&self) -> &[CausalTrainingPair] {
        &self.pairs
    }
}

/// Seed causal training pairs spanning multiple domains.
///
/// Returns ~50 high-quality seed pairs for LLM paraphrase expansion.
pub fn seed_training_pairs() -> Vec<CausalTrainingPair> {
    vec![
        // === Health / Biological ===
        CausalTrainingPair::new(
            "Chronic stress elevates cortisol levels through sustained HPA axis activation".into(),
            "Elevated cortisol damages hippocampal neurons and impairs memory formation".into(),
            TrainingDirection::Forward,
            0.92,
        )
        .with_mechanism("biological")
        .with_domain("health")
        .with_hard_negative("The hippocampus plays a key role in spatial navigation and memory recall"),
        CausalTrainingPair::new(
            "Smoking cigarettes introduces carcinogens into lung tissue".into(),
            "Long-term smoking significantly increases the risk of lung cancer".into(),
            TrainingDirection::Forward,
            0.95,
        )
        .with_mechanism("biological")
        .with_domain("health")
        .with_hard_negative("Lung cancer screening uses low-dose CT scans for early detection"),
        CausalTrainingPair::new(
            "Regular aerobic exercise increases BDNF expression in the brain".into(),
            "Enhanced BDNF promotes neuroplasticity and improved cognitive function".into(),
            TrainingDirection::Forward,
            0.88,
        )
        .with_mechanism("biological")
        .with_domain("health")
        .with_hard_negative("Cognitive tests measure attention, memory, and executive function"),
        CausalTrainingPair::new(
            "Chronic sleep deprivation disrupts immune system regulation".into(),
            "Weakened immune function increases susceptibility to infections".into(),
            TrainingDirection::Forward,
            0.85,
        )
        .with_mechanism("biological")
        .with_domain("health")
        .with_hard_negative("The immune system consists of innate and adaptive components"),
        CausalTrainingPair::new(
            "Obesity causes chronic low-grade inflammation".into(),
            "Chronic inflammation leads to insulin resistance and type 2 diabetes".into(),
            TrainingDirection::Forward,
            0.90,
        )
        .with_mechanism("biological")
        .with_domain("health")
        .with_hard_negative("Blood glucose levels are measured using HbA1c tests"),
        CausalTrainingPair::new(
            "Anxiety increases cortisol and disrupts sleep patterns".into(),
            "Chronic insomnia worsens anxiety symptoms through cognitive impairment".into(),
            TrainingDirection::Bidirectional,
            0.82,
        )
        .with_mechanism("feedback")
        .with_domain("health")
        .with_hard_negative("Cognitive behavioral therapy is an effective treatment for anxiety"),
        CausalTrainingPair::new(
            "High sodium intake raises blood pressure".into(),
            "Sustained hypertension damages arterial walls and increases stroke risk".into(),
            TrainingDirection::Forward,
            0.91,
        )
        .with_mechanism("biological")
        .with_domain("health")
        .with_hard_negative("Blood pressure is measured in millimeters of mercury (mmHg)"),
        CausalTrainingPair::new(
            "Gut microbiome dysbiosis impairs serotonin production".into(),
            "Reduced serotonin availability contributes to depression symptoms".into(),
            TrainingDirection::Forward,
            0.78,
        )
        .with_mechanism("mediated")
        .with_domain("health")
        .with_hard_negative("Serotonin is a neurotransmitter involved in mood regulation"),
        CausalTrainingPair::new(
            "UV radiation damages DNA in skin cells".into(),
            "Accumulated DNA damage leads to melanoma and other skin cancers".into(),
            TrainingDirection::Forward,
            0.93,
        )
        .with_mechanism("biological")
        .with_domain("health")
        .with_hard_negative("Dermatologists recommend annual skin cancer screenings"),
        CausalTrainingPair::new(
            "Antibiotic overuse selects for resistant bacterial strains".into(),
            "Antimicrobial resistance renders standard treatments ineffective".into(),
            TrainingDirection::Forward,
            0.89,
        )
        .with_mechanism("biological")
        .with_domain("health")
        .with_hard_negative("Penicillin was the first widely used antibiotic"),

        // === Environment ===
        CausalTrainingPair::new(
            "Burning fossil fuels releases CO2 into the atmosphere".into(),
            "Increased atmospheric CO2 traps heat and raises global temperatures".into(),
            TrainingDirection::Forward,
            0.95,
        )
        .with_mechanism("physical")
        .with_domain("environment")
        .with_hard_negative("Carbon dioxide is a colorless, odorless gas at standard conditions"),
        CausalTrainingPair::new(
            "Deforestation eliminates carbon sinks and disrupts water cycles".into(),
            "Loss of forest cover accelerates soil erosion and regional drought".into(),
            TrainingDirection::Forward,
            0.87,
        )
        .with_mechanism("ecological")
        .with_domain("environment")
        .with_hard_negative("Forests cover approximately 31% of the global land area"),
        CausalTrainingPair::new(
            "Ocean acidification from absorbed CO2 weakens coral skeletons".into(),
            "Weakened coral structures lead to reef collapse and marine biodiversity loss".into(),
            TrainingDirection::Forward,
            0.86,
        )
        .with_mechanism("chemical")
        .with_domain("environment")
        .with_hard_negative("The Great Barrier Reef is visible from space"),
        CausalTrainingPair::new(
            "Rising global temperatures accelerate polar ice melt".into(),
            "Melting ice raises sea levels and threatens coastal communities".into(),
            TrainingDirection::Forward,
            0.93,
        )
        .with_mechanism("physical")
        .with_domain("environment")
        .with_hard_negative("Antarctica contains approximately 26.5 million cubic kilometers of ice"),
        CausalTrainingPair::new(
            "Agricultural runoff introduces excess nitrogen and phosphorus into waterways".into(),
            "Nutrient pollution causes algal blooms that deplete dissolved oxygen".into(),
            TrainingDirection::Forward,
            0.84,
        )
        .with_mechanism("chemical")
        .with_domain("environment")
        .with_hard_negative("The nitrogen cycle involves fixation, nitrification, and denitrification"),
        CausalTrainingPair::new(
            "Plastic waste accumulates in ocean gyres".into(),
            "Marine animals ingest microplastics, causing bioaccumulation of toxins in food chains".into(),
            TrainingDirection::Forward,
            0.83,
        )
        .with_mechanism("ecological")
        .with_domain("environment")
        .with_hard_negative("Recycling rates vary significantly between different types of plastic"),

        // === Economics ===
        CausalTrainingPair::new(
            "Central banks raise interest rates to curb inflation".into(),
            "Higher interest rates reduce consumer borrowing and slow economic growth".into(),
            TrainingDirection::Forward,
            0.90,
        )
        .with_mechanism("economic")
        .with_domain("economics")
        .with_hard_negative("The Federal Reserve was established in 1913"),
        CausalTrainingPair::new(
            "Supply chain disruptions reduce the availability of goods".into(),
            "Reduced supply with constant demand drives price increases".into(),
            TrainingDirection::Forward,
            0.88,
        )
        .with_mechanism("economic")
        .with_domain("economics")
        .with_hard_negative("Supply chain management involves logistics, procurement, and inventory control"),
        CausalTrainingPair::new(
            "Automation replaces repetitive manual labor tasks".into(),
            "Workers in automated sectors face unemployment and need to reskill".into(),
            TrainingDirection::Forward,
            0.82,
        )
        .with_mechanism("economic")
        .with_domain("economics")
        .with_hard_negative("The unemployment rate measures the percentage of the labor force without jobs"),
        CausalTrainingPair::new(
            "Government deficit spending increases money supply".into(),
            "Excess money supply relative to goods causes inflationary pressure".into(),
            TrainingDirection::Forward,
            0.85,
        )
        .with_mechanism("economic")
        .with_domain("economics")
        .with_hard_negative("Monetary policy tools include open market operations and reserve requirements"),
        CausalTrainingPair::new(
            "Trade tariffs increase the cost of imported goods".into(),
            "Higher import costs reduce consumer purchasing power and hurt import-dependent industries".into(),
            TrainingDirection::Forward,
            0.87,
        )
        .with_mechanism("economic")
        .with_domain("economics")
        .with_hard_negative("International trade agreements set rules for cross-border commerce"),
        CausalTrainingPair::new(
            "A housing market bubble inflates property values beyond fundamentals".into(),
            "When the bubble bursts, negative equity and foreclosures trigger a financial crisis".into(),
            TrainingDirection::Forward,
            0.88,
        )
        .with_mechanism("economic")
        .with_domain("economics")
        .with_hard_negative("Mortgage interest rates are influenced by the federal funds rate"),

        // === Technology ===
        CausalTrainingPair::new(
            "Memory leaks in long-running processes accumulate unreleased allocations".into(),
            "Accumulated memory leaks cause out-of-memory crashes and service degradation".into(),
            TrainingDirection::Forward,
            0.92,
        )
        .with_mechanism("technical")
        .with_domain("technology")
        .with_hard_negative("Garbage collectors automatically reclaim unused memory in managed languages"),
        CausalTrainingPair::new(
            "SQL injection vulnerabilities allow attackers to execute arbitrary queries".into(),
            "Unauthorized database access leads to data breaches and privacy violations".into(),
            TrainingDirection::Forward,
            0.94,
        )
        .with_mechanism("technical")
        .with_domain("technology")
        .with_hard_negative("Prepared statements are a common defense against SQL injection"),
        CausalTrainingPair::new(
            "Training neural networks on biased datasets encodes discriminatory patterns".into(),
            "Biased AI models produce unfair outcomes in hiring, lending, and policing".into(),
            TrainingDirection::Forward,
            0.86,
        )
        .with_mechanism("technical")
        .with_domain("technology")
        .with_hard_negative("Machine learning models learn patterns from labeled training data"),
        CausalTrainingPair::new(
            "Network congestion from excessive traffic exceeds bandwidth capacity".into(),
            "Packet loss and latency spikes degrade application performance".into(),
            TrainingDirection::Forward,
            0.89,
        )
        .with_mechanism("technical")
        .with_domain("technology")
        .with_hard_negative("TCP uses flow control and congestion avoidance algorithms"),
        CausalTrainingPair::new(
            "Distributed systems lack a single source of truth for state".into(),
            "Concurrent writes without coordination cause data inconsistency and split-brain".into(),
            TrainingDirection::Forward,
            0.84,
        )
        .with_mechanism("technical")
        .with_domain("technology")
        .with_hard_negative("The CAP theorem constrains distributed database design choices"),

        // === Social ===
        CausalTrainingPair::new(
            "Social media algorithms maximize engagement through emotionally charged content".into(),
            "Algorithmic amplification of outrage deepens political polarization".into(),
            TrainingDirection::Forward,
            0.81,
        )
        .with_mechanism("social")
        .with_domain("social")
        .with_hard_negative("Social media platforms generate revenue primarily through advertising"),
        CausalTrainingPair::new(
            "Income inequality limits access to quality education and healthcare".into(),
            "Lack of equal opportunity perpetuates cycles of poverty across generations".into(),
            TrainingDirection::Forward,
            0.80,
        )
        .with_mechanism("social")
        .with_domain("social")
        .with_hard_negative("The Gini coefficient measures statistical dispersion of income"),
        CausalTrainingPair::new(
            "Lead exposure in childhood impairs neurodevelopment".into(),
            "Cognitive deficits from lead poisoning reduce educational attainment and earning potential".into(),
            TrainingDirection::Forward,
            0.91,
        )
        .with_mechanism("biological")
        .with_domain("social")
        .with_hard_negative("Lead paint was banned in US residential properties in 1978"),
        CausalTrainingPair::new(
            "Urban sprawl increases commute distances and car dependency".into(),
            "Car-centric planning contributes to air pollution and sedentary lifestyles".into(),
            TrainingDirection::Forward,
            0.79,
        )
        .with_mechanism("social")
        .with_domain("social")
        .with_hard_negative("Public transit ridership varies significantly between cities"),

        // === Non-causal pairs (hard negatives for training) ===
        CausalTrainingPair::new(
            "The Pacific Ocean is the largest ocean on Earth".into(),
            "Coral reefs support approximately 25% of marine species".into(),
            TrainingDirection::None,
            0.05,
        )
        .with_domain("environment")
        .with_hard_negative("Oceanography studies the physical and biological properties of the ocean"),
        CausalTrainingPair::new(
            "Python is a high-level programming language".into(),
            "Machine learning models require large datasets for training".into(),
            TrainingDirection::None,
            0.10,
        )
        .with_domain("technology")
        .with_hard_negative("Programming languages have different paradigms including OOP and functional"),
        CausalTrainingPair::new(
            "The Eiffel Tower is located in Paris, France".into(),
            "Tourism contributes significantly to France's GDP".into(),
            TrainingDirection::None,
            0.15,
        )
        .with_domain("economics")
        .with_hard_negative("France is the most visited country in the world by tourist arrivals"),
        CausalTrainingPair::new(
            "DNA consists of four nucleotide bases: A, T, G, and C".into(),
            "Proteins are synthesized by ribosomes in the cytoplasm".into(),
            TrainingDirection::None,
            0.12,
        )
        .with_domain("health")
        .with_hard_negative("Molecular biology studies the structure and function of macromolecules"),
    ]
}

/// Response format from LLM training data generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmTrainingPairResponse {
    /// Paraphrased cause text.
    pub paraphrased_cause: String,
    /// Paraphrased effect text.
    pub paraphrased_effect: String,
    /// Hard negative: topically similar but non-causal.
    pub hard_negative: String,
    /// Explanation of WHY this is causal.
    pub rationale: String,
    /// LLM confidence in the causal link.
    pub confidence: f32,
    /// Domain category.
    pub domain: String,
}

/// GBNF grammar for training pair generation.
pub const TRAINING_PAIR_GRAMMAR: &str = r#"root ::= "{" ws paraphrased-cause "," ws paraphrased-effect "," ws hard-negative "," ws rationale "," ws confidence "," ws domain ws "}"
paraphrased-cause ::= "\"paraphrased_cause\"" ws ":" ws string
paraphrased-effect ::= "\"paraphrased_effect\"" ws ":" ws string
hard-negative ::= "\"hard_negative\"" ws ":" ws string
rationale ::= "\"rationale\"" ws ":" ws string
confidence ::= "\"confidence\"" ws ":" ws number
domain ::= "\"domain\"" ws ":" ws domain-value
domain-value ::= "\"health\"" | "\"environment\"" | "\"economics\"" | "\"technology\"" | "\"social\"" | "\"general\""
number ::= "0" ("." [0-9] [0-9]?)? | "1" ("." "0" "0"?)?
string ::= "\"" ([^"\\] | "\\" .)* "\""
ws ::= [ \t\n\r]*"#;

/// Save training pairs to JSONL file.
pub fn save_pairs_jsonl(
    pairs: &[CausalTrainingPair],
    path: &std::path::Path,
) -> std::io::Result<()> {
    use std::io::Write;
    let file = std::fs::File::create(path)?;
    let mut writer = std::io::BufWriter::new(file);
    for pair in pairs {
        let json = serde_json::to_string(pair).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
        })?;
        writeln!(writer, "{}", json)?;
    }
    Ok(())
}

/// Load training pairs from JSONL file.
pub fn load_pairs_jsonl(path: &std::path::Path) -> std::io::Result<Vec<CausalTrainingPair>> {
    use std::io::BufRead;
    let file = std::fs::File::open(path)?;
    let reader = std::io::BufReader::new(file);
    let mut pairs = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let pair: CausalTrainingPair = serde_json::from_str(&line).map_err(|e| {
            std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string())
        })?;
        pairs.push(pair);
    }
    Ok(pairs)
}

/// Expand seed pairs to ~500+ training examples via programmatic augmentation.
///
/// Expansion strategies:
/// 1. **Reversed pairs**: Swap cause/effect text, direction becomes Backward (~50 new pairs)
/// 2. **Non-causal negatives**: Cross-domain pairing of unrelated cause/effect texts (~100+ pairs)
/// 3. **Cross-domain hard negatives**: Same domain, different relationship (added to hard_negative field)
pub fn expand_seed_pairs(pairs: &[CausalTrainingPair]) -> Vec<CausalTrainingPair> {
    let mut expanded: Vec<CausalTrainingPair> = pairs.to_vec();

    // 1. Reversed pairs: swap cause/effect, direction becomes Backward
    let reversed: Vec<CausalTrainingPair> = pairs
        .iter()
        .filter(|p| matches!(p.direction, TrainingDirection::Forward))
        .map(|p| {
            CausalTrainingPair::new(
                p.effect_text.clone(),
                p.cause_text.clone(),
                TrainingDirection::Backward,
                p.confidence,
            )
            .with_mechanism(p.mechanism.clone())
            .with_domain(p.domain.clone())
            .with_hard_negative(p.hard_negative.clone())
        })
        .collect();
    expanded.extend(reversed);

    // 2. Non-causal negatives: cross-domain pairing
    let non_causal = generate_non_causal_pairs(pairs);
    expanded.extend(non_causal);

    // 3. Cross-domain hard negatives: pair cause from one relationship
    //    with effect from a different relationship in the same domain
    let causal_pairs: Vec<&CausalTrainingPair> = pairs
        .iter()
        .filter(|p| p.is_causal())
        .collect();

    for i in 0..causal_pairs.len() {
        for j in (i + 1)..causal_pairs.len() {
            let a = causal_pairs[i];
            let b = causal_pairs[j];

            // Same domain, different relationship → hard negative
            if a.domain == b.domain {
                let mut hard_neg_pair = CausalTrainingPair::new(
                    a.cause_text.clone(),
                    b.effect_text.clone(),
                    TrainingDirection::None,
                    0.15,
                )
                .with_domain(a.domain.clone())
                .with_mechanism("cross_relationship");

                // Set the actual matching effect as hard_negative context
                hard_neg_pair.hard_negative = a.effect_text.clone();
                expanded.push(hard_neg_pair);
            }
        }
    }

    expanded
}

/// Generate non-causal training pairs by cross-domain pairing.
///
/// Takes cause texts from one domain and pairs them with effect texts from
/// unrelated domains. These serve as explicit negative examples.
pub fn generate_non_causal_pairs(pairs: &[CausalTrainingPair]) -> Vec<CausalTrainingPair> {
    let mut non_causal = Vec::new();

    // Group causal pairs by domain
    let mut by_domain: std::collections::HashMap<&str, Vec<&CausalTrainingPair>> =
        std::collections::HashMap::new();
    for pair in pairs.iter().filter(|p| p.is_causal()) {
        by_domain.entry(pair.domain.as_str()).or_default().push(pair);
    }

    let domains: Vec<&str> = by_domain.keys().copied().collect();

    for (i, &domain_a) in domains.iter().enumerate() {
        for &domain_b in domains.iter().skip(i + 1) {
            let pairs_a = &by_domain[domain_a];
            let pairs_b = &by_domain[domain_b];

            // Take up to 3 cross-domain pairs per domain combination
            for (idx, pair_a) in pairs_a.iter().enumerate().take(3) {
                if let Some(pair_b) = pairs_b.get(idx % pairs_b.len()) {
                    non_causal.push(
                        CausalTrainingPair::new(
                            pair_a.cause_text.clone(),
                            pair_b.effect_text.clone(),
                            TrainingDirection::None,
                            0.05,
                        )
                        .with_domain("cross_domain")
                        .with_mechanism("non_causal")
                        .with_hard_negative(pair_a.effect_text.clone()),
                    );
                }
            }
        }
    }

    non_causal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seed_pairs_coverage() {
        let pairs = seed_training_pairs();
        assert!(pairs.len() >= 30, "Should have at least 30 seed pairs");

        // Check domain coverage
        let domains: std::collections::HashSet<_> = pairs.iter().map(|p| p.domain.as_str()).collect();
        assert!(domains.contains("health"), "Missing health domain");
        assert!(domains.contains("environment"), "Missing environment domain");
        assert!(domains.contains("economics"), "Missing economics domain");
        assert!(domains.contains("technology"), "Missing technology domain");
        assert!(domains.contains("social"), "Missing social domain");
    }

    #[test]
    fn test_seed_pairs_have_hard_negatives() {
        let pairs = seed_training_pairs();
        let with_negatives = pairs.iter().filter(|p| !p.hard_negative.is_empty()).count();
        assert!(
            with_negatives >= 25,
            "At least 25 seed pairs should have hard negatives, got {}",
            with_negatives
        );
    }

    #[test]
    fn test_training_direction_parsing() {
        assert_eq!(TrainingDirection::from_str("forward"), TrainingDirection::Forward);
        assert_eq!(TrainingDirection::from_str("A_causes_B"), TrainingDirection::Forward);
        assert_eq!(TrainingDirection::from_str("backward"), TrainingDirection::Backward);
        assert_eq!(TrainingDirection::from_str("bidirectional"), TrainingDirection::Bidirectional);
        assert_eq!(TrainingDirection::from_str("none"), TrainingDirection::None);
        assert_eq!(TrainingDirection::from_str("garbage"), TrainingDirection::None);
    }

    #[test]
    fn test_difficulty_levels() {
        let easy = CausalTrainingPair::new(
            "Stress causes insomnia because of cortisol".into(),
            "Insomnia therefore leads to fatigue".into(),
            TrainingDirection::Forward,
            0.9,
        );
        assert!(easy.difficulty() < 0.5, "Explicit markers should be easy");

        let non_causal = CausalTrainingPair::new(
            "The sky is blue".into(),
            "Water is wet".into(),
            TrainingDirection::None,
            0.1,
        );
        assert_eq!(non_causal.difficulty(), 0.0, "Non-causal should be difficulty 0");
    }

    #[test]
    fn test_data_loader_batching() {
        let pairs = seed_training_pairs();
        let total = pairs.len();
        let mut loader = CausalDataLoader::new(pairs, 8, 42);
        assert_eq!(loader.num_batches(), (total + 7) / 8);

        loader.shuffle_epoch();
        let mut total_seen = 0;
        let mut batch_idx = 0;
        while let Some(batch) = loader.next_batch(batch_idx) {
            total_seen += batch.len();
            batch_idx += 1;
        }
        assert_eq!(total_seen, total, "Should see all pairs across batches");
    }

    #[test]
    fn test_jsonl_round_trip() {
        let pairs = vec![
            CausalTrainingPair::new(
                "A causes B".into(),
                "B is caused by A".into(),
                TrainingDirection::Forward,
                0.9,
            )
            .with_domain("test"),
        ];

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.jsonl");

        save_pairs_jsonl(&pairs, &path).unwrap();
        let loaded = load_pairs_jsonl(&path).unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].cause_text, "A causes B");
        assert_eq!(loaded[0].domain, "test");
    }
}
