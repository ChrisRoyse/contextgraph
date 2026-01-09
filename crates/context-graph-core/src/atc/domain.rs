//! Domain-Specific Threshold Management
//!
//! Per-domain threshold adaptation with transfer learning.
//! Supports Code, Medical, Legal, Creative, Research, and General domains.
//!
//! Transfer learning formula:
//! θ_new = α × θ_similar_domain + (1 - α) × θ_general

use std::collections::HashMap;

/// Supported domains
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Domain {
    Code,
    Medical,
    Legal,
    Creative,
    Research,
    General,
}

impl Domain {
    pub fn as_str(&self) -> &str {
        match self {
            Domain::Code => "code",
            Domain::Medical => "medical",
            Domain::Legal => "legal",
            Domain::Creative => "creative",
            Domain::Research => "research",
            Domain::General => "general",
        }
    }

    /// Get description from constitution
    pub fn description(&self) -> &str {
        match self {
            Domain::Code => "Strict thresholds, low tolerance for false positives",
            Domain::Medical => "Very strict, high causal weight",
            Domain::Legal => "Moderate, high semantic precision",
            Domain::Creative => "Loose thresholds, exploration encouraged",
            Domain::Research => "Balanced, novelty valued",
            Domain::General => "Default priors",
        }
    }

    /// Get recommended strictness (0=loose, 1=strict)
    pub fn strictness(&self) -> f32 {
        match self {
            Domain::Code => 0.9,
            Domain::Medical => 1.0,
            Domain::Legal => 0.8,
            Domain::Creative => 0.2,
            Domain::Research => 0.5,
            Domain::General => 0.5,
        }
    }

    /// Find most similar domain for transfer learning
    pub fn find_similar(&self) -> Domain {
        match self {
            Domain::Code => Domain::Research,
            Domain::Medical => Domain::Legal,
            Domain::Legal => Domain::Medical,
            Domain::Creative => Domain::Research,
            Domain::Research => Domain::General,
            Domain::General => Domain::General,
        }
    }
}

impl std::str::FromStr for Domain {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "code" => Ok(Domain::Code),
            "medical" => Ok(Domain::Medical),
            "legal" => Ok(Domain::Legal),
            "creative" => Ok(Domain::Creative),
            "research" => Ok(Domain::Research),
            "general" => Ok(Domain::General),
            _ => Err(()),
        }
    }
}

/// Thresholds for a specific domain
#[derive(Debug, Clone)]
pub struct DomainThresholds {
    pub domain: Domain,
    pub theta_opt: f32,
    pub theta_acc: f32,
    pub theta_warn: f32,
    pub theta_dup: f32,
    pub theta_edge: f32,
    pub confidence_bias: f32,  // Domain-specific confidence calibration
}

impl DomainThresholds {
    /// Create default thresholds for a domain
    pub fn new(domain: Domain) -> Self {
        let strictness = domain.strictness();

        // Adjust base thresholds by domain strictness
        // Stricter domains have higher thresholds
        let theta_opt = 0.75 + (strictness * 0.1);  // [0.75, 0.85]
        let theta_acc = 0.70 + (strictness * 0.08); // [0.70, 0.78]
        let theta_warn = 0.55 + (strictness * 0.05); // [0.55, 0.60]

        Self {
            domain,
            theta_opt,
            theta_acc,
            theta_warn,
            theta_dup: 0.90,
            theta_edge: 0.70,
            confidence_bias: 1.0,
        }
    }

    /// Transfer learning: blend with similar domain thresholds
    pub fn blend_with_similar(&mut self, similar_thresholds: &DomainThresholds, alpha: f32) {
        let alpha = alpha.clamp(0.0, 1.0);

        self.theta_opt = alpha * similar_thresholds.theta_opt + (1.0 - alpha) * self.theta_opt;
        self.theta_acc = alpha * similar_thresholds.theta_acc + (1.0 - alpha) * self.theta_acc;
        self.theta_warn = alpha * similar_thresholds.theta_warn + (1.0 - alpha) * self.theta_warn;
        self.theta_dup = alpha * similar_thresholds.theta_dup + (1.0 - alpha) * self.theta_dup;
        self.theta_edge = alpha * similar_thresholds.theta_edge + (1.0 - alpha) * self.theta_edge;
    }

    /// Check if thresholds are valid (monotonicity, ranges)
    pub fn is_valid(&self) -> bool {
        // Monotonicity
        if !(self.theta_opt > self.theta_acc && self.theta_acc > self.theta_warn) {
            return false;
        }

        // Range checks
        if self.theta_opt < 0.60 || self.theta_opt > 0.90 {
            return false;
        }
        if self.theta_acc < 0.55 || self.theta_acc > 0.85 {
            return false;
        }
        if self.theta_warn < 0.40 || self.theta_warn > 0.70 {
            return false;
        }
        if self.theta_dup < 0.80 || self.theta_dup > 0.98 {
            return false;
        }
        if self.theta_edge < 0.50 || self.theta_edge > 0.85 {
            return false;
        }

        true
    }

    /// Clamp thresholds to valid ranges
    pub fn clamp(&mut self) {
        self.theta_opt = self.theta_opt.clamp(0.60, 0.90);
        self.theta_acc = self.theta_acc.clamp(0.55, 0.85);
        self.theta_warn = self.theta_warn.clamp(0.40, 0.70);
        self.theta_dup = self.theta_dup.clamp(0.80, 0.98);
        self.theta_edge = self.theta_edge.clamp(0.50, 0.85);
    }
}

/// Domain threshold manager
#[derive(Debug)]
pub struct DomainManager {
    thresholds: HashMap<Domain, DomainThresholds>,
}

impl Default for DomainManager {
    fn default() -> Self {
        Self::new()
    }
}

impl DomainManager {
    /// Create new domain manager with defaults
    pub fn new() -> Self {
        let mut thresholds = HashMap::new();

        for domain in [
            Domain::Code,
            Domain::Medical,
            Domain::Legal,
            Domain::Creative,
            Domain::Research,
            Domain::General,
        ] {
            thresholds.insert(domain, DomainThresholds::new(domain));
        }

        Self { thresholds }
    }

    /// Get thresholds for a domain
    pub fn get(&self, domain: Domain) -> Option<&DomainThresholds> {
        self.thresholds.get(&domain)
    }

    /// Get mutable thresholds for a domain
    pub fn get_mut(&mut self, domain: Domain) -> Option<&mut DomainThresholds> {
        self.thresholds.get_mut(&domain)
    }

    /// Update thresholds for a domain
    pub fn update(&mut self, domain: Domain, thresholds: DomainThresholds) -> Result<(), String> {
        if !thresholds.is_valid() {
            return Err("Thresholds fail validity check".to_string());
        }

        self.thresholds.insert(domain, thresholds);
        Ok(())
    }

    /// Transfer learn from one domain to another
    /// Uses: θ_target = α × θ_source + (1 - α) × θ_target
    pub fn transfer_learn(
        &mut self,
        target_domain: Domain,
        source_domain: Domain,
        alpha: f32,
    ) -> Result<(), String> {
        let source_copy = self
            .thresholds
            .get(&source_domain)
            .ok_or("Source domain not found")?
            .clone();

        let target = self
            .thresholds
            .get_mut(&target_domain)
            .ok_or("Target domain not found")?;

        target.blend_with_similar(&source_copy, alpha);
        target.clamp();

        Ok(())
    }

    /// Apply similarity-based transfer learning
    /// Automatically finds similar domain and blends
    pub fn apply_similarity_transfer(
        &mut self,
        domain: Domain,
        alpha: f32,
    ) -> Result<(), String> {
        let similar = domain.find_similar();
        if similar != domain {
            self.transfer_learn(domain, similar, alpha)?;
        }
        Ok(())
    }

    /// Get all domains and their thresholds
    pub fn get_all(&self) -> Vec<(Domain, &DomainThresholds)> {
        self.thresholds
            .iter()
            .map(|(d, t)| (*d, t))
            .collect()
    }

    /// Validate all domains
    pub fn validate_all(&self) -> Vec<(Domain, bool)> {
        self.thresholds
            .iter()
            .map(|(d, t)| (*d, t.is_valid()))
            .collect()
    }

    /// Reset all domains to defaults
    pub fn reset_all(&mut self) {
        for domain in [
            Domain::Code,
            Domain::Medical,
            Domain::Legal,
            Domain::Creative,
            Domain::Research,
            Domain::General,
        ] {
            self.thresholds.insert(domain, DomainThresholds::new(domain));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_descriptions() {
        assert!(!Domain::Code.description().is_empty());
        assert!(!Domain::Medical.description().is_empty());
    }

    #[test]
    fn test_domain_strictness() {
        assert!(Domain::Medical.strictness() > Domain::Creative.strictness());
        assert!(Domain::Code.strictness() > Domain::General.strictness());
    }

    #[test]
    fn test_domain_thresholds_creation() {
        let thresholds = DomainThresholds::new(Domain::Code);
        assert!(thresholds.is_valid());
        assert!(thresholds.theta_opt > thresholds.theta_acc);
        assert!(thresholds.theta_acc > thresholds.theta_warn);
    }

    #[test]
    fn test_domain_differences() {
        let code = DomainThresholds::new(Domain::Code);
        let creative = DomainThresholds::new(Domain::Creative);

        // Code should have stricter thresholds
        assert!(code.theta_opt > creative.theta_opt);
    }

    #[test]
    fn test_blend_with_similar() {
        let mut code = DomainThresholds::new(Domain::Code);
        let research = DomainThresholds::new(Domain::Research);

        code.blend_with_similar(&research, 0.5);
        assert!(code.is_valid());
    }

    #[test]
    fn test_domain_manager() {
        let manager = DomainManager::new();
        assert!(manager.get(Domain::Code).is_some());
        assert!(manager.get(Domain::Medical).is_some());
    }

    #[test]
    fn test_transfer_learning() {
        let mut manager = DomainManager::new();
        let original_opt = manager.get(Domain::Creative).unwrap().theta_opt;

        manager
            .transfer_learn(Domain::Creative, Domain::Code, 0.3)
            .unwrap();

        let new_opt = manager.get(Domain::Creative).unwrap().theta_opt;
        // Should be different after transfer
        assert!((new_opt - original_opt).abs() > 0.01);
    }

    #[test]
    fn test_validate_all() {
        let manager = DomainManager::new();
        let validation = manager.validate_all();

        // All default domains should be valid
        assert!(validation.iter().all(|(_, valid)| *valid));
    }

    #[test]
    fn test_clamping() {
        let mut thresholds = DomainThresholds::new(Domain::Code);
        thresholds.theta_opt = 0.95; // Out of range

        thresholds.clamp();
        assert_eq!(thresholds.theta_opt, 0.90); // Clamped to max

        assert!(thresholds.is_valid());
    }

    #[test]
    fn test_similarity_chain() {
        let code_similar = Domain::Code.find_similar();
        assert_eq!(code_similar, Domain::Research);

        let research_similar = Domain::Research.find_similar();
        assert_eq!(research_similar, Domain::General);
    }
}
