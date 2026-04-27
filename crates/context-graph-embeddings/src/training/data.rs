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
    #[allow(clippy::should_implement_trait)]
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
        self.pairs.len().div_ceil(self.batch_size)
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
}

/// Seed causal training pairs spanning multiple domains.
///
/// Returns 250+ high-quality seed pairs for LLM paraphrase expansion.
/// Domains: health, environment, economics, technology, social, physics,
/// nutrition, cybersecurity, psychology, history, legal, engineering.
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

        // === Physics ===
        CausalTrainingPair::new(
            "Heating a gas in a closed container increases molecular kinetic energy".into(),
            "Increased molecular collisions raise pressure inside the container".into(),
            TrainingDirection::Forward,
            0.94,
        )
        .with_mechanism("physical")
        .with_domain("physics")
        .with_hard_negative("The ideal gas law relates pressure, volume, and temperature"),
        CausalTrainingPair::new(
            "Gravitational attraction between two massive bodies".into(),
            "Orbital motion of planets around stars follows Keplerian trajectories".into(),
            TrainingDirection::Forward,
            0.92,
        )
        .with_mechanism("physical")
        .with_domain("physics")
        .with_hard_negative("Kepler's laws describe the motion of planets in the solar system"),
        CausalTrainingPair::new(
            "Electric current flowing through a resistor dissipates energy as heat".into(),
            "Joule heating raises the temperature of the conductor".into(),
            TrainingDirection::Forward,
            0.91,
        )
        .with_mechanism("physical")
        .with_domain("physics")
        .with_hard_negative("Ohm's law states voltage equals current times resistance"),
        CausalTrainingPair::new(
            "A net external force acts on a stationary object".into(),
            "The object accelerates in the direction of the applied force".into(),
            TrainingDirection::Forward,
            0.96,
        )
        .with_mechanism("physical")
        .with_domain("physics")
        .with_hard_negative("Newton's three laws of motion form the basis of classical mechanics"),
        CausalTrainingPair::new(
            "Electromagnetic radiation strikes a metal surface with photon energy above the work function".into(),
            "Electrons are ejected from the metal via the photoelectric effect".into(),
            TrainingDirection::Forward,
            0.93,
        )
        .with_mechanism("quantum")
        .with_domain("physics")
        .with_hard_negative("Einstein won the Nobel Prize for his explanation of the photoelectric effect"),

        // === Nutrition ===
        CausalTrainingPair::new(
            "Excessive refined sugar consumption causes rapid blood glucose spikes".into(),
            "Repeated glucose spikes promote insulin resistance and metabolic syndrome".into(),
            TrainingDirection::Forward,
            0.88,
        )
        .with_mechanism("biological")
        .with_domain("nutrition")
        .with_hard_negative("The glycemic index ranks carbohydrates by their effect on blood glucose"),
        CausalTrainingPair::new(
            "Vitamin D deficiency impairs calcium absorption in the intestine".into(),
            "Inadequate calcium leads to decreased bone density and osteoporosis risk".into(),
            TrainingDirection::Forward,
            0.90,
        )
        .with_mechanism("biological")
        .with_domain("nutrition")
        .with_hard_negative("Dairy products are a common dietary source of calcium"),
        CausalTrainingPair::new(
            "Chronic iron deficiency reduces hemoglobin production".into(),
            "Low hemoglobin impairs oxygen transport, causing fatigue and anemia".into(),
            TrainingDirection::Forward,
            0.91,
        )
        .with_mechanism("biological")
        .with_domain("nutrition")
        .with_hard_negative("Red meat and leafy greens are rich sources of dietary iron"),
        CausalTrainingPair::new(
            "High dietary fiber intake promotes beneficial gut bacteria growth".into(),
            "A healthy microbiome improves nutrient absorption and immune function".into(),
            TrainingDirection::Forward,
            0.83,
        )
        .with_mechanism("biological")
        .with_domain("nutrition")
        .with_hard_negative("The recommended daily fiber intake is 25-30 grams for adults"),
        CausalTrainingPair::new(
            "Excess caloric intake beyond daily energy expenditure".into(),
            "Surplus energy is stored as adipose tissue, leading to weight gain".into(),
            TrainingDirection::Forward,
            0.93,
        )
        .with_mechanism("metabolic")
        .with_domain("nutrition")
        .with_hard_negative("Basal metabolic rate accounts for 60-70% of daily energy expenditure"),

        // === Cybersecurity ===
        CausalTrainingPair::new(
            "Phishing emails trick users into revealing credentials".into(),
            "Stolen credentials enable unauthorized access to corporate networks".into(),
            TrainingDirection::Forward,
            0.92,
        )
        .with_mechanism("technical")
        .with_domain("cybersecurity")
        .with_hard_negative("Multi-factor authentication adds an additional layer of security"),
        CausalTrainingPair::new(
            "Unpatched software vulnerabilities expose exploitable attack surfaces".into(),
            "Attackers gain remote code execution through known CVEs".into(),
            TrainingDirection::Forward,
            0.94,
        )
        .with_mechanism("technical")
        .with_domain("cybersecurity")
        .with_hard_negative("The CVE database catalogs publicly disclosed cybersecurity vulnerabilities"),
        CausalTrainingPair::new(
            "Ransomware encrypts files on the victim's system".into(),
            "Organizations lose access to critical data and face operational disruption".into(),
            TrainingDirection::Forward,
            0.91,
        )
        .with_mechanism("technical")
        .with_domain("cybersecurity")
        .with_hard_negative("Regular offline backups are a key defense against ransomware"),
        CausalTrainingPair::new(
            "Weak password policies allow brute-force credential guessing".into(),
            "Compromised accounts provide lateral movement across the network".into(),
            TrainingDirection::Forward,
            0.87,
        )
        .with_mechanism("technical")
        .with_domain("cybersecurity")
        .with_hard_negative("Password managers generate and store complex unique passwords"),
        CausalTrainingPair::new(
            "Supply chain compromise injects malicious code into trusted software updates".into(),
            "Thousands of downstream users unknowingly install backdoored software".into(),
            TrainingDirection::Forward,
            0.90,
        )
        .with_mechanism("technical")
        .with_domain("cybersecurity")
        .with_hard_negative("Software bill of materials tracks third-party dependencies"),

        // === Psychology ===
        CausalTrainingPair::new(
            "Early childhood trauma disrupts attachment bond formation".into(),
            "Insecure attachment patterns persist into adult relationships".into(),
            TrainingDirection::Forward,
            0.84,
        )
        .with_mechanism("psychological")
        .with_domain("psychology")
        .with_hard_negative("Attachment theory was developed by John Bowlby in the 1960s"),
        CausalTrainingPair::new(
            "Chronic social isolation reduces dopamine reward circuit activation".into(),
            "Diminished reward response contributes to depression and anhedonia".into(),
            TrainingDirection::Forward,
            0.82,
        )
        .with_mechanism("neuropsychological")
        .with_domain("psychology")
        .with_hard_negative("Dopamine is a neurotransmitter involved in reward and motivation"),
        CausalTrainingPair::new(
            "Repeated exposure to feared stimuli without negative consequences".into(),
            "Fear response gradually extinguishes through habituation".into(),
            TrainingDirection::Forward,
            0.89,
        )
        .with_mechanism("behavioral")
        .with_domain("psychology")
        .with_hard_negative("Exposure therapy is based on principles of classical conditioning"),
        CausalTrainingPair::new(
            "Cognitive distortions magnify perceived threats and failures".into(),
            "Distorted thinking patterns maintain anxiety and depressive disorders".into(),
            TrainingDirection::Forward,
            0.86,
        )
        .with_mechanism("cognitive")
        .with_domain("psychology")
        .with_hard_negative("Cognitive behavioral therapy identifies and challenges thought patterns"),
        CausalTrainingPair::new(
            "Sleep deprivation impairs prefrontal cortex executive function".into(),
            "Reduced impulse control leads to poor decision-making and emotional dysregulation".into(),
            TrainingDirection::Forward,
            0.87,
        )
        .with_mechanism("neuropsychological")
        .with_domain("psychology")
        .with_hard_negative("Adults need 7-9 hours of sleep per night for optimal functioning"),

        // === History ===
        CausalTrainingPair::new(
            "The assassination of Archduke Franz Ferdinand destabilized European alliances".into(),
            "Cascading treaty obligations triggered the outbreak of World War I".into(),
            TrainingDirection::Forward,
            0.88,
        )
        .with_mechanism("political")
        .with_domain("history")
        .with_hard_negative("World War I lasted from 1914 to 1918"),
        CausalTrainingPair::new(
            "The invention of the printing press enabled mass production of texts".into(),
            "Widespread literacy and information access accelerated the Reformation and scientific revolution".into(),
            TrainingDirection::Forward,
            0.85,
        )
        .with_mechanism("technological")
        .with_domain("history")
        .with_hard_negative("Johannes Gutenberg introduced the movable-type printing press around 1440"),
        CausalTrainingPair::new(
            "The Black Death killed a third of Europe's population".into(),
            "Severe labor shortages shifted economic power to surviving workers and weakened feudalism".into(),
            TrainingDirection::Forward,
            0.87,
        )
        .with_mechanism("socioeconomic")
        .with_domain("history")
        .with_hard_negative("The Black Death peaked in Europe between 1347 and 1351"),
        CausalTrainingPair::new(
            "Harsh reparations imposed by the Treaty of Versailles crippled Germany's economy".into(),
            "Economic desperation and resentment fueled the rise of extremist political movements".into(),
            TrainingDirection::Forward,
            0.84,
        )
        .with_mechanism("political")
        .with_domain("history")
        .with_hard_negative("The Treaty of Versailles was signed on June 28, 1919"),
        CausalTrainingPair::new(
            "The Industrial Revolution mechanized manufacturing processes".into(),
            "Mass migration from rural areas to factory cities transformed social structures".into(),
            TrainingDirection::Forward,
            0.86,
        )
        .with_mechanism("socioeconomic")
        .with_domain("history")
        .with_hard_negative("The Industrial Revolution began in Britain in the late 18th century"),

        // === Non-causal pairs (hard negatives for training) ===
        CausalTrainingPair::new(
            "The Pacific Ocean is the largest ocean on Earth".into(),
            "Coral reefs support approximately 25% of marine species".into(),
            TrainingDirection::None,
            0.05,
        )
        .with_mechanism("observational")
        .with_domain("environment")
        .with_hard_negative("Oceanography studies the physical and biological properties of the ocean"),
        CausalTrainingPair::new(
            "Python is a high-level programming language".into(),
            "Machine learning models require large datasets for training".into(),
            TrainingDirection::None,
            0.10,
        )
        .with_mechanism("observational")
        .with_domain("technology")
        .with_hard_negative("Programming languages have different paradigms including OOP and functional"),
        CausalTrainingPair::new(
            "The Eiffel Tower is located in Paris, France".into(),
            "Tourism contributes significantly to France's GDP".into(),
            TrainingDirection::None,
            0.15,
        )
        .with_mechanism("observational")
        .with_domain("economics")
        .with_hard_negative("France is the most visited country in the world by tourist arrivals"),
        CausalTrainingPair::new(
            "DNA consists of four nucleotide bases: A, T, G, and C".into(),
            "Proteins are synthesized by ribosomes in the cytoplasm".into(),
            TrainingDirection::None,
            0.12,
        )
        .with_mechanism("observational")
        .with_domain("health")
        .with_hard_negative("Molecular biology studies the structure and function of macromolecules"),

        // ================================================================
        // NEW PAIRS: Legal domain (20 pairs)
        // ================================================================
        CausalTrainingPair::new(
            "A company fails to comply with GDPR data protection requirements".into(),
            "Regulatory authorities impose substantial fines and mandate corrective action".into(),
            TrainingDirection::Forward,
            0.93,
        )
        .with_mechanism("regulatory")
        .with_domain("legal")
        .with_hard_negative("The GDPR was adopted by the European Parliament in April 2016"),
        CausalTrainingPair::new(
            "Breach of fiduciary duty by corporate officers".into(),
            "Shareholders file derivative lawsuits seeking damages and injunctive relief".into(),
            TrainingDirection::Forward,
            0.88,
        )
        .with_mechanism("legal")
        .with_domain("legal")
        .with_hard_negative("Fiduciary duties include the duty of care and the duty of loyalty"),
        CausalTrainingPair::new(
            "Ambiguous contract language leaves key terms undefined".into(),
            "Disputes arise over interpretation, requiring judicial construction of intent".into(),
            TrainingDirection::Forward,
            0.85,
        )
        .with_mechanism("legal")
        .with_domain("legal")
        .with_hard_negative("The parol evidence rule limits extrinsic evidence in contract interpretation"),
        CausalTrainingPair::new(
            "Evidence obtained through an unlawful search violates the Fourth Amendment".into(),
            "Courts suppress the tainted evidence under the exclusionary rule".into(),
            TrainingDirection::Forward,
            0.94,
        )
        .with_mechanism("constitutional")
        .with_domain("legal")
        .with_hard_negative("The Fourth Amendment was ratified as part of the Bill of Rights in 1791"),
        CausalTrainingPair::new(
            "A manufacturer distributes a product with a known design defect".into(),
            "Injured consumers pursue strict liability claims for damages".into(),
            TrainingDirection::Forward,
            0.90,
        )
        .with_mechanism("tort")
        .with_domain("legal")
        .with_hard_negative("Product liability law varies between strict liability and negligence standards"),
        CausalTrainingPair::new(
            "Antitrust violations through price-fixing agreements among competitors".into(),
            "Regulatory agencies impose treble damages and criminal penalties".into(),
            TrainingDirection::Forward,
            0.92,
        )
        .with_mechanism("regulatory")
        .with_domain("legal")
        .with_hard_negative("The Sherman Antitrust Act was enacted in 1890"),
        CausalTrainingPair::new(
            "Failure to obtain informed consent before a medical procedure".into(),
            "Patients bring medical malpractice claims for battery or negligence".into(),
            TrainingDirection::Forward,
            0.87,
        )
        .with_mechanism("tort")
        .with_domain("legal")
        .with_hard_negative("Informed consent requires disclosure of risks, benefits, and alternatives"),
        CausalTrainingPair::new(
            "Wrongful termination in violation of anti-discrimination statutes".into(),
            "Former employees file Title VII complaints with the EEOC seeking reinstatement".into(),
            TrainingDirection::Forward,
            0.89,
        )
        .with_mechanism("statutory")
        .with_domain("legal")
        .with_hard_negative("Title VII of the Civil Rights Act prohibits employment discrimination"),
        CausalTrainingPair::new(
            "Unauthorized use of copyrighted material in a commercial product".into(),
            "The copyright holder obtains an injunction and statutory damages".into(),
            TrainingDirection::Forward,
            0.91,
        )
        .with_mechanism("intellectual_property")
        .with_domain("legal")
        .with_hard_negative("Copyright protection in the US lasts for the life of the author plus 70 years"),
        CausalTrainingPair::new(
            "Corporate executives engage in insider trading on material nonpublic information".into(),
            "The SEC brings enforcement actions resulting in disgorgement and civil penalties".into(),
            TrainingDirection::Forward,
            0.93,
        )
        .with_mechanism("regulatory")
        .with_domain("legal")
        .with_hard_negative("The Securities Exchange Act of 1934 governs secondary market transactions"),
        CausalTrainingPair::new(
            "A landlord fails to maintain habitable living conditions".into(),
            "Tenants exercise the right to withhold rent or pursue constructive eviction claims".into(),
            TrainingDirection::Forward,
            0.84,
        )
        .with_mechanism("contractual")
        .with_domain("legal")
        .with_hard_negative("The implied warranty of habitability applies in most residential lease agreements"),
        CausalTrainingPair::new(
            "Police interrogation without Miranda warnings on a custodial suspect".into(),
            "Statements obtained are inadmissible as evidence at trial".into(),
            TrainingDirection::Forward,
            0.95,
        )
        .with_mechanism("constitutional")
        .with_domain("legal")
        .with_hard_negative("Miranda v. Arizona was decided by the Supreme Court in 1966"),
        CausalTrainingPair::new(
            "Negligent misrepresentation in a securities prospectus".into(),
            "Investors suffer financial losses and bring Section 11 claims".into(),
            TrainingDirection::Forward,
            0.86,
        )
        .with_mechanism("statutory")
        .with_domain("legal")
        .with_hard_negative("The Securities Act of 1933 requires registration of public offerings"),
        CausalTrainingPair::new(
            "Environmental contamination by industrial discharge into waterways".into(),
            "The EPA initiates Superfund cleanup and imposes joint and several liability on responsible parties".into(),
            TrainingDirection::Forward,
            0.90,
        )
        .with_mechanism("regulatory")
        .with_domain("legal")
        .with_hard_negative("CERCLA established the Superfund program in 1980"),
        CausalTrainingPair::new(
            "A defendant's counsel provides constitutionally ineffective assistance".into(),
            "Appellate courts vacate the conviction under the Strickland standard".into(),
            TrainingDirection::Forward,
            0.83,
        )
        .with_mechanism("constitutional")
        .with_domain("legal")
        .with_hard_negative("The Sixth Amendment guarantees the right to counsel in criminal proceedings"),
        CausalTrainingPair::new(
            "Patent infringement by a competitor selling an identical device".into(),
            "The patent holder obtains injunctive relief and reasonable royalty damages".into(),
            TrainingDirection::Forward,
            0.88,
        )
        .with_mechanism("intellectual_property")
        .with_domain("legal")
        .with_hard_negative("Utility patents protect inventions for 20 years from the filing date"),
        CausalTrainingPair::new(
            "Fraudulent conveyance transfers assets to avoid creditor claims".into(),
            "Courts void the transfer and order asset recovery under the Uniform Fraudulent Transfer Act".into(),
            TrainingDirection::Forward,
            0.87,
        )
        .with_mechanism("equitable")
        .with_domain("legal")
        .with_hard_negative("Bankruptcy law provides an automatic stay against creditor collection actions"),
        // Implicit causal - legal
        CausalTrainingPair::new(
            "The merger created a dominant firm controlling 80% of the regional market".into(),
            "Consumer prices in the region climbed steadily for the following three years".into(),
            TrainingDirection::Forward,
            0.78,
        )
        .with_mechanism("economic")
        .with_domain("legal")
        .with_hard_negative("Market concentration is measured using the Herfindahl-Hirschman Index"),
        CausalTrainingPair::new(
            "Mandatory minimum sentencing statutes removed judicial discretion".into(),
            "Prison populations expanded dramatically while recidivism rates remained unchanged".into(),
            TrainingDirection::Forward,
            0.80,
        )
        .with_mechanism("policy")
        .with_domain("legal")
        .with_hard_negative("The US federal sentencing guidelines were first established in 1987"),
        CausalTrainingPair::new(
            "The new data privacy regulation imposed strict consent requirements on tech companies".into(),
            "Many smaller advertising firms could not afford compliance and exited the market".into(),
            TrainingDirection::Forward,
            0.76,
        )
        .with_mechanism("regulatory")
        .with_domain("legal")
        .with_hard_negative("Data privacy regulations differ between the European Union and the United States"),

        // ================================================================
        // NEW PAIRS: Engineering domain (20 pairs)
        // ================================================================
        CausalTrainingPair::new(
            "Cyclic thermal loading on welded steel joints induces fatigue crack initiation".into(),
            "Progressive crack growth culminates in brittle fracture at loads below yield strength".into(),
            TrainingDirection::Forward,
            0.91,
        )
        .with_mechanism("mechanical")
        .with_domain("engineering")
        .with_hard_negative("S-N curves characterize the fatigue life of materials under cyclic loading"),
        CausalTrainingPair::new(
            "Insufficient foundation depth in expansive clay soils".into(),
            "Seasonal moisture changes cause differential settlement and structural cracking".into(),
            TrainingDirection::Forward,
            0.89,
        )
        .with_mechanism("geotechnical")
        .with_domain("engineering")
        .with_hard_negative("Soil bearing capacity is determined through standard penetration tests"),
        CausalTrainingPair::new(
            "Resonant frequency excitation of a suspension bridge by wind vortex shedding".into(),
            "Aeroelastic flutter amplifies oscillations until structural failure occurs".into(),
            TrainingDirection::Forward,
            0.93,
        )
        .with_mechanism("structural")
        .with_domain("engineering")
        .with_hard_negative("The Tacoma Narrows Bridge collapsed in 1940 due to aerodynamic instability"),
        CausalTrainingPair::new(
            "Galvanic contact between dissimilar metals in a saltwater environment".into(),
            "Accelerated corrosion of the anodic metal compromises joint integrity".into(),
            TrainingDirection::Forward,
            0.90,
        )
        .with_mechanism("electrochemical")
        .with_domain("engineering")
        .with_hard_negative("The galvanic series ranks metals by their electrode potential in seawater"),
        CausalTrainingPair::new(
            "Excessive heat generation in power electronics without adequate heat sinking".into(),
            "Thermal runaway destroys semiconductor junctions and causes component failure".into(),
            TrainingDirection::Forward,
            0.92,
        )
        .with_mechanism("thermal")
        .with_domain("engineering")
        .with_hard_negative("Thermal interface materials reduce contact resistance between surfaces"),
        CausalTrainingPair::new(
            "Water hammer from rapid valve closure in a pressurized pipeline".into(),
            "Transient pressure spikes exceed pipe wall yield strength and cause rupture".into(),
            TrainingDirection::Forward,
            0.88,
        )
        .with_mechanism("hydraulic")
        .with_domain("engineering")
        .with_hard_negative("The Joukowsky equation estimates pressure rise from sudden flow stoppage"),
        CausalTrainingPair::new(
            "Improper concrete curing allows premature moisture loss from the surface".into(),
            "Drying shrinkage cracks propagate through the slab and reduce structural capacity".into(),
            TrainingDirection::Forward,
            0.85,
        )
        .with_mechanism("material")
        .with_domain("engineering")
        .with_hard_negative("Concrete achieves approximately 70% of its design strength after 7 days"),
        CausalTrainingPair::new(
            "Signal integrity degradation from impedance mismatch on high-speed PCB traces".into(),
            "Reflected signals corrupt data transmission and increase bit error rates".into(),
            TrainingDirection::Forward,
            0.87,
        )
        .with_mechanism("electrical")
        .with_domain("engineering")
        .with_hard_negative("Controlled impedance PCB traces typically target 50 ohms for single-ended signals"),
        CausalTrainingPair::new(
            "Bearing lubrication film breakdown under excessive load or temperature".into(),
            "Metal-to-metal contact causes rapid wear, seizure, and catastrophic bearing failure".into(),
            TrainingDirection::Forward,
            0.91,
        )
        .with_mechanism("tribological")
        .with_domain("engineering")
        .with_hard_negative("The Stribeck curve describes friction regimes from boundary to hydrodynamic lubrication"),
        CausalTrainingPair::new(
            "Exceeding the critical buckling load on a slender compression column".into(),
            "The column undergoes lateral deflection and sudden loss of load-carrying capacity".into(),
            TrainingDirection::Forward,
            0.94,
        )
        .with_mechanism("structural")
        .with_domain("engineering")
        .with_hard_negative("Euler's formula calculates the critical load for ideal elastic columns"),
        CausalTrainingPair::new(
            "Chloride ion penetration into reinforced concrete from deicing salts".into(),
            "Reinforcing steel undergoes pitting corrosion and expansive oxide formation spalls the cover".into(),
            TrainingDirection::Forward,
            0.86,
        )
        .with_mechanism("chemical")
        .with_domain("engineering")
        .with_hard_negative("Epoxy-coated rebar is used as a corrosion protection strategy in bridge decks"),
        CausalTrainingPair::new(
            "Control loop gain set too high in a feedback control system".into(),
            "The system oscillates with growing amplitude until saturation or mechanical damage".into(),
            TrainingDirection::Forward,
            0.89,
        )
        .with_mechanism("control")
        .with_domain("engineering")
        .with_hard_negative("PID controllers combine proportional, integral, and derivative actions"),
        CausalTrainingPair::new(
            "Hydrogen embrittlement from cathodic protection overprotection of steel".into(),
            "Atomic hydrogen diffuses into the lattice, reducing ductility and enabling sudden fracture".into(),
            TrainingDirection::Forward,
            0.87,
        )
        .with_mechanism("metallurgical")
        .with_domain("engineering")
        .with_hard_negative("Cathodic protection systems use sacrificial anodes or impressed current"),
        CausalTrainingPair::new(
            "Electromagnetic interference from unshielded power cables near signal wires".into(),
            "Induced noise corrupts sensor readings and degrades measurement accuracy".into(),
            TrainingDirection::Forward,
            0.84,
        )
        .with_mechanism("electromagnetic")
        .with_domain("engineering")
        .with_hard_negative("Twisted pair cables reduce electromagnetic interference through cancellation"),
        CausalTrainingPair::new(
            "Soil liquefaction during seismic events transforms saturated sand into fluid".into(),
            "Foundations lose bearing support and structures experience catastrophic tilting or collapse".into(),
            TrainingDirection::Forward,
            0.92,
        )
        .with_mechanism("geotechnical")
        .with_domain("engineering")
        .with_hard_negative("The standard penetration test N-value helps assess liquefaction potential"),
        // Implicit causal - engineering
        CausalTrainingPair::new(
            "The bridge deck was resurfaced with a heavier asphalt overlay than originally specified".into(),
            "Deflection measurements at midspan showed a 15% increase over the following inspection cycle".into(),
            TrainingDirection::Forward,
            0.75,
        )
        .with_mechanism("structural")
        .with_domain("engineering")
        .with_hard_negative("Bridge load ratings are periodically reassessed during routine inspections"),
        CausalTrainingPair::new(
            "The turbine blades were manufactured with a casting porosity slightly above specification".into(),
            "Field inspections revealed premature creep deformation after 8000 hours of service".into(),
            TrainingDirection::Forward,
            0.77,
        )
        .with_mechanism("material")
        .with_domain("engineering")
        .with_hard_negative("Investment casting is the standard manufacturing process for turbine blades"),
        CausalTrainingPair::new(
            "The HVAC system was sized based on outdated occupancy assumptions".into(),
            "Indoor air quality complaints and thermal discomfort increased after the building was renovated for open-plan offices".into(),
            TrainingDirection::Forward,
            0.73,
        )
        .with_mechanism("thermal")
        .with_domain("engineering")
        .with_hard_negative("ASHRAE Standard 62.1 establishes minimum ventilation rates for acceptable indoor air quality"),
        CausalTrainingPair::new(
            "A software update changed the PLC timing parameters for the assembly line".into(),
            "The downstream robotic welder began missing alignment targets intermittently".into(),
            TrainingDirection::Forward,
            0.74,
        )
        .with_mechanism("control")
        .with_domain("engineering")
        .with_hard_negative("Programmable logic controllers execute ladder logic programs in scan cycles"),
        CausalTrainingPair::new(
            "The retaining wall drainage system became clogged with silt over several years".into(),
            "Hydrostatic pressure built up behind the wall until visible displacement was observed".into(),
            TrainingDirection::Forward,
            0.78,
        )
        .with_mechanism("geotechnical")
        .with_domain("engineering")
        .with_hard_negative("Geotextile filter fabrics are commonly used behind retaining walls"),

        // ================================================================
        // NEW PAIRS: Additional Health pairs
        // ================================================================
        CausalTrainingPair::new(
            "Chronic exposure to fine particulate matter (PM2.5) triggers systemic inflammation".into(),
            "Persistent vascular inflammation accelerates atherosclerosis and cardiovascular events".into(),
            TrainingDirection::Forward,
            0.89,
        )
        .with_mechanism("biological")
        .with_domain("health")
        .with_hard_negative("Air quality index values above 150 are considered unhealthy for all groups"),
        CausalTrainingPair::new(
            "Prolonged sedentary behavior reduces peripheral insulin sensitivity".into(),
            "Impaired glucose uptake by skeletal muscle elevates fasting blood sugar over time".into(),
            TrainingDirection::Forward,
            0.84,
        )
        .with_mechanism("metabolic")
        .with_domain("health")
        .with_hard_negative("Standing desks have become popular in modern office environments"),
        CausalTrainingPair::new(
            "Chronic alcohol consumption damages hepatocytes through acetaldehyde toxicity".into(),
            "Progressive hepatocyte loss and fibrosis advance to cirrhosis and liver failure".into(),
            TrainingDirection::Forward,
            0.92,
        )
        .with_mechanism("biological")
        .with_domain("health")
        .with_hard_negative("The liver is the largest solid organ in the human body"),
        CausalTrainingPair::new(
            "Persistent Helicobacter pylori infection erodes the gastric mucosal barrier".into(),
            "Chronic gastric inflammation progresses to peptic ulcer disease".into(),
            TrainingDirection::Forward,
            0.90,
        )
        .with_mechanism("biological")
        .with_domain("health")
        .with_hard_negative("H. pylori colonizes approximately half of the world's population"),
        CausalTrainingPair::new(
            "Repeated concussive impacts to the head accumulate tau protein deposits".into(),
            "Tau aggregation in frontal and temporal lobes manifests as chronic traumatic encephalopathy".into(),
            TrainingDirection::Forward,
            0.86,
        )
        .with_mechanism("neurological")
        .with_domain("health")
        .with_hard_negative("CTE can currently only be definitively diagnosed through post-mortem examination"),

        // ================================================================
        // NEW PAIRS: Additional Environment pairs
        // ================================================================
        CausalTrainingPair::new(
            "Permafrost thawing releases trapped methane and CO2 into the atmosphere".into(),
            "Additional greenhouse gas emissions amplify warming in a positive feedback loop".into(),
            TrainingDirection::Forward,
            0.88,
        )
        .with_mechanism("climate")
        .with_domain("environment")
        .with_hard_negative("Permafrost underlies approximately 25% of the Northern Hemisphere's land surface"),
        CausalTrainingPair::new(
            "Introduction of invasive species disrupts established predator-prey dynamics".into(),
            "Native species populations decline as they face novel competition and predation".into(),
            TrainingDirection::Forward,
            0.85,
        )
        .with_mechanism("ecological")
        .with_domain("environment")
        .with_hard_negative("Invasive species management costs billions of dollars annually worldwide"),
        CausalTrainingPair::new(
            "Overextraction of groundwater exceeds natural aquifer recharge rates".into(),
            "Water tables drop permanently and land subsidence damages surface infrastructure".into(),
            TrainingDirection::Forward,
            0.87,
        )
        .with_mechanism("hydrological")
        .with_domain("environment")
        .with_hard_negative("The Ogallala Aquifer is one of the largest freshwater aquifers in the world"),
        CausalTrainingPair::new(
            "Light pollution from urban areas disrupts nocturnal wildlife navigation".into(),
            "Migratory bird mortality increases as species collide with illuminated structures".into(),
            TrainingDirection::Forward,
            0.80,
        )
        .with_mechanism("ecological")
        .with_domain("environment")
        .with_hard_negative("The International Dark-Sky Association promotes responsible outdoor lighting"),
        CausalTrainingPair::new(
            "Wetland drainage for agricultural expansion eliminates natural flood buffers".into(),
            "Downstream communities experience more severe and frequent flooding events".into(),
            TrainingDirection::Forward,
            0.83,
        )
        .with_mechanism("hydrological")
        .with_domain("environment")
        .with_hard_negative("Wetlands cover approximately 6% of the Earth's land surface"),

        // ================================================================
        // NEW PAIRS: Additional Economics pairs
        // ================================================================
        CausalTrainingPair::new(
            "Quantitative easing floods financial markets with central bank liquidity".into(),
            "Asset prices inflate beyond fundamental valuations as investors chase yield".into(),
            TrainingDirection::Forward,
            0.86,
        )
        .with_mechanism("monetary")
        .with_domain("economics")
        .with_hard_negative("The Federal Reserve's balance sheet expanded significantly after 2008"),
        CausalTrainingPair::new(
            "Rapid currency devaluation erodes purchasing power of domestic savings".into(),
            "Capital flight accelerates as investors seek stable foreign denominations".into(),
            TrainingDirection::Forward,
            0.88,
        )
        .with_mechanism("economic")
        .with_domain("economics")
        .with_hard_negative("Exchange rates are determined by supply and demand in foreign exchange markets"),
        CausalTrainingPair::new(
            "Demographic aging shrinks the working-age population relative to retirees".into(),
            "Social security systems face funding shortfalls as dependency ratios worsen".into(),
            TrainingDirection::Forward,
            0.83,
        )
        .with_mechanism("demographic")
        .with_domain("economics")
        .with_hard_negative("Japan has one of the oldest populations in the world by median age"),
        CausalTrainingPair::new(
            "Monopolistic market structures eliminate competitive pricing pressure".into(),
            "Consumer welfare declines as prices rise and product innovation stagnates".into(),
            TrainingDirection::Forward,
            0.85,
        )
        .with_mechanism("economic")
        .with_domain("economics")
        .with_hard_negative("Perfect competition assumes many buyers and sellers with homogeneous products"),
        CausalTrainingPair::new(
            "Pandemic lockdowns shuttered service sector businesses for months".into(),
            "Unemployment surged and consumer spending contracted sharply in affected economies".into(),
            TrainingDirection::Forward,
            0.91,
        )
        .with_mechanism("economic")
        .with_domain("economics")
        .with_hard_negative("GDP measures the total value of goods and services produced within a country"),

        // ================================================================
        // NEW PAIRS: Additional Technology pairs
        // ================================================================
        CausalTrainingPair::new(
            "Insufficient input validation in a web API endpoint".into(),
            "Attackers craft malicious payloads that achieve remote code execution on the server".into(),
            TrainingDirection::Forward,
            0.93,
        )
        .with_mechanism("technical")
        .with_domain("technology")
        .with_hard_negative("The OWASP Top 10 lists the most critical web application security risks"),
        CausalTrainingPair::new(
            "Database index fragmentation accumulates as rows are inserted and deleted".into(),
            "Query response times degrade progressively until index maintenance is performed".into(),
            TrainingDirection::Forward,
            0.85,
        )
        .with_mechanism("technical")
        .with_domain("technology")
        .with_hard_negative("B-tree indexes are the most common index structure in relational databases"),
        CausalTrainingPair::new(
            "Tight coupling between microservices creates hidden runtime dependencies".into(),
            "Failure of one service cascades through the dependency chain and brings down the entire platform".into(),
            TrainingDirection::Forward,
            0.87,
        )
        .with_mechanism("architectural")
        .with_domain("technology")
        .with_hard_negative("Circuit breaker patterns are used to prevent cascade failures in distributed systems"),
        CausalTrainingPair::new(
            "Training a large language model on uncurated internet text".into(),
            "The model reproduces toxic content and confidential information from the training corpus".into(),
            TrainingDirection::Forward,
            0.84,
        )
        .with_mechanism("technical")
        .with_domain("technology")
        .with_hard_negative("Reinforcement learning from human feedback is used to align language model outputs"),
        CausalTrainingPair::new(
            "Clock skew between distributed system nodes exceeds tolerance thresholds".into(),
            "Timestamp-based ordering assumptions break and transactions are incorrectly serialized".into(),
            TrainingDirection::Forward,
            0.86,
        )
        .with_mechanism("technical")
        .with_domain("technology")
        .with_hard_negative("Network Time Protocol synchronizes clocks across computer networks"),

        // ================================================================
        // NEW PAIRS: Additional Social pairs
        // ================================================================
        CausalTrainingPair::new(
            "Concentrated poverty in neighborhoods limits access to quality schools and jobs".into(),
            "Residents face diminished upward mobility and intergenerational disadvantage persists".into(),
            TrainingDirection::Forward,
            0.82,
        )
        .with_mechanism("socioeconomic")
        .with_domain("social")
        .with_hard_negative("Neighborhood effects research examines how place influences individual outcomes"),
        CausalTrainingPair::new(
            "Mass incarceration removes working-age adults from communities".into(),
            "Family instability increases and local economic activity contracts".into(),
            TrainingDirection::Forward,
            0.81,
        )
        .with_mechanism("social")
        .with_domain("social")
        .with_hard_negative("The United States has the highest incarceration rate in the world"),
        CausalTrainingPair::new(
            "Maternal education levels strongly predict childhood health outcomes".into(),
            "Children of more educated mothers experience lower infant mortality and better nutrition".into(),
            TrainingDirection::Forward,
            0.85,
        )
        .with_mechanism("social")
        .with_domain("social")
        .with_hard_negative("Global female literacy rates have improved significantly over the past century"),
        CausalTrainingPair::new(
            "Disinformation campaigns deliberately erode public trust in institutions".into(),
            "Vaccine hesitancy and democratic disengagement increase in targeted populations".into(),
            TrainingDirection::Forward,
            0.83,
        )
        .with_mechanism("informational")
        .with_domain("social")
        .with_hard_negative("Fact-checking organizations evaluate the accuracy of public claims"),
        CausalTrainingPair::new(
            "Gentrification displaces long-term residents through rising property costs".into(),
            "Community social networks fracture and cultural institutions lose their patronage base".into(),
            TrainingDirection::Forward,
            0.80,
        )
        .with_mechanism("socioeconomic")
        .with_domain("social")
        .with_hard_negative("Urban renewal policies have been debated since the mid-20th century"),

        // ================================================================
        // NEW PAIRS: Additional Physics pairs
        // ================================================================
        CausalTrainingPair::new(
            "A charged particle moving through a magnetic field experiences the Lorentz force".into(),
            "The particle follows a curved helical trajectory perpendicular to the field lines".into(),
            TrainingDirection::Forward,
            0.94,
        )
        .with_mechanism("electromagnetic")
        .with_domain("physics")
        .with_hard_negative("Magnetic fields are measured in units of tesla or gauss"),
        CausalTrainingPair::new(
            "Nuclear fission splits a heavy nucleus into lighter fragments".into(),
            "The mass deficit is converted to kinetic energy per the mass-energy equivalence relation".into(),
            TrainingDirection::Forward,
            0.95,
        )
        .with_mechanism("nuclear")
        .with_domain("physics")
        .with_hard_negative("Uranium-235 is the primary fissile isotope used in nuclear reactors"),
        CausalTrainingPair::new(
            "Constructive interference occurs when two coherent waves arrive in phase".into(),
            "The combined amplitude at the interference point is the sum of individual amplitudes".into(),
            TrainingDirection::Forward,
            0.93,
        )
        .with_mechanism("wave")
        .with_domain("physics")
        .with_hard_negative("Young's double-slit experiment demonstrates the wave nature of light"),
        CausalTrainingPair::new(
            "A temperature gradient exists across a solid material boundary".into(),
            "Heat energy flows from the higher temperature region to the lower temperature region via conduction".into(),
            TrainingDirection::Forward,
            0.95,
        )
        .with_mechanism("thermodynamic")
        .with_domain("physics")
        .with_hard_negative("Fourier's law relates heat flux to the temperature gradient in a material"),
        CausalTrainingPair::new(
            "Reducing the volume of an incompressible fluid in a closed hydraulic system".into(),
            "Pressure increases uniformly throughout the fluid per Pascal's principle".into(),
            TrainingDirection::Forward,
            0.92,
        )
        .with_mechanism("fluid")
        .with_domain("physics")
        .with_hard_negative("Hydraulic systems are used in vehicle braking systems and heavy machinery"),

        // ================================================================
        // NEW PAIRS: Additional Nutrition pairs
        // ================================================================
        CausalTrainingPair::new(
            "Chronic vitamin B12 deficiency impairs myelin sheath maintenance".into(),
            "Demyelination of peripheral nerves produces numbness and neurological dysfunction".into(),
            TrainingDirection::Forward,
            0.88,
        )
        .with_mechanism("biological")
        .with_domain("nutrition")
        .with_hard_negative("Vitamin B12 is primarily found in animal-derived foods"),
        CausalTrainingPair::new(
            "Excessive trans fat consumption raises LDL and lowers HDL cholesterol".into(),
            "Unfavorable lipid profiles increase coronary artery disease risk".into(),
            TrainingDirection::Forward,
            0.90,
        )
        .with_mechanism("metabolic")
        .with_domain("nutrition")
        .with_hard_negative("The FDA banned artificial trans fats in processed foods in 2018"),
        CausalTrainingPair::new(
            "Severe protein-calorie malnutrition in early childhood".into(),
            "Impaired organ development and stunted growth with lasting cognitive deficits".into(),
            TrainingDirection::Forward,
            0.93,
        )
        .with_mechanism("developmental")
        .with_domain("nutrition")
        .with_hard_negative("The WHO Growth Standards chart pediatric development milestones"),
        CausalTrainingPair::new(
            "Chronic magnesium deficiency disrupts neuromuscular signal transmission".into(),
            "Persistent muscle cramps, arrhythmias, and tremors develop over time".into(),
            TrainingDirection::Forward,
            0.85,
        )
        .with_mechanism("biological")
        .with_domain("nutrition")
        .with_hard_negative("Magnesium is involved in over 300 enzymatic reactions in the human body"),
        CausalTrainingPair::new(
            "Omega-3 fatty acid deficiency reduces anti-inflammatory eicosanoid production".into(),
            "Chronic low-grade systemic inflammation persists and accelerates vascular damage".into(),
            TrainingDirection::Forward,
            0.82,
        )
        .with_mechanism("biological")
        .with_domain("nutrition")
        .with_hard_negative("Fatty fish like salmon and mackerel are rich dietary sources of omega-3s"),

        // ================================================================
        // NEW PAIRS: Additional Cybersecurity pairs
        // ================================================================
        CausalTrainingPair::new(
            "DNS cache poisoning injects forged records into a resolver's cache".into(),
            "Users are redirected to attacker-controlled servers without any browser warning".into(),
            TrainingDirection::Forward,
            0.91,
        )
        .with_mechanism("technical")
        .with_domain("cybersecurity")
        .with_hard_negative("DNSSEC adds cryptographic signatures to DNS records for verification"),
        CausalTrainingPair::new(
            "Misconfigured cloud storage buckets expose sensitive data publicly".into(),
            "Attackers discover and exfiltrate confidential records including personal identifiable information".into(),
            TrainingDirection::Forward,
            0.90,
        )
        .with_mechanism("operational")
        .with_domain("cybersecurity")
        .with_hard_negative("Cloud shared responsibility models divide security duties between provider and customer"),
        CausalTrainingPair::new(
            "Session tokens transmitted over unencrypted HTTP connections".into(),
            "Man-in-the-middle attackers intercept tokens and hijack authenticated sessions".into(),
            TrainingDirection::Forward,
            0.92,
        )
        .with_mechanism("technical")
        .with_domain("cybersecurity")
        .with_hard_negative("TLS certificates authenticate servers and encrypt data in transit"),
        CausalTrainingPair::new(
            "Zero-day kernel vulnerability disclosed before vendor patch availability".into(),
            "Threat actors weaponize the exploit for privilege escalation across affected systems".into(),
            TrainingDirection::Forward,
            0.93,
        )
        .with_mechanism("technical")
        .with_domain("cybersecurity")
        .with_hard_negative("Bug bounty programs incentivize responsible disclosure of vulnerabilities"),
        CausalTrainingPair::new(
            "Insufficient log monitoring fails to detect lateral movement within the network".into(),
            "Attackers dwell undetected for months and exfiltrate data incrementally".into(),
            TrainingDirection::Forward,
            0.86,
        )
        .with_mechanism("operational")
        .with_domain("cybersecurity")
        .with_hard_negative("SIEM platforms aggregate and correlate security events across an organization"),

        // ================================================================
        // NEW PAIRS: Additional Psychology pairs
        // ================================================================
        CausalTrainingPair::new(
            "Chronic workplace bullying activates sustained threat responses in the victim".into(),
            "Prolonged stress exposure precipitates anxiety disorders and occupational burnout".into(),
            TrainingDirection::Forward,
            0.85,
        )
        .with_mechanism("stress")
        .with_domain("psychology")
        .with_hard_negative("Workplace harassment policies are mandated in many jurisdictions"),
        CausalTrainingPair::new(
            "Dopaminergic reward prediction error from variable-ratio reinforcement schedules".into(),
            "Behavioral addiction patterns emerge with compulsive repetition despite negative consequences".into(),
            TrainingDirection::Forward,
            0.87,
        )
        .with_mechanism("neuropsychological")
        .with_domain("psychology")
        .with_hard_negative("B.F. Skinner's operant conditioning research identified four reinforcement schedules"),
        CausalTrainingPair::new(
            "Chronic parental neglect deprives children of consistent emotional attunement".into(),
            "Emotional regulation deficits and interpersonal difficulties persist into adulthood".into(),
            TrainingDirection::Forward,
            0.86,
        )
        .with_mechanism("developmental")
        .with_domain("psychology")
        .with_hard_negative("The Adverse Childhood Experiences study linked childhood adversity to adult health"),
        CausalTrainingPair::new(
            "Confirmation bias selectively filters information to match existing beliefs".into(),
            "Belief polarization intensifies as individuals reject disconfirming evidence".into(),
            TrainingDirection::Forward,
            0.83,
        )
        .with_mechanism("cognitive")
        .with_domain("psychology")
        .with_hard_negative("Cognitive biases are systematic deviations from rational judgment"),
        CausalTrainingPair::new(
            "Prenatal maternal stress elevates fetal cortisol exposure through placental transfer".into(),
            "Offspring exhibit heightened stress reactivity and increased anxiety risk in childhood".into(),
            TrainingDirection::Forward,
            0.81,
        )
        .with_mechanism("developmental")
        .with_domain("psychology")
        .with_hard_negative("The fetal programming hypothesis links prenatal conditions to later health outcomes"),

        // ================================================================
        // NEW PAIRS: Additional History pairs
        // ================================================================
        CausalTrainingPair::new(
            "The cotton gin dramatically increased processing efficiency of raw cotton".into(),
            "Demand for enslaved labor on cotton plantations expanded across the American South".into(),
            TrainingDirection::Forward,
            0.87,
        )
        .with_mechanism("socioeconomic")
        .with_domain("history")
        .with_hard_negative("Eli Whitney patented the cotton gin in 1794"),
        CausalTrainingPair::new(
            "The partition of the Indian subcontinent drew borders through mixed religious communities".into(),
            "Communal violence erupted and mass displacement affected millions during partition".into(),
            TrainingDirection::Forward,
            0.89,
        )
        .with_mechanism("political")
        .with_domain("history")
        .with_hard_negative("British India was partitioned into India and Pakistan in August 1947"),
        CausalTrainingPair::new(
            "Discovery of large gold deposits in California in 1848".into(),
            "A massive westward migration reshaped demographics and accelerated US territorial expansion".into(),
            TrainingDirection::Forward,
            0.86,
        )
        .with_mechanism("economic")
        .with_domain("history")
        .with_hard_negative("California became the 31st US state in 1850"),
        CausalTrainingPair::new(
            "The Chernobyl reactor explosion released radioactive fallout across Europe".into(),
            "Public opposition to nuclear energy surged and several countries abandoned nuclear programs".into(),
            TrainingDirection::Forward,
            0.88,
        )
        .with_mechanism("political")
        .with_domain("history")
        .with_hard_negative("The Chernobyl disaster occurred on April 26, 1986"),
        CausalTrainingPair::new(
            "The transatlantic slave trade forcibly relocated millions of Africans to the Americas".into(),
            "Demographic collapse in West Africa and entrenched racial hierarchies in the New World persisted for centuries".into(),
            TrainingDirection::Forward,
            0.90,
        )
        .with_mechanism("socioeconomic")
        .with_domain("history")
        .with_hard_negative("The Middle Passage refers to the forced voyage of enslaved Africans across the Atlantic"),

        // ================================================================
        // NEW PAIRS: Implicit causal (no explicit markers) — 15+ pairs
        // ================================================================
        CausalTrainingPair::new(
            "The city's water treatment plant switched from chloramine to chlorine disinfection".into(),
            "Pediatric blood lead levels in the community rose significantly over the next 18 months".into(),
            TrainingDirection::Forward,
            0.80,
        )
        .with_mechanism("chemical")
        .with_domain("health")
        .with_hard_negative("Blood lead levels are measured in micrograms per deciliter"),
        CausalTrainingPair::new(
            "Annual rainfall in the Sahel region declined steadily from the 1960s through the 1980s".into(),
            "Pastoral communities abandoned traditional grazing lands and migrated southward".into(),
            TrainingDirection::Forward,
            0.78,
        )
        .with_mechanism("climate")
        .with_domain("environment")
        .with_hard_negative("The Sahel is a semi-arid region stretching across Africa south of the Sahara"),
        CausalTrainingPair::new(
            "The central bank held interest rates near zero for over a decade".into(),
            "Residential property valuations in major cities doubled during the same period".into(),
            TrainingDirection::Forward,
            0.77,
        )
        .with_mechanism("monetary")
        .with_domain("economics")
        .with_hard_negative("Real estate markets are influenced by location, demographics, and supply"),
        CausalTrainingPair::new(
            "A popular social media platform removed its chronological feed in favor of algorithmic ranking".into(),
            "Average daily screen time among teenagers increased by 40 minutes within one year".into(),
            TrainingDirection::Forward,
            0.75,
        )
        .with_mechanism("behavioral")
        .with_domain("technology")
        .with_hard_negative("Social media usage statistics vary across different age demographics"),
        CausalTrainingPair::new(
            "Several high-profile police shootings were captured on video and widely shared online".into(),
            "Nationwide protests and a broad social movement for policing reform emerged".into(),
            TrainingDirection::Forward,
            0.79,
        )
        .with_mechanism("social")
        .with_domain("social")
        .with_hard_negative("Body-worn cameras are used by law enforcement agencies across the country"),
        CausalTrainingPair::new(
            "Atmospheric pressure dropped rapidly ahead of the approaching tropical cyclone".into(),
            "Ocean surface levels rose as the storm surge inundated low-lying coastal areas".into(),
            TrainingDirection::Forward,
            0.82,
        )
        .with_mechanism("physical")
        .with_domain("physics")
        .with_hard_negative("Tropical cyclones are classified by sustained wind speed on the Saffir-Simpson scale"),
        CausalTrainingPair::new(
            "The region's traditional diet shifted heavily toward processed and fast foods".into(),
            "Rates of childhood obesity and type 2 diabetes tripled within a generation".into(),
            TrainingDirection::Forward,
            0.80,
        )
        .with_mechanism("metabolic")
        .with_domain("nutrition")
        .with_hard_negative("The WHO recommends limiting daily added sugar intake to less than 10% of total energy"),
        CausalTrainingPair::new(
            "A former employee retained valid VPN credentials months after termination".into(),
            "Sensitive engineering documents appeared on a competitor's product shortly thereafter".into(),
            TrainingDirection::Forward,
            0.81,
        )
        .with_mechanism("operational")
        .with_domain("cybersecurity")
        .with_hard_negative("Identity and access management systems centralize user provisioning and deprovisioning"),
        CausalTrainingPair::new(
            "Children who experienced prolonged school closures during the pandemic".into(),
            "Standardized test scores for the affected cohort dropped measurably compared to prior years".into(),
            TrainingDirection::Forward,
            0.79,
        )
        .with_mechanism("developmental")
        .with_domain("psychology")
        .with_hard_negative("Educational attainment is a key predictor of socioeconomic status"),
        CausalTrainingPair::new(
            "The collapse of the Soviet Union removed the primary ideological counterweight to Western capitalism".into(),
            "Former Eastern Bloc nations rapidly privatized state industries and joined Western economic institutions".into(),
            TrainingDirection::Forward,
            0.84,
        )
        .with_mechanism("political")
        .with_domain("history")
        .with_hard_negative("The Soviet Union officially dissolved on December 26, 1991"),

        // ================================================================
        // NEW PAIRS: Bidirectional / feedback loops (5+ pairs)
        // ================================================================
        CausalTrainingPair::new(
            "Poverty limits access to education and skill development".into(),
            "Low educational attainment restricts employment options and perpetuates poverty".into(),
            TrainingDirection::Bidirectional,
            0.88,
        )
        .with_mechanism("socioeconomic_feedback")
        .with_domain("social")
        .with_hard_negative("Education spending as a percentage of GDP varies widely among nations"),
        CausalTrainingPair::new(
            "Rising global temperatures melt Arctic sea ice and reduce albedo".into(),
            "Lower albedo increases solar heat absorption, further accelerating warming".into(),
            TrainingDirection::Bidirectional,
            0.90,
        )
        .with_mechanism("climate_feedback")
        .with_domain("environment")
        .with_hard_negative("Albedo is the fraction of solar radiation reflected by a surface"),
        CausalTrainingPair::new(
            "Market panic drives investors to sell assets rapidly".into(),
            "Falling asset prices deepen panic and trigger further selling".into(),
            TrainingDirection::Bidirectional,
            0.85,
        )
        .with_mechanism("economic_feedback")
        .with_domain("economics")
        .with_hard_negative("Circuit breakers temporarily halt trading during extreme price declines"),
        CausalTrainingPair::new(
            "Antibiotic-resistant infections require stronger, broader-spectrum antibiotics".into(),
            "Broader antibiotic use applies wider selection pressure, breeding more resistance".into(),
            TrainingDirection::Bidirectional,
            0.87,
        )
        .with_mechanism("evolutionary_feedback")
        .with_domain("health")
        .with_hard_negative("Antibiotic stewardship programs aim to optimize antimicrobial prescribing"),
        CausalTrainingPair::new(
            "Soil erosion reduces agricultural productivity on marginal lands".into(),
            "Farmers clear additional forest to compensate, exposing more soil to erosion".into(),
            TrainingDirection::Bidirectional,
            0.84,
        )
        .with_mechanism("ecological_feedback")
        .with_domain("environment")
        .with_hard_negative("Conservation tillage practices reduce soil disturbance during planting"),
        CausalTrainingPair::new(
            "Urban traffic congestion increases commute times and fuel consumption".into(),
            "Frustrated commuters relocate to suburbs, increasing sprawl and congestion on arterial roads".into(),
            TrainingDirection::Bidirectional,
            0.82,
        )
        .with_mechanism("urban_feedback")
        .with_domain("social")
        .with_hard_negative("Traffic congestion indices measure delays relative to free-flow travel times"),
        CausalTrainingPair::new(
            "Sleep deprivation increases stress hormone levels and emotional reactivity".into(),
            "Heightened stress and anxiety make it more difficult to fall asleep the following night".into(),
            TrainingDirection::Bidirectional,
            0.86,
        )
        .with_mechanism("neuropsychological_feedback")
        .with_domain("psychology")
        .with_hard_negative("Polysomnography records brain waves, oxygen levels, and body movements during sleep"),

        // ================================================================
        // NEW PAIRS: Non-causal (TrainingDirection::None) — 36+ pairs
        // ================================================================
        CausalTrainingPair::new(
            "The Amazon River is the largest river by discharge volume".into(),
            "Brazil is the fifth largest country in the world by area".into(),
            TrainingDirection::None,
            0.08,
        )
        .with_mechanism("observational")
        .with_domain("environment")
        .with_hard_negative("The Amazon Basin contains the world's largest tropical rainforest"),
        CausalTrainingPair::new(
            "Shakespeare wrote 37 plays during his career".into(),
            "London's Globe Theatre was originally built in 1599".into(),
            TrainingDirection::None,
            0.06,
        )
        .with_mechanism("observational")
        .with_domain("history")
        .with_hard_negative("Elizabethan theatre featured open-air performances"),
        CausalTrainingPair::new(
            "The human genome contains approximately 20,000 protein-coding genes".into(),
            "CRISPR-Cas9 is a gene editing tool derived from bacterial immune systems".into(),
            TrainingDirection::None,
            0.10,
        )
        .with_mechanism("observational")
        .with_domain("health")
        .with_hard_negative("The Human Genome Project was completed in 2003"),
        CausalTrainingPair::new(
            "Rust is a systems programming language focused on memory safety".into(),
            "Linux kernel version 6.1 introduced initial Rust support".into(),
            TrainingDirection::None,
            0.09,
        )
        .with_mechanism("observational")
        .with_domain("technology")
        .with_hard_negative("Memory safety bugs are a common source of security vulnerabilities"),
        CausalTrainingPair::new(
            "The speed of light in vacuum is approximately 299,792,458 meters per second".into(),
            "Radio telescopes detect electromagnetic radiation from distant celestial objects".into(),
            TrainingDirection::None,
            0.07,
        )
        .with_mechanism("observational")
        .with_domain("physics")
        .with_hard_negative("Electromagnetic radiation spans the spectrum from radio waves to gamma rays"),
        CausalTrainingPair::new(
            "Avocados are a source of monounsaturated fatty acids".into(),
            "Mediterranean diets emphasize olive oil, fish, and whole grains".into(),
            TrainingDirection::None,
            0.11,
        )
        .with_mechanism("observational")
        .with_domain("nutrition")
        .with_hard_negative("Dietary guidelines are published by national health organizations"),
        CausalTrainingPair::new(
            "AES-256 encryption uses a 256-bit symmetric key".into(),
            "Quantum computers use qubits that can exist in superposition states".into(),
            TrainingDirection::None,
            0.08,
        )
        .with_mechanism("observational")
        .with_domain("cybersecurity")
        .with_hard_negative("Post-quantum cryptography aims to resist quantum computing attacks"),
        CausalTrainingPair::new(
            "Maslow's hierarchy of needs arranges human motivations in five levels".into(),
            "The Stanford marshmallow experiment studied delayed gratification in children".into(),
            TrainingDirection::None,
            0.10,
        )
        .with_mechanism("observational")
        .with_domain("psychology")
        .with_hard_negative("Personality psychology studies individual differences in behavior and cognition"),
        CausalTrainingPair::new(
            "The GDP of the United States exceeds 25 trillion dollars".into(),
            "Switzerland consistently ranks among the highest in GDP per capita".into(),
            TrainingDirection::None,
            0.07,
        )
        .with_mechanism("observational")
        .with_domain("economics")
        .with_hard_negative("GDP can be measured using the expenditure, income, or production approach"),
        CausalTrainingPair::new(
            "Public libraries provide free access to books and digital resources".into(),
            "Adult literacy rates vary significantly between developed and developing nations".into(),
            TrainingDirection::None,
            0.12,
        )
        .with_mechanism("observational")
        .with_domain("social")
        .with_hard_negative("UNESCO promotes literacy as a fundamental human right"),
        CausalTrainingPair::new(
            "The US Constitution establishes three branches of government".into(),
            "Jury trials are guaranteed by the Sixth and Seventh Amendments".into(),
            TrainingDirection::None,
            0.09,
        )
        .with_mechanism("observational")
        .with_domain("legal")
        .with_hard_negative("Constitutional law governs the structure and powers of government"),
        CausalTrainingPair::new(
            "Concrete has a compressive strength much greater than its tensile strength".into(),
            "Steel reinforcement is placed in tension zones of structural members".into(),
            TrainingDirection::None,
            0.13,
        )
        .with_mechanism("observational")
        .with_domain("engineering")
        .with_hard_negative("Reinforced concrete combines the strengths of both concrete and steel"),
        CausalTrainingPair::new(
            "Mars has two small moons named Phobos and Deimos".into(),
            "The Perseverance rover landed on Mars in February 2021".into(),
            TrainingDirection::None,
            0.05,
        )
        .with_mechanism("observational")
        .with_domain("physics")
        .with_hard_negative("Mars is the fourth planet from the Sun"),
        CausalTrainingPair::new(
            "The Krebs cycle occurs in the mitochondrial matrix".into(),
            "ATP synthase is located in the inner mitochondrial membrane".into(),
            TrainingDirection::None,
            0.14,
        )
        .with_mechanism("observational")
        .with_domain("health")
        .with_hard_negative("Mitochondria are often called the powerhouses of the cell"),
        CausalTrainingPair::new(
            "HTTP status code 404 indicates a resource was not found".into(),
            "REST APIs use standard HTTP methods including GET, POST, PUT, and DELETE".into(),
            TrainingDirection::None,
            0.06,
        )
        .with_mechanism("observational")
        .with_domain("technology")
        .with_hard_negative("The HTTP protocol was designed by Tim Berners-Lee at CERN"),
        CausalTrainingPair::new(
            "The Nile River flows through eleven countries in northeastern Africa".into(),
            "Egypt's population is concentrated along the Nile Delta".into(),
            TrainingDirection::None,
            0.13,
        )
        .with_mechanism("observational")
        .with_domain("environment")
        .with_hard_negative("The Nile is traditionally considered the longest river in the world"),
        CausalTrainingPair::new(
            "Contract law requires offer, acceptance, and consideration".into(),
            "Arbitration clauses direct disputes to alternative resolution forums".into(),
            TrainingDirection::None,
            0.10,
        )
        .with_mechanism("observational")
        .with_domain("legal")
        .with_hard_negative("The Uniform Commercial Code governs commercial transactions in the US"),
        CausalTrainingPair::new(
            "Stainless steel contains at least 10.5% chromium by mass".into(),
            "Titanium alloys are used in aerospace applications for their strength-to-weight ratio".into(),
            TrainingDirection::None,
            0.07,
        )
        .with_mechanism("observational")
        .with_domain("engineering")
        .with_hard_negative("Material science studies the properties and applications of metals and alloys"),
        CausalTrainingPair::new(
            "The Magna Carta was signed in 1215".into(),
            "The French Revolution began in 1789 with the storming of the Bastille".into(),
            TrainingDirection::None,
            0.05,
        )
        .with_mechanism("observational")
        .with_domain("history")
        .with_hard_negative("European history spans from ancient civilizations to the modern era"),
        CausalTrainingPair::new(
            "Potassium is an essential electrolyte for cellular function".into(),
            "Bananas are a popular fruit consumed worldwide".into(),
            TrainingDirection::None,
            0.11,
        )
        .with_mechanism("observational")
        .with_domain("nutrition")
        .with_hard_negative("Electrolytes maintain fluid balance and nerve signal transmission"),
        CausalTrainingPair::new(
            "REM sleep cycles last approximately 90 minutes in adults".into(),
            "Melatonin supplements are widely available over the counter".into(),
            TrainingDirection::None,
            0.08,
        )
        .with_mechanism("observational")
        .with_domain("psychology")
        .with_hard_negative("Sleep architecture includes both REM and non-REM stages"),
        CausalTrainingPair::new(
            "Bitcoin uses a proof-of-work consensus mechanism".into(),
            "Ethereum transitioned to proof-of-stake in September 2022".into(),
            TrainingDirection::None,
            0.06,
        )
        .with_mechanism("observational")
        .with_domain("technology")
        .with_hard_negative("Blockchain technology provides a distributed ledger for recording transactions"),
        CausalTrainingPair::new(
            "The World Trade Organization has 164 member states".into(),
            "Bilateral trade agreements are negotiated between two countries".into(),
            TrainingDirection::None,
            0.09,
        )
        .with_mechanism("observational")
        .with_domain("economics")
        .with_hard_negative("International trade theory includes comparative and absolute advantage"),
        CausalTrainingPair::new(
            "The International Space Station orbits Earth at approximately 408 kilometers altitude".into(),
            "Astronauts experience bone density loss during extended stays in microgravity".into(),
            TrainingDirection::None,
            0.14,
        )
        .with_mechanism("observational")
        .with_domain("physics")
        .with_hard_negative("The ISS has been continuously occupied since November 2000"),
        CausalTrainingPair::new(
            "The tort of defamation requires a false statement of fact published to a third party".into(),
            "Appellate courts review lower court decisions for errors of law".into(),
            TrainingDirection::None,
            0.07,
        )
        .with_mechanism("observational")
        .with_domain("legal")
        .with_hard_negative("The US legal system is based on common law inherited from England"),
        CausalTrainingPair::new(
            "OSHA sets workplace safety standards for employers in the United States".into(),
            "The Americans with Disabilities Act prohibits discrimination based on disability".into(),
            TrainingDirection::None,
            0.08,
        )
        .with_mechanism("observational")
        .with_domain("legal")
        .with_hard_negative("Federal regulatory agencies enforce compliance with statutory requirements"),
        CausalTrainingPair::new(
            "Finite element analysis divides structures into discrete elements for computation".into(),
            "Computational fluid dynamics models fluid flow using the Navier-Stokes equations".into(),
            TrainingDirection::None,
            0.09,
        )
        .with_mechanism("observational")
        .with_domain("engineering")
        .with_hard_negative("Numerical simulation tools are widely used in modern engineering design"),
        CausalTrainingPair::new(
            "Carbon fiber composites have a high tensile strength-to-weight ratio".into(),
            "3D printing enables rapid prototyping of complex geometries".into(),
            TrainingDirection::None,
            0.06,
        )
        .with_mechanism("observational")
        .with_domain("engineering")
        .with_hard_negative("Advanced manufacturing techniques continue to evolve in the aerospace industry"),
        CausalTrainingPair::new(
            "Coral bleaching occurs when symbiotic algae are expelled from coral tissue".into(),
            "Mangrove forests serve as nursery habitats for many marine fish species".into(),
            TrainingDirection::None,
            0.10,
        )
        .with_mechanism("observational")
        .with_domain("environment")
        .with_hard_negative("Marine ecosystems support a wide variety of plant and animal life"),
        CausalTrainingPair::new(
            "Social identity theory explains in-group favoritism and out-group discrimination".into(),
            "The bystander effect describes reduced helping behavior in the presence of others".into(),
            TrainingDirection::None,
            0.08,
        )
        .with_mechanism("observational")
        .with_domain("psychology")
        .with_hard_negative("Social psychology examines how people's thoughts and behaviors are influenced by others"),
        CausalTrainingPair::new(
            "Feudalism organized medieval European society around land ownership and service".into(),
            "The Silk Road connected East Asian and European trade networks".into(),
            TrainingDirection::None,
            0.07,
        )
        .with_mechanism("observational")
        .with_domain("history")
        .with_hard_negative("Medieval European civilization was shaped by Christianity, feudalism, and trade"),
        CausalTrainingPair::new(
            "IP address allocation is managed by regional internet registries".into(),
            "Firewalls filter network traffic based on predefined security rules".into(),
            TrainingDirection::None,
            0.09,
        )
        .with_mechanism("observational")
        .with_domain("cybersecurity")
        .with_hard_negative("Network security involves multiple layers of defense at the edge and within the network"),
        CausalTrainingPair::new(
            "Insulin is produced by beta cells in the pancreatic islets of Langerhans".into(),
            "Glucagon is secreted by alpha cells in the same pancreatic islets".into(),
            TrainingDirection::None,
            0.12,
        )
        .with_mechanism("observational")
        .with_domain("health")
        .with_hard_negative("The pancreas serves both endocrine and exocrine functions"),
        CausalTrainingPair::new(
            "Median household income varies substantially across US states".into(),
            "Property tax revenue funds the majority of public school budgets".into(),
            TrainingDirection::None,
            0.13,
        )
        .with_mechanism("observational")
        .with_domain("social")
        .with_hard_negative("Public education funding mechanisms differ between countries"),
        CausalTrainingPair::new(
            "Zinc is an essential trace mineral for immune system function".into(),
            "Probiotics are live microorganisms found in fermented foods".into(),
            TrainingDirection::None,
            0.07,
        )
        .with_mechanism("observational")
        .with_domain("nutrition")
        .with_hard_negative("Dietary supplements are regulated differently from pharmaceutical drugs"),
        CausalTrainingPair::new(
            "Supply-side economics emphasizes tax cuts to stimulate production".into(),
            "Keynesian economics advocates government spending during recessions".into(),
            TrainingDirection::None,
            0.10,
        )
        .with_mechanism("observational")
        .with_domain("economics")
        .with_hard_negative("Macroeconomic theory encompasses multiple competing schools of thought"),

        // ================================================================
        // ADDITIONAL PAIRS: Reaching 250+ total
        // ================================================================

        // --- More Legal Forward pairs ---
        CausalTrainingPair::new(
            "Class action waivers in consumer contracts bar collective litigation".into(),
            "Individual consumers lack economic incentive to pursue small claims, leaving corporate misconduct unchecked".into(),
            TrainingDirection::Forward,
            0.82,
        )
        .with_mechanism("procedural")
        .with_domain("legal")
        .with_hard_negative("The Federal Arbitration Act governs arbitration agreements in the United States"),
        CausalTrainingPair::new(
            "Retroactive application of a criminal statute to conduct that was lawful when performed".into(),
            "Courts strike down the statute as an unconstitutional ex post facto law".into(),
            TrainingDirection::Forward,
            0.93,
        )
        .with_mechanism("constitutional")
        .with_domain("legal")
        .with_hard_negative("Article I of the US Constitution contains the Ex Post Facto Clause"),

        // --- More Engineering Forward pairs ---
        CausalTrainingPair::new(
            "Alkali-silica reaction in concrete produces an expansive gel over decades".into(),
            "Internal swelling pressure cracks the concrete matrix and reduces structural service life".into(),
            TrainingDirection::Forward,
            0.86,
        )
        .with_mechanism("chemical")
        .with_domain("engineering")
        .with_hard_negative("Low-alkali cement and supplementary cementitious materials mitigate ASR"),
        CausalTrainingPair::new(
            "Creep deformation in prestressed concrete beams under sustained loading".into(),
            "Prestress force diminishes over time and long-term deflections exceed design predictions".into(),
            TrainingDirection::Forward,
            0.84,
        )
        .with_mechanism("material")
        .with_domain("engineering")
        .with_hard_negative("Prestressed concrete uses high-strength steel tendons to introduce compression"),

        // --- More Health Forward pairs ---
        CausalTrainingPair::new(
            "Chronic noise exposure above 85 dB damages cochlear hair cells".into(),
            "Irreversible sensorineural hearing loss develops progressively".into(),
            TrainingDirection::Forward,
            0.90,
        )
        .with_mechanism("biological")
        .with_domain("health")
        .with_hard_negative("OSHA mandates hearing protection in workplaces exceeding 85 dB TWA"),
        CausalTrainingPair::new(
            "Gestational diabetes impairs placental glucose regulation".into(),
            "Fetal macrosomia increases the risk of birth complications and neonatal hypoglycemia".into(),
            TrainingDirection::Forward,
            0.87,
        )
        .with_mechanism("biological")
        .with_domain("health")
        .with_hard_negative("Oral glucose tolerance tests screen for gestational diabetes between 24 and 28 weeks"),
        CausalTrainingPair::new(
            "Chronic renal failure impairs erythropoietin production by the kidneys".into(),
            "Insufficient erythropoietin reduces red blood cell production and causes anemia of chronic disease".into(),
            TrainingDirection::Forward,
            0.89,
        )
        .with_mechanism("biological")
        .with_domain("health")
        .with_hard_negative("Erythropoietin-stimulating agents are used in dialysis patients"),

        // --- More Environment Forward pairs ---
        CausalTrainingPair::new(
            "Mountaintop removal mining strips vegetation and topsoil from Appalachian ridges".into(),
            "Valley fills bury headwater streams and permanently alter watershed hydrology".into(),
            TrainingDirection::Forward,
            0.85,
        )
        .with_mechanism("ecological")
        .with_domain("environment")
        .with_hard_negative("Surface mining accounts for a significant portion of US coal production"),
        CausalTrainingPair::new(
            "Ozone layer depletion from CFC emissions increases ultraviolet radiation at the surface".into(),
            "Elevated UV exposure harms phytoplankton productivity and disrupts marine food webs".into(),
            TrainingDirection::Forward,
            0.88,
        )
        .with_mechanism("atmospheric")
        .with_domain("environment")
        .with_hard_negative("The Montreal Protocol phased out production of ozone-depleting substances"),

        // --- More Technology Forward pairs ---
        CausalTrainingPair::new(
            "Accumulating technical debt through shortcuts and deferred refactoring".into(),
            "Development velocity drops as the codebase becomes harder to understand and modify".into(),
            TrainingDirection::Forward,
            0.83,
        )
        .with_mechanism("organizational")
        .with_domain("technology")
        .with_hard_negative("Code review practices help maintain codebase quality over time"),
        CausalTrainingPair::new(
            "GPU memory exhaustion during inference of an oversized neural network model".into(),
            "The application crashes with an out-of-memory error and fails to serve predictions".into(),
            TrainingDirection::Forward,
            0.90,
        )
        .with_mechanism("technical")
        .with_domain("technology")
        .with_hard_negative("Model quantization reduces memory footprint by using lower-precision arithmetic"),

        // --- More Economics Forward pairs ---
        CausalTrainingPair::new(
            "Agricultural subsidies in developed nations lower the price of exported commodities".into(),
            "Farmers in developing countries cannot compete and abandon domestic production".into(),
            TrainingDirection::Forward,
            0.84,
        )
        .with_mechanism("economic")
        .with_domain("economics")
        .with_hard_negative("The World Bank monitors global commodity prices and agricultural trade flows"),
        CausalTrainingPair::new(
            "Student loan debt burdens delay major financial milestones for graduates".into(),
            "Homeownership rates and household formation decline among younger cohorts".into(),
            TrainingDirection::Forward,
            0.81,
        )
        .with_mechanism("economic")
        .with_domain("economics")
        .with_hard_negative("Federal student loan interest rates are set annually by Congress"),

        // --- More Social Forward pairs ---
        CausalTrainingPair::new(
            "Redlining policies historically denied mortgage access in minority neighborhoods".into(),
            "Generational wealth gaps persist along racial lines decades after the practice ended".into(),
            TrainingDirection::Forward,
            0.86,
        )
        .with_mechanism("institutional")
        .with_domain("social")
        .with_hard_negative("The Fair Housing Act of 1968 prohibited discrimination in housing transactions"),
        CausalTrainingPair::new(
            "Widespread smartphone adoption among adolescents transformed social interaction patterns".into(),
            "Rates of in-person socialization among teenagers declined markedly".into(),
            TrainingDirection::Forward,
            0.78,
        )
        .with_mechanism("behavioral")
        .with_domain("social")
        .with_hard_negative("Smartphone ownership among US teenagers exceeds 90%"),

        // --- More Physics Forward pairs ---
        CausalTrainingPair::new(
            "A massive star exhausts its nuclear fuel and can no longer sustain radiation pressure".into(),
            "Gravitational collapse triggers a supernova explosion and disperses heavy elements into space".into(),
            TrainingDirection::Forward,
            0.93,
        )
        .with_mechanism("astrophysical")
        .with_domain("physics")
        .with_hard_negative("Supernovae are classified as Type I or Type II based on spectral characteristics"),
        CausalTrainingPair::new(
            "An electron transitions from a higher to a lower energy level in an atom".into(),
            "A photon is emitted with energy equal to the difference between the two levels".into(),
            TrainingDirection::Forward,
            0.95,
        )
        .with_mechanism("quantum")
        .with_domain("physics")
        .with_hard_negative("Atomic emission spectra are unique to each element"),

        // --- More Nutrition Forward pairs ---
        CausalTrainingPair::new(
            "Chronic folate deficiency during pregnancy impairs neural tube closure in the embryo".into(),
            "Infants are born with spina bifida or anencephaly at elevated rates".into(),
            TrainingDirection::Forward,
            0.92,
        )
        .with_mechanism("developmental")
        .with_domain("nutrition")
        .with_hard_negative("Prenatal vitamins typically contain 400-800 micrograms of folic acid"),
        CausalTrainingPair::new(
            "Excessive sodium intake chronically exceeding renal excretion capacity".into(),
            "Fluid retention and vascular stiffening elevate blood pressure over years".into(),
            TrainingDirection::Forward,
            0.86,
        )
        .with_mechanism("physiological")
        .with_domain("nutrition")
        .with_hard_negative("The American Heart Association recommends no more than 2300 mg sodium per day"),

        // --- More Cybersecurity Forward pairs ---
        CausalTrainingPair::new(
            "Legacy systems running end-of-life operating systems receive no security patches".into(),
            "Known vulnerabilities accumulate and provide reliable entry points for attackers".into(),
            TrainingDirection::Forward,
            0.90,
        )
        .with_mechanism("operational")
        .with_domain("cybersecurity")
        .with_hard_negative("Operating system lifecycle policies define support and patch timelines"),
        CausalTrainingPair::new(
            "Deepfake audio convincingly mimics a CEO's voice in a phone call to the CFO".into(),
            "The CFO authorizes a fraudulent wire transfer believing the instruction is legitimate".into(),
            TrainingDirection::Forward,
            0.88,
        )
        .with_mechanism("social_engineering")
        .with_domain("cybersecurity")
        .with_hard_negative("Voice biometric authentication analyzes vocal characteristics for identity verification"),

        // --- More Psychology Forward pairs ---
        CausalTrainingPair::new(
            "Excessive social comparison on curated social media profiles".into(),
            "Self-esteem erosion and body image dissatisfaction intensify among adolescent users".into(),
            TrainingDirection::Forward,
            0.82,
        )
        .with_mechanism("cognitive")
        .with_domain("psychology")
        .with_hard_negative("Social comparison theory was proposed by Leon Festinger in 1954"),
        CausalTrainingPair::new(
            "Traumatic brain injury to the orbitofrontal cortex disrupts emotional regulation circuits".into(),
            "Patients exhibit impulsive behavior, poor social judgment, and personality changes".into(),
            TrainingDirection::Forward,
            0.88,
        )
        .with_mechanism("neuropsychological")
        .with_domain("psychology")
        .with_hard_negative("The famous case of Phineas Gage illustrated frontal lobe function in personality"),

        // --- More History Forward pairs ---
        CausalTrainingPair::new(
            "The Manhattan Project successfully developed nuclear weapons during World War II".into(),
            "A global nuclear arms race began as rival nations sought strategic deterrence".into(),
            TrainingDirection::Forward,
            0.89,
        )
        .with_mechanism("military")
        .with_domain("history")
        .with_hard_negative("The Treaty on the Non-Proliferation of Nuclear Weapons entered into force in 1970"),
        CausalTrainingPair::new(
            "Gutenberg's printing press made the Bible widely accessible in vernacular languages".into(),
            "Clerical monopoly on scriptural interpretation weakened and reform movements proliferated".into(),
            TrainingDirection::Forward,
            0.85,
        )
        .with_mechanism("cultural")
        .with_domain("history")
        .with_hard_negative("Martin Luther posted his 95 Theses in 1517"),

        // --- More Implicit causal pairs ---
        CausalTrainingPair::new(
            "The factory upstream began operations in the spring of that year".into(),
            "Fish populations in the downstream reach declined sharply by autumn".into(),
            TrainingDirection::Forward,
            0.74,
        )
        .with_mechanism("ecological")
        .with_domain("environment")
        .with_hard_negative("Fish population surveys are conducted using electrofishing and seine netting"),
        CausalTrainingPair::new(
            "After the minimum wage increase took effect in January".into(),
            "Several small restaurants in the area closed or reduced staff hours by mid-year".into(),
            TrainingDirection::Forward,
            0.72,
        )
        .with_mechanism("economic")
        .with_domain("economics")
        .with_hard_negative("Minimum wage laws set the lowest hourly rate employers may legally pay workers"),
        CausalTrainingPair::new(
            "The pharmaceutical company obtained exclusive marketing rights for the only approved treatment".into(),
            "Patient out-of-pocket costs for the medication increased tenfold over five years".into(),
            TrainingDirection::Forward,
            0.79,
        )
        .with_mechanism("economic")
        .with_domain("legal")
        .with_hard_negative("The Hatch-Waxman Act governs generic drug approval in the United States"),
        CausalTrainingPair::new(
            "A critical weld inspection was skipped during the construction phase to meet the deadline".into(),
            "The pressure vessel failed at a fraction of its rated capacity during commissioning".into(),
            TrainingDirection::Forward,
            0.81,
        )
        .with_mechanism("quality")
        .with_domain("engineering")
        .with_hard_negative("Non-destructive testing methods include ultrasonic, radiographic, and magnetic particle inspection"),
        CausalTrainingPair::new(
            "The hospital reduced nursing staff ratios to cut operating costs".into(),
            "Patient falls and medication errors increased over the following quarter".into(),
            TrainingDirection::Forward,
            0.80,
        )
        .with_mechanism("organizational")
        .with_domain("health")
        .with_hard_negative("Nurse-to-patient ratios are mandated by law in some states"),

        // --- Bidirectional feedback ---
        CausalTrainingPair::new(
            "Chronic pain reduces physical activity and social engagement".into(),
            "Physical deconditioning and isolation worsen pain perception through central sensitization".into(),
            TrainingDirection::Bidirectional,
            0.85,
        )
        .with_mechanism("neurobiological_feedback")
        .with_domain("health")
        .with_hard_negative("Pain management approaches include pharmacological and cognitive-behavioral strategies"),

        // --- More Non-causal pairs ---
        CausalTrainingPair::new(
            "The periodic table organizes elements by atomic number and electron configuration".into(),
            "Noble gases have complete outer electron shells and are chemically inert".into(),
            TrainingDirection::None,
            0.12,
        )
        .with_mechanism("observational")
        .with_domain("physics")
        .with_hard_negative("Dmitri Mendeleev published the first widely recognized periodic table in 1869"),
        CausalTrainingPair::new(
            "Professional engineers must pass the Fundamentals of Engineering exam".into(),
            "Building codes specify minimum requirements for structural loads and fire resistance".into(),
            TrainingDirection::None,
            0.08,
        )
        .with_mechanism("observational")
        .with_domain("engineering")
        .with_hard_negative("Engineering licensure requirements vary by state and discipline"),
        CausalTrainingPair::new(
            "The Supreme Court consists of nine justices appointed for life".into(),
            "Federal judges are nominated by the President and confirmed by the Senate".into(),
            TrainingDirection::None,
            0.07,
        )
        .with_mechanism("observational")
        .with_domain("legal")
        .with_hard_negative("The US federal court system has three tiers: district, circuit, and Supreme Court"),
        CausalTrainingPair::new(
            "Photovoltaic cells convert sunlight directly into electrical energy".into(),
            "Wind turbines generate electricity from the kinetic energy of moving air".into(),
            TrainingDirection::None,
            0.06,
        )
        .with_mechanism("observational")
        .with_domain("engineering")
        .with_hard_negative("Renewable energy sources include solar, wind, hydroelectric, and geothermal"),
        CausalTrainingPair::new(
            "The WHO recommends exclusive breastfeeding for the first six months of life".into(),
            "Vitamin K is administered to newborns shortly after birth".into(),
            TrainingDirection::None,
            0.09,
        )
        .with_mechanism("observational")
        .with_domain("nutrition")
        .with_hard_negative("Neonatal care protocols differ between countries and healthcare systems"),
        CausalTrainingPair::new(
            "Common law systems rely on judicial precedent and case law".into(),
            "Civil law systems are based on comprehensive statutory codes".into(),
            TrainingDirection::None,
            0.08,
        )
        .with_mechanism("observational")
        .with_domain("legal")
        .with_hard_negative("Legal systems around the world fall into several major traditions"),
        CausalTrainingPair::new(
            "Hadoop processes large datasets across clusters of commodity hardware".into(),
            "Kubernetes orchestrates containerized applications across cloud infrastructure".into(),
            TrainingDirection::None,
            0.07,
        )
        .with_mechanism("observational")
        .with_domain("technology")
        .with_hard_negative("Distributed computing platforms enable processing of big data workloads"),
        CausalTrainingPair::new(
            "The Renaissance began in Italy in the 14th century".into(),
            "The Enlightenment emphasized reason and individual rights in the 18th century".into(),
            TrainingDirection::None,
            0.09,
        )
        .with_mechanism("observational")
        .with_domain("history")
        .with_hard_negative("European intellectual movements shaped modern Western civilization"),

        // --- Final batch to ensure 250+ total ---
        CausalTrainingPair::new(
            "Prolonged drought conditions desiccate vegetation and lower fuel moisture content".into(),
            "Wildfire ignition probability and burn severity increase dramatically".into(),
            TrainingDirection::Forward,
            0.90,
        )
        .with_mechanism("ecological")
        .with_domain("environment")
        .with_hard_negative("The National Interagency Fire Center coordinates wildfire response in the US"),
        CausalTrainingPair::new(
            "Microplastic ingestion by filter-feeding organisms at the base of the food chain".into(),
            "Bioaccumulation concentrates plastic-derived toxins in apex predators".into(),
            TrainingDirection::Forward,
            0.84,
        )
        .with_mechanism("ecological")
        .with_domain("environment")
        .with_hard_negative("Microplastics are defined as plastic particles smaller than 5 millimeters"),
        CausalTrainingPair::new(
            "Improper grounding of electrical systems in industrial facilities".into(),
            "Ground fault currents find unintended paths, creating electrocution hazards for workers".into(),
            TrainingDirection::Forward,
            0.91,
        )
        .with_mechanism("electrical")
        .with_domain("engineering")
        .with_hard_negative("The National Electrical Code specifies grounding requirements for electrical installations"),
        CausalTrainingPair::new(
            "Vibration-induced loosening of bolted connections in rotating machinery".into(),
            "Progressive joint relaxation allows misalignment that damages bearings and seals".into(),
            TrainingDirection::Forward,
            0.85,
        )
        .with_mechanism("mechanical")
        .with_domain("engineering")
        .with_hard_negative("Torque specifications ensure bolted joints maintain adequate clamping force"),
        CausalTrainingPair::new(
            "Prosecutorial overcharging pressures defendants into accepting plea bargains".into(),
            "Innocent defendants sometimes plead guilty to avoid the risk of harsher trial sentences".into(),
            TrainingDirection::Forward,
            0.82,
        )
        .with_mechanism("procedural")
        .with_domain("legal")
        .with_hard_negative("Approximately 97% of federal criminal cases are resolved through plea bargains"),
        CausalTrainingPair::new(
            "Standardized testing emphasis in school curricula narrows instructional focus".into(),
            "Creative thinking and problem-solving skills receive less classroom attention".into(),
            TrainingDirection::Forward,
            0.79,
        )
        .with_mechanism("institutional")
        .with_domain("social")
        .with_hard_negative("Educational assessment methods include formative, summative, and diagnostic approaches"),
        CausalTrainingPair::new(
            "Rapid prototyping with 3D printing accelerates design iteration cycles".into(),
            "Time to market for new products decreases and design flaws are caught earlier".into(),
            TrainingDirection::Forward,
            0.82,
        )
        .with_mechanism("process")
        .with_domain("engineering")
        .with_hard_negative("Additive manufacturing technologies include FDM, SLA, and SLS processes"),
        CausalTrainingPair::new(
            "Dopamine receptor downregulation from chronic stimulant use".into(),
            "Baseline reward sensitivity diminishes, producing anhedonia and drug tolerance".into(),
            TrainingDirection::Forward,
            0.88,
        )
        .with_mechanism("neurochemical")
        .with_domain("psychology")
        .with_hard_negative("The mesolimbic pathway is the primary dopaminergic reward circuit in the brain"),
        CausalTrainingPair::new(
            "The Apollo 11 mission successfully landed humans on the Moon".into(),
            "Congress approved funding for the Space Shuttle program in the following decade".into(),
            TrainingDirection::None,
            0.14,
        )
        .with_mechanism("observational")
        .with_domain("history")
        .with_hard_negative("NASA's crewed spaceflight programs have evolved over six decades"),
        CausalTrainingPair::new(
            "Copper is an excellent conductor of electricity".into(),
            "Fiber optic cables transmit data using pulses of light".into(),
            TrainingDirection::None,
            0.06,
        )
        .with_mechanism("observational")
        .with_domain("engineering")
        .with_hard_negative("Telecommunications infrastructure uses both electrical and optical transmission media"),
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
domain-value ::= "\"health\"" | "\"environment\"" | "\"economics\"" | "\"technology\"" | "\"social\"" | "\"physics\"" | "\"nutrition\"" | "\"cybersecurity\"" | "\"psychology\"" | "\"history\"" | "\"legal\"" | "\"engineering\"" | "\"general\""
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
        let json = serde_json::to_string(pair)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
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
        let pair: CausalTrainingPair = serde_json::from_str(&line)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))?;
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
    let causal_pairs: Vec<&CausalTrainingPair> = pairs.iter().filter(|p| p.is_causal()).collect();

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
        by_domain
            .entry(pair.domain.as_str())
            .or_default()
            .push(pair);
    }

    let domains: Vec<&str> = by_domain.keys().copied().collect();

    for (i, &domain_a) in domains.iter().enumerate() {
        for &domain_b in domains.iter().skip(i + 1) {
            let pairs_a = &by_domain[domain_a];
            let pairs_b = &by_domain[domain_b];

            // Take up to 5 cross-domain pairs per domain combination (expanded for WS2)
            for (idx, pair_a) in pairs_a.iter().enumerate().take(5) {
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
        assert!(
            pairs.len() >= 250,
            "Should have at least 250 seed pairs, got {}",
            pairs.len()
        );

        // Check all 12 domain coverage
        let domains: std::collections::HashSet<_> =
            pairs.iter().map(|p| p.domain.as_str()).collect();
        assert!(domains.contains("health"), "Missing health domain");
        assert!(
            domains.contains("environment"),
            "Missing environment domain"
        );
        assert!(domains.contains("economics"), "Missing economics domain");
        assert!(domains.contains("technology"), "Missing technology domain");
        assert!(domains.contains("social"), "Missing social domain");
        assert!(domains.contains("physics"), "Missing physics domain");
        assert!(domains.contains("nutrition"), "Missing nutrition domain");
        assert!(
            domains.contains("cybersecurity"),
            "Missing cybersecurity domain"
        );
        assert!(domains.contains("psychology"), "Missing psychology domain");
        assert!(domains.contains("history"), "Missing history domain");
        assert!(domains.contains("legal"), "Missing legal domain");
        assert!(
            domains.contains("engineering"),
            "Missing engineering domain"
        );

        // Verify each domain has at least 4 pairs
        for domain in &[
            "health",
            "environment",
            "economics",
            "technology",
            "social",
            "physics",
            "nutrition",
            "cybersecurity",
            "psychology",
            "history",
            "legal",
            "engineering",
        ] {
            let count = pairs.iter().filter(|p| p.domain == *domain).count();
            assert!(
                count >= 4,
                "Domain '{}' should have >= 4 pairs, got {}",
                domain,
                count
            );
        }

        // Verify legal and engineering have at least 15 pairs each
        let legal_count = pairs.iter().filter(|p| p.domain == "legal").count();
        assert!(
            legal_count >= 15,
            "Legal domain should have >= 15 pairs, got {}",
            legal_count
        );
        let eng_count = pairs.iter().filter(|p| p.domain == "engineering").count();
        assert!(
            eng_count >= 15,
            "Engineering domain should have >= 15 pairs, got {}",
            eng_count
        );

        // Verify non-causal pairs ratio (~20%)
        let none_count = pairs
            .iter()
            .filter(|p| matches!(p.direction, TrainingDirection::None))
            .count();
        assert!(
            none_count >= 40,
            "Should have >= 40 non-causal pairs, got {}",
            none_count
        );

        // Verify bidirectional pairs
        let bidir_count = pairs
            .iter()
            .filter(|p| matches!(p.direction, TrainingDirection::Bidirectional))
            .count();
        assert!(
            bidir_count >= 5,
            "Should have >= 5 bidirectional pairs, got {}",
            bidir_count
        );
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
        assert_eq!(
            TrainingDirection::from_str("forward"),
            TrainingDirection::Forward
        );
        assert_eq!(
            TrainingDirection::from_str("A_causes_B"),
            TrainingDirection::Forward
        );
        assert_eq!(
            TrainingDirection::from_str("backward"),
            TrainingDirection::Backward
        );
        assert_eq!(
            TrainingDirection::from_str("bidirectional"),
            TrainingDirection::Bidirectional
        );
        assert_eq!(TrainingDirection::from_str("none"), TrainingDirection::None);
        assert_eq!(
            TrainingDirection::from_str("garbage"),
            TrainingDirection::None
        );
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
        assert_eq!(
            non_causal.difficulty(),
            0.0,
            "Non-causal should be difficulty 0"
        );
    }

    #[test]
    fn test_data_loader_batching() {
        let pairs = seed_training_pairs();
        let total = pairs.len();
        let mut loader = CausalDataLoader::new(pairs, 8, 42);
        assert_eq!(loader.num_batches(), total.div_ceil(8));

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
        let pairs = vec![CausalTrainingPair::new(
            "A causes B".into(),
            "B is caused by A".into(),
            TrainingDirection::Forward,
            0.9,
        )
        .with_domain("test")];

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.jsonl");

        save_pairs_jsonl(&pairs, &path).unwrap();
        let loaded = load_pairs_jsonl(&path).unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].cause_text, "A causes B");
        assert_eq!(loaded[0].domain, "test");
    }
}
