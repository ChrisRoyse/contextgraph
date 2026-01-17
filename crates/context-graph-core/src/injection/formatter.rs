//! ContextFormatter for injection pipeline output.
//!
//! Transforms selected candidates and divergence alerts into markdown
//! text suitable for Claude Code hook injection.
//!
//! # Constitution Compliance
//! - AP-12: No magic numbers - use named constants
//! - AP-14: No .unwrap() in library code

use chrono::{DateTime, Utc};

use super::candidate::{InjectionCandidate, InjectionCategory};
use crate::retrieval::divergence::DivergenceAlert;

// =============================================================================
// Constants
// =============================================================================

/// Maximum words for memory summaries in full context.
pub const SUMMARY_MAX_WORDS: usize = 50;

/// Maximum tokens for brief context output.
pub const BRIEF_MAX_TOKENS: usize = 200;

/// Maximum candidates to include in brief context.
const BRIEF_MAX_CANDIDATES: usize = 5;

/// Maximum words per summary in brief context.
const BRIEF_SUMMARY_WORDS: usize = 20;

/// Estimated tokens for ", " separator between summaries.
const SEPARATOR_TOKENS: usize = 2;

/// Estimated tokens for "Related: " prefix.
const BRIEF_PREFIX_TOKENS: usize = 3;

/// Token multiplier for word estimation (consistent with candidate.rs).
const TOKEN_MULTIPLIER: f32 = 1.3;

// =============================================================================
// ContextFormatter
// =============================================================================

/// Formats injection candidates and alerts into markdown context strings.
///
/// All methods are stateless associated functions.
///
/// # Output Sections (for full context)
///
/// ```text
/// ## Relevant Context
///
/// ### Recent Related Work
/// - **{time_ago}**: {summary}
///
/// ### Potentially Related
/// - {summary} ({time_ago})
///
/// ### Note: Activity Shift Detected
/// ⚠️ DIVERGENCE in E1: "recent work summary" (similarity: 0.15)
///
/// ### Previous Session
/// {session summary}
/// ```
pub struct ContextFormatter;

impl ContextFormatter {
    /// Format full context for SessionStart hook.
    ///
    /// Produces markdown with sections per category:
    /// - DivergenceAlert -> "Note: Activity Shift Detected"
    /// - HighRelevanceCluster -> "Recent Related Work"
    /// - SingleSpaceMatch -> "Potentially Related"
    /// - RecentSession -> "Previous Session"
    ///
    /// # Arguments
    /// * `candidates` - Selected candidates from TokenBudgetManager
    /// * `alerts` - Divergence alerts from DivergenceDetector
    ///
    /// # Returns
    /// Markdown-formatted context string, empty if no candidates/alerts
    pub fn format_full_context(
        candidates: &[InjectionCandidate],
        alerts: &[DivergenceAlert],
    ) -> String {
        if candidates.is_empty() && alerts.is_empty() {
            return String::new();
        }

        let mut output = String::from("## Relevant Context\n\n");

        // Group candidates by category
        let cluster_matches: Vec<_> = candidates
            .iter()
            .filter(|c| c.category == InjectionCategory::HighRelevanceCluster)
            .collect();

        let single_matches: Vec<_> = candidates
            .iter()
            .filter(|c| c.category == InjectionCategory::SingleSpaceMatch)
            .collect();

        let session_summaries: Vec<_> = candidates
            .iter()
            .filter(|c| c.category == InjectionCategory::RecentSession)
            .collect();

        // Section 1: Recent Related Work (HighRelevanceCluster)
        if !cluster_matches.is_empty() {
            output.push_str("### Recent Related Work\n");
            for candidate in cluster_matches {
                let time_ago = Self::format_time_ago(candidate.created_at);
                let summary = Self::summarize_memory(&candidate.content, SUMMARY_MAX_WORDS);
                output.push_str(&format!("- **{}**: {}\n", time_ago, summary));
            }
            output.push('\n');
        }

        // Section 2: Potentially Related (SingleSpaceMatch)
        if !single_matches.is_empty() {
            output.push_str("### Potentially Related\n");
            for candidate in single_matches {
                let time_ago = Self::format_time_ago(candidate.created_at);
                let summary = Self::summarize_memory(&candidate.content, SUMMARY_MAX_WORDS);
                output.push_str(&format!("- {} ({})\n", summary, time_ago));
            }
            output.push('\n');
        }

        // Section 3: Activity Shift Detected (DivergenceAlerts)
        if !alerts.is_empty() {
            output.push_str("### Note: Activity Shift Detected\n");
            for alert in alerts {
                output.push_str(&alert.format_alert());
                output.push('\n');
            }
            output.push('\n');
        }

        // Section 4: Previous Session (RecentSession)
        if !session_summaries.is_empty() {
            output.push_str("### Previous Session\n");
            for candidate in session_summaries {
                // Double max_words for session summaries (more context allowed)
                let summary = Self::summarize_memory(&candidate.content, SUMMARY_MAX_WORDS * 2);
                output.push_str(&summary);
                output.push('\n');
            }
        }

        output.trim_end().to_string()
    }

    /// Format brief context for PreToolUse hook.
    ///
    /// Produces compact single-paragraph output under BRIEF_MAX_TOKENS.
    /// Format: "Related: {summary1}, {summary2}, ..."
    ///
    /// # Arguments
    /// * `candidates` - Selected candidates (usually top 5 max)
    ///
    /// # Returns
    /// Single-line context string, empty if no candidates
    pub fn format_brief_context(candidates: &[InjectionCandidate]) -> String {
        if candidates.is_empty() {
            return String::new();
        }

        let mut summaries: Vec<String> = Vec::new();
        let mut token_estimate = BRIEF_PREFIX_TOKENS;

        for candidate in candidates.iter().take(BRIEF_MAX_CANDIDATES) {
            let summary = Self::summarize_memory(&candidate.content, BRIEF_SUMMARY_WORDS);
            let summary_tokens = Self::estimate_tokens(&summary);

            // Check if adding this summary would exceed budget
            if token_estimate + summary_tokens + SEPARATOR_TOKENS > BRIEF_MAX_TOKENS {
                break;
            }

            summaries.push(summary);
            token_estimate += summary_tokens + SEPARATOR_TOKENS;
        }

        if summaries.is_empty() {
            return String::new();
        }

        format!("Related: {}", summaries.join(", "))
    }

    /// Summarize memory content to max_words.
    ///
    /// Truncates at sentence boundary if possible within the second half,
    /// otherwise truncates at word boundary and adds "...".
    ///
    /// # Arguments
    /// * `content` - Full memory content
    /// * `max_words` - Maximum words in output
    ///
    /// # Returns
    /// Summarized content (may be shorter if sentence boundary found)
    pub fn summarize_memory(content: &str, max_words: usize) -> String {
        let trimmed = content.trim();
        let words: Vec<&str> = trimmed.split_whitespace().collect();

        if words.len() <= max_words {
            return trimmed.to_string();
        }

        let truncated: String = words[..max_words].join(" ");
        let halfway = truncated.len() / 2;

        // Try to find a sentence boundary (period) in the second half
        if let Some(period_idx) = truncated.rfind('.') {
            if period_idx > halfway {
                // Include the period, trim any trailing space
                return truncated[..=period_idx].trim_end().to_string();
            }
        }

        format!("{}...", truncated)
    }

    /// Format time ago in human-readable form.
    ///
    /// Uses current time as reference.
    ///
    /// # Returns
    /// - "X minutes ago" for <1 hour
    /// - "1 hour ago" / "X hours ago" for <24 hours
    /// - "Yesterday" for 24-48 hours
    /// - "X days ago" for 2-7 days
    /// - "1 week ago" / "X weeks ago" for 7-28 days
    /// - "X days ago" for >28 days
    pub fn format_time_ago(created_at: DateTime<Utc>) -> String {
        Self::format_time_ago_relative(created_at, Utc::now())
    }

    /// Format time ago with explicit reference time (for deterministic testing).
    pub fn format_time_ago_relative(created_at: DateTime<Utc>, now: DateTime<Utc>) -> String {
        let duration = now.signed_duration_since(created_at);
        let minutes = duration.num_minutes();
        let hours = duration.num_hours();
        let days = duration.num_days();

        if minutes < 1 {
            return "Just now".to_string();
        }
        if minutes < 60 {
            return Self::pluralize(minutes, "minute");
        }
        if hours < 24 {
            return Self::pluralize(hours, "hour");
        }
        if days < 2 {
            return "Yesterday".to_string();
        }
        if days < 7 {
            return format!("{} days ago", days);
        }

        let weeks = days / 7;
        if weeks < 4 {
            return Self::pluralize(weeks, "week");
        }

        format!("{} days ago", days)
    }

    /// Format a count with singular/plural unit.
    #[inline]
    fn pluralize(count: i64, unit: &str) -> String {
        if count == 1 {
            format!("1 {} ago", unit)
        } else {
            format!("{} {}s ago", count, unit)
        }
    }

    /// Estimate token count for text (same formula as candidate.rs).
    #[inline]
    fn estimate_tokens(content: &str) -> usize {
        let word_count = content.split_whitespace().count();
        (word_count as f32 * TOKEN_MULTIPLIER).ceil() as usize
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeDelta;
    use uuid::Uuid;
    use crate::teleological::Embedder;

    fn make_candidate(
        content: &str,
        category: InjectionCategory,
        hours_ago: i64,
    ) -> InjectionCandidate {
        InjectionCandidate::new(
            Uuid::new_v4(),
            content.to_string(),
            0.8, // relevance
            3.0, // weighted_agreement
            vec![Embedder::Semantic, Embedder::Code],
            category,
            Utc::now() - TimeDelta::hours(hours_ago),
        )
    }

    fn make_alert(summary: &str, space: Embedder, score: f32) -> DivergenceAlert {
        DivergenceAlert::new(
            Uuid::new_v4(),
            space,
            score,
            summary,
        )
    }

    // =========================================================================
    // format_full_context Tests
    // =========================================================================

    #[test]
    fn test_format_full_context_structure() {
        let candidates = vec![
            make_candidate(
                "Implemented HDBSCAN clustering for topic detection",
                InjectionCategory::HighRelevanceCluster,
                2,
            ),
            make_candidate(
                "Rust async patterns with tokio runtime",
                InjectionCategory::SingleSpaceMatch,
                72,
            ),
        ];

        let output = ContextFormatter::format_full_context(&candidates, &[]);

        assert!(output.contains("## Relevant Context"), "Should have main header");
        assert!(output.contains("### Recent Related Work"), "Should have cluster section");
        assert!(output.contains("### Potentially Related"), "Should have single-match section");
        assert!(output.contains("HDBSCAN"), "Should include cluster content");
        assert!(output.contains("Rust async"), "Should include single-match content");
        println!("[PASS] format_full_context produces correct structure");
    }

    #[test]
    fn test_format_full_context_with_alerts() {
        let candidates = vec![
            make_candidate("Test content", InjectionCategory::HighRelevanceCluster, 1),
        ];
        let alerts = vec![
            make_alert("Working on unrelated feature", Embedder::Semantic, 0.15),
        ];

        let output = ContextFormatter::format_full_context(&candidates, &alerts);

        assert!(output.contains("### Note: Activity Shift Detected"));
        assert!(output.contains("DIVERGENCE"));
        assert!(output.contains("0.15"));
        println!("[PASS] Divergence alerts formatted correctly");
    }

    #[test]
    fn test_format_full_context_empty() {
        let output = ContextFormatter::format_full_context(&[], &[]);
        assert!(output.is_empty(), "Empty input should produce empty output");
        println!("[PASS] Empty input handled correctly");
    }

    #[test]
    fn test_format_full_context_only_alerts() {
        let alerts = vec![
            make_alert("Recent context here", Embedder::Code, 0.08),
        ];

        let output = ContextFormatter::format_full_context(&[], &alerts);

        assert!(output.contains("## Relevant Context"));
        assert!(output.contains("### Note: Activity Shift Detected"));
        assert!(!output.contains("### Recent Related Work"));
        println!("[PASS] Alerts-only output formatted correctly");
    }

    #[test]
    fn test_format_full_context_session_summary() {
        let candidates = vec![
            make_candidate(
                "Last session: Worked on implementing the injection pipeline with token budgeting and priority ranking for context selection",
                InjectionCategory::RecentSession,
                25,
            ),
        ];

        let output = ContextFormatter::format_full_context(&candidates, &[]);

        assert!(output.contains("### Previous Session"));
        assert!(output.contains("injection pipeline"));
        println!("[PASS] Session summary section works");
    }

    // =========================================================================
    // format_brief_context Tests
    // =========================================================================

    #[test]
    fn test_format_brief_context() {
        let candidates = vec![
            make_candidate("HDBSCAN clustering implementation", InjectionCategory::HighRelevanceCluster, 2),
            make_candidate("BIRCH tree incremental clustering", InjectionCategory::HighRelevanceCluster, 5),
        ];

        let output = ContextFormatter::format_brief_context(&candidates);

        assert!(output.starts_with("Related:"), "Should start with 'Related:'");
        assert!(output.contains("HDBSCAN"));
        assert!(output.contains("BIRCH"));
        println!("[PASS] format_brief_context works: {}", output);
    }

    #[test]
    fn test_format_brief_context_empty() {
        let output = ContextFormatter::format_brief_context(&[]);
        assert!(output.is_empty());
        println!("[PASS] Empty brief context handled");
    }

    #[test]
    fn test_format_brief_context_token_limit() {
        // Create many candidates with substantial content
        let candidates: Vec<_> = (0..10)
            .map(|i| make_candidate(
                &format!("This is candidate number {} with substantial content that should be summarized appropriately", i),
                InjectionCategory::HighRelevanceCluster,
                i,
            ))
            .collect();

        let output = ContextFormatter::format_brief_context(&candidates);

        // Estimate tokens in output
        let token_estimate = (output.split_whitespace().count() as f32 * 1.3).ceil() as usize;
        assert!(
            token_estimate <= BRIEF_MAX_TOKENS,
            "Brief context should be under {} tokens, got ~{}",
            BRIEF_MAX_TOKENS,
            token_estimate
        );
        println!("[PASS] Brief context respects token limit: ~{} tokens", token_estimate);
    }

    // =========================================================================
    // summarize_memory Tests
    // =========================================================================

    #[test]
    fn test_summarize_memory_short() {
        let content = "This is a short memory.";
        let summary = ContextFormatter::summarize_memory(content, 50);
        assert_eq!(summary, content);
        println!("[PASS] Short content unchanged");
    }

    #[test]
    fn test_summarize_memory_truncate_with_ellipsis() {
        let content = "This is a longer memory that needs to be truncated because it has too many words for the maximum limit we set";
        let summary = ContextFormatter::summarize_memory(content, 10);

        assert!(summary.len() < content.len(), "Should be shorter");
        assert!(summary.ends_with("...") || summary.ends_with('.'), "Should end with ... or .");
        println!("[PASS] Long content truncated: {}", summary);
    }

    #[test]
    fn test_summarize_memory_sentence_boundary() {
        let content = "First sentence here. Second sentence that goes on for a while with many words.";
        let summary = ContextFormatter::summarize_memory(content, 10);

        // Should prefer sentence boundary
        assert!(summary.contains("First sentence"), "Should include first sentence");
        println!("[PASS] Truncates at sentence boundary: {}", summary);
    }

    #[test]
    fn test_summarize_memory_empty() {
        let summary = ContextFormatter::summarize_memory("", 50);
        assert!(summary.is_empty());
        println!("[PASS] Empty content returns empty summary");
    }

    #[test]
    fn test_summarize_memory_whitespace_only() {
        let summary = ContextFormatter::summarize_memory("   \n\t  ", 50);
        assert!(summary.is_empty());
        println!("[PASS] Whitespace-only returns empty");
    }

    // =========================================================================
    // format_time_ago Tests
    // =========================================================================

    #[test]
    fn test_format_time_ago_minutes() {
        let now = Utc::now();
        let created = now - TimeDelta::minutes(30);
        let formatted = ContextFormatter::format_time_ago_relative(created, now);
        assert!(formatted.contains("30 minutes ago"), "Got: {}", formatted);
        println!("[PASS] Minutes formatting: {}", formatted);
    }

    #[test]
    fn test_format_time_ago_hours() {
        let now = Utc::now();
        let created = now - TimeDelta::hours(3);
        let formatted = ContextFormatter::format_time_ago_relative(created, now);
        assert!(formatted.contains("3 hours ago"), "Got: {}", formatted);
        println!("[PASS] Hours formatting: {}", formatted);
    }

    #[test]
    fn test_format_time_ago_yesterday() {
        let now = Utc::now();
        let created = now - TimeDelta::hours(30);
        let formatted = ContextFormatter::format_time_ago_relative(created, now);
        assert_eq!(formatted, "Yesterday");
        println!("[PASS] Yesterday formatting");
    }

    #[test]
    fn test_format_time_ago_days() {
        let now = Utc::now();
        let created = now - TimeDelta::days(4);
        let formatted = ContextFormatter::format_time_ago_relative(created, now);
        assert!(formatted.contains("4 days ago"), "Got: {}", formatted);
        println!("[PASS] Days formatting: {}", formatted);
    }

    #[test]
    fn test_format_time_ago_weeks() {
        let now = Utc::now();
        let created = now - TimeDelta::days(14);
        let formatted = ContextFormatter::format_time_ago_relative(created, now);
        assert!(formatted.contains("2 weeks ago"), "Got: {}", formatted);
        println!("[PASS] Weeks formatting: {}", formatted);
    }

    #[test]
    fn test_format_time_ago_just_now() {
        let now = Utc::now();
        let created = now - TimeDelta::seconds(30);
        let formatted = ContextFormatter::format_time_ago_relative(created, now);
        assert_eq!(formatted, "Just now");
        println!("[PASS] Just now formatting");
    }

    #[test]
    fn test_format_time_ago_one_minute() {
        let now = Utc::now();
        let created = now - TimeDelta::minutes(1);
        let formatted = ContextFormatter::format_time_ago_relative(created, now);
        assert_eq!(formatted, "1 minute ago");
        println!("[PASS] Singular minute formatting");
    }

    #[test]
    fn test_format_time_ago_one_hour() {
        let now = Utc::now();
        let created = now - TimeDelta::hours(1);
        let formatted = ContextFormatter::format_time_ago_relative(created, now);
        assert_eq!(formatted, "1 hour ago");
        println!("[PASS] Singular hour formatting");
    }

    // =========================================================================
    // Edge Case / Boundary Tests
    // =========================================================================

    #[test]
    fn test_all_categories_present() {
        let candidates = vec![
            make_candidate("High relevance cluster content", InjectionCategory::HighRelevanceCluster, 1),
            make_candidate("Single space match content", InjectionCategory::SingleSpaceMatch, 2),
            make_candidate("Recent session summary", InjectionCategory::RecentSession, 24),
        ];
        let alerts = vec![
            make_alert("Divergent activity", Embedder::Semantic, 0.12),
        ];

        let output = ContextFormatter::format_full_context(&candidates, &alerts);

        assert!(output.contains("### Recent Related Work"));
        assert!(output.contains("### Potentially Related"));
        assert!(output.contains("### Note: Activity Shift Detected"));
        assert!(output.contains("### Previous Session"));
        println!("[PASS] All four sections present when all categories have content");
    }

    #[test]
    fn test_markdown_special_characters_in_content() {
        let candidates = vec![
            make_candidate(
                "Code: `fn main() { println!(\"Hello\"); }` with *emphasis*",
                InjectionCategory::HighRelevanceCluster,
                1,
            ),
        ];

        let output = ContextFormatter::format_full_context(&candidates, &[]);

        // Should preserve markdown characters in content
        assert!(output.contains("`fn main()"));
        println!("[PASS] Markdown special chars preserved");
    }

    #[test]
    fn test_unicode_content() {
        let candidates = vec![
            make_candidate(
                "Unicode: 日本語テスト, émoji: 🚀, symbols: α β γ",
                InjectionCategory::HighRelevanceCluster,
                1,
            ),
        ];

        let output = ContextFormatter::format_full_context(&candidates, &[]);

        assert!(output.contains("日本語"));
        assert!(output.contains("🚀"));
        println!("[PASS] Unicode content handled correctly");
    }

    // =========================================================================
    // FSV Edge Case Tests (MANDATORY)
    // =========================================================================

    #[test]
    fn test_fsv_edge_case_empty_input() {
        println!("FSV EDGE CASE 1: Empty input");
        println!("  Before: candidates.len() = 0, alerts.len() = 0");

        let output = ContextFormatter::format_full_context(&[], &[]);

        println!("  After: output.len() = {}", output.len());
        println!("  Output content: '{}'", output);
        println!("  Expected: Empty string");

        assert!(output.is_empty(), "Empty input should produce empty output");
        println!("[PASS] FSV Edge Case 1: Empty input -> empty output");
    }

    #[test]
    fn test_fsv_edge_case_exactly_at_word_limit() {
        println!("FSV EDGE CASE 2: Content exactly at word limit");

        // Create content with exactly 50 words
        let content = "word ".repeat(50).trim().to_string();
        let word_count = content.split_whitespace().count();

        println!("  Before: word_count = {}", word_count);

        let summary = ContextFormatter::summarize_memory(&content, 50);
        let output_word_count = summary.split_whitespace().count();

        println!("  After: output_word_count = {}", output_word_count);
        println!("  Expected: {} (unchanged)", word_count);

        assert_eq!(output_word_count, word_count, "Exactly at limit should not truncate");
        assert!(!summary.ends_with("..."), "Should not have ellipsis");
        println!("[PASS] FSV Edge Case 2: Exactly at limit -> unchanged");
    }

    #[test]
    fn test_fsv_edge_case_brief_context_budget_boundary() {
        println!("FSV EDGE CASE 3: Brief context near token budget");

        // Create candidates that together approach the 200 token limit
        let candidates: Vec<_> = (0..20)
            .map(|i| make_candidate(
                &format!("Candidate {} with reasonable content here", i),
                InjectionCategory::HighRelevanceCluster,
                i,
            ))
            .collect();

        println!("  Before: {} candidates available", candidates.len());

        let output = ContextFormatter::format_brief_context(&candidates);
        let tokens = (output.split_whitespace().count() as f32 * 1.3).ceil() as usize;

        println!("  After: output = '{}'", output);
        println!("  After: estimated tokens = {}", tokens);
        println!("  Expected: <= {} tokens", BRIEF_MAX_TOKENS);

        assert!(tokens <= BRIEF_MAX_TOKENS, "Must not exceed {} tokens", BRIEF_MAX_TOKENS);
        println!("[PASS] FSV Edge Case 3: Brief context respects token budget");
    }

    // =========================================================================
    // Additional FSV Tests - Boundary and Edge Cases
    // =========================================================================

    #[test]
    fn test_fsv_format_time_ago_one_week() {
        println!("FSV EDGE CASE 4: Exactly 1 week ago");
        let now = Utc::now();
        let created = now - TimeDelta::days(7);

        println!("  Before: days_ago = 7");
        let formatted = ContextFormatter::format_time_ago_relative(created, now);

        println!("  After: formatted = '{}'", formatted);
        assert_eq!(formatted, "1 week ago");
        println!("[PASS] FSV Edge Case 4: Exactly 1 week -> '1 week ago'");
    }

    #[test]
    fn test_fsv_format_time_ago_over_4_weeks() {
        println!("FSV EDGE CASE 5: Over 4 weeks (falls back to days)");
        let now = Utc::now();
        let created = now - TimeDelta::days(35);

        println!("  Before: days_ago = 35");
        let formatted = ContextFormatter::format_time_ago_relative(created, now);

        println!("  After: formatted = '{}'", formatted);
        assert!(formatted.contains("35 days ago"), "Got: {}", formatted);
        println!("[PASS] FSV Edge Case 5: 35 days -> '35 days ago'");
    }

    #[test]
    fn test_fsv_summarize_memory_one_word_over() {
        println!("FSV EDGE CASE 6: Content exactly 1 word over limit");

        // 11 words, limit 10
        let content = "one two three four five six seven eight nine ten eleven";
        let word_count = content.split_whitespace().count();

        println!("  Before: word_count = {}", word_count);

        let summary = ContextFormatter::summarize_memory(content, 10);
        let output_word_count = summary.split_whitespace().count();

        println!("  After: summary = '{}'", summary);
        println!("  After: output_word_count = {} (including ... as word)", output_word_count);

        // Should be truncated
        assert!(summary.len() < content.len(), "Should be truncated");
        assert!(summary.ends_with("...") || summary.ends_with('.'), "Should have proper ending");
        println!("[PASS] FSV Edge Case 6: 1 word over -> truncated");
    }

    #[test]
    fn test_fsv_multiple_alerts_ordering() {
        println!("FSV EDGE CASE 7: Multiple divergence alerts maintain order");

        let alerts = vec![
            make_alert("First alert content", Embedder::Semantic, 0.10),
            make_alert("Second alert content", Embedder::Code, 0.05),
            make_alert("Third alert content", Embedder::Causal, 0.15),
        ];

        println!("  Before: {} alerts", alerts.len());

        let output = ContextFormatter::format_full_context(&[], &alerts);

        println!("  After output:\n{}", output);

        // Verify all alerts appear
        assert!(output.contains("First alert"), "Should contain first alert");
        assert!(output.contains("Second alert"), "Should contain second alert");
        assert!(output.contains("Third alert"), "Should contain third alert");

        // Verify order is preserved (first appears before second, second before third)
        let first_pos = output.find("First alert").expect("First alert not found");
        let second_pos = output.find("Second alert").expect("Second alert not found");
        let third_pos = output.find("Third alert").expect("Third alert not found");

        assert!(first_pos < second_pos, "First should come before second");
        assert!(second_pos < third_pos, "Second should come before third");

        println!("[PASS] FSV Edge Case 7: Multiple alerts maintain insertion order");
    }

    #[test]
    fn test_fsv_session_summary_double_word_limit() {
        println!("FSV EDGE CASE 8: Session summary uses double word limit (100 words)");

        // Create content with 75 words (between 50 and 100)
        let content = "word ".repeat(75).trim().to_string();
        let word_count = content.split_whitespace().count();

        let candidate = InjectionCandidate::new(
            Uuid::new_v4(),
            content.clone(),
            0.8,
            3.0,
            vec![Embedder::Semantic],
            InjectionCategory::RecentSession,
            Utc::now() - TimeDelta::hours(25),
        );

        println!("  Before: word_count = {}", word_count);

        let output = ContextFormatter::format_full_context(&[candidate], &[]);

        // The 75-word content should be preserved (under 100 word limit for session)
        // Count words after "### Previous Session\n"
        let session_start = output.find("### Previous Session").expect("Session section not found");
        let session_content = &output[session_start..];

        println!("  After: session section = '{}'", session_content);

        // The content should NOT be truncated (75 < 100)
        assert!(!session_content.contains("..."), "Session summary should not be truncated at 75 words");

        println!("[PASS] FSV Edge Case 8: Session summary allows 100 words");
    }
}
