//! Topic management commands
//!
//! Commands for topic portfolio exploration and stability analysis.
//!
//! # Commands
//!
//! - `topic portfolio`: Get discovered topics with profiles
//! - `topic stability`: Get portfolio-level stability metrics
//! - `topic detect`: Force topic detection recalculation
//!
//! # Constitution Compliance
//!
//! - ARCH-09: Topic threshold is weighted_agreement >= 2.5
//! - AP-60: Temporal embedders MUST NOT count toward topics
//! - AP-61: Topic threshold MUST be weighted_agreement >= 2.5
//! - AP-26: Exit code 1 on error, 2 on corruption

use clap::{Args, Subcommand, ValueEnum};
use tracing::{error, info};

use crate::mcp_client::McpClient;

/// Topic management subcommands.
///
/// These commands explore emergent topic portfolio and stability metrics.
/// Topics emerge from weighted multi-space clustering - they are NOT manually defined.
#[derive(Subcommand)]
pub enum TopicCommands {
    /// Get discovered topics with profiles and stability metrics
    ///
    /// Shows all topics that have emerged from weighted multi-space clustering.
    /// Topics require weighted_agreement >= 2.5 to be recognized.
    ///
    /// # Examples
    ///
    /// ```bash
    /// # Default standard format
    /// context-graph-cli topic portfolio
    ///
    /// # Brief format (names + confidence only)
    /// context-graph-cli topic portfolio --format brief
    ///
    /// # Verbose format (full profiles with all 13 embedder strengths)
    /// context-graph-cli topic portfolio --format verbose
    /// ```
    Portfolio(PortfolioArgs),

    /// Get portfolio-level stability metrics
    ///
    /// Shows churn rate, entropy, and phase breakdown.
    /// Dream consolidation is recommended when entropy > 0.7 AND churn > 0.5.
    ///
    /// # Examples
    ///
    /// ```bash
    /// # Default 6-hour lookback
    /// context-graph-cli topic stability
    ///
    /// # Custom lookback period
    /// context-graph-cli topic stability --hours 24
    /// ```
    Stability(StabilityArgs),

    /// Force topic detection recalculation
    ///
    /// Runs HDBSCAN clustering on all memories to detect topics.
    /// Requires minimum 3 memories (per clustering.parameters.min_cluster_size).
    ///
    /// # Examples
    ///
    /// ```bash
    /// # Normal detection (skips if recently computed)
    /// context-graph-cli topic detect
    ///
    /// # Force detection even if recent
    /// context-graph-cli topic detect --force
    /// ```
    Detect(DetectArgs),
}

/// Output format for topic portfolio.
#[derive(Copy, Clone, Debug, Default, ValueEnum)]
pub enum TopicFormat {
    /// Names + confidence only
    Brief,
    /// Includes contributing embedding spaces
    #[default]
    Standard,
    /// Full profiles with all 13 embedder strengths
    Verbose,
}

impl TopicFormat {
    fn as_str(&self) -> &'static str {
        match self {
            TopicFormat::Brief => "brief",
            TopicFormat::Standard => "standard",
            TopicFormat::Verbose => "verbose",
        }
    }
}

/// Arguments for topic portfolio command.
#[derive(Args)]
pub struct PortfolioArgs {
    /// Output format (brief, standard, verbose)
    #[arg(short, long, default_value = "standard")]
    pub format: TopicFormat,

    /// Output as JSON instead of human-readable
    #[arg(long)]
    pub json: bool,
}

/// Arguments for topic stability command.
#[derive(Args)]
pub struct StabilityArgs {
    /// Lookback period in hours (1-168)
    #[arg(short = 'H', long, default_value = "6")]
    pub hours: u32,

    /// Output as JSON instead of human-readable
    #[arg(long)]
    pub json: bool,
}

/// Arguments for topic detect command.
#[derive(Args)]
pub struct DetectArgs {
    /// Force detection even if recently computed
    #[arg(short, long)]
    pub force: bool,

    /// Output as JSON instead of human-readable
    #[arg(long)]
    pub json: bool,
}

/// Handle topic subcommands.
///
/// Routes to appropriate handler based on subcommand.
/// Returns exit code per AP-26: 0=success, 1=error, 2=corruption.
pub async fn handle_topic_command(cmd: TopicCommands) -> i32 {
    match cmd {
        TopicCommands::Portfolio(args) => handle_portfolio(args).await,
        TopicCommands::Stability(args) => handle_stability(args).await,
        TopicCommands::Detect(args) => handle_detect(args).await,
    }
}

/// Handle topic portfolio command.
async fn handle_portfolio(args: PortfolioArgs) -> i32 {
    let client = McpClient::new();

    // Check if server is running
    match client.is_server_running().await {
        Ok(true) => {}
        Ok(false) => {
            eprintln!(
                "Error: MCP server not running at {}",
                client.server_address()
            );
            eprintln!("Start the server with: context-graph-mcp");
            return 1;
        }
        Err(e) => {
            error!("Failed to check server status: {}", e);
            eprintln!("Error: {}", e);
            return 1;
        }
    }

    // Call MCP tool
    match client.get_topic_portfolio(Some(args.format.as_str())).await {
        Ok(result) => {
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result).unwrap_or_default()
                );
            } else {
                print_portfolio(&result, args.format);
            }
            info!("Topic portfolio retrieved successfully");
            0
        }
        Err(e) => {
            error!("Failed to get topic portfolio: {}", e);
            eprintln!("Error: {}", e);
            1
        }
    }
}

/// Handle topic stability command.
async fn handle_stability(args: StabilityArgs) -> i32 {
    // CLI-M1 FIX: Validate hours range (1-168) before any server interaction.
    // Documented range is 1-168 (weekly). Reject 0 and values > 168.
    if args.hours == 0 || args.hours > 168 {
        eprintln!(
            "Error: --hours must be between 1 and 168, got {}",
            args.hours
        );
        return 1;
    }

    let client = McpClient::new();

    // Check if server is running
    match client.is_server_running().await {
        Ok(true) => {}
        Ok(false) => {
            eprintln!(
                "Error: MCP server not running at {}",
                client.server_address()
            );
            eprintln!("Start the server with: context-graph-mcp");
            return 1;
        }
        Err(e) => {
            error!("Failed to check server status: {}", e);
            eprintln!("Error: {}", e);
            return 1;
        }
    }

    // Call MCP tool
    match client.get_topic_stability(Some(args.hours)).await {
        Ok(result) => {
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result).unwrap_or_default()
                );
            } else {
                print_stability(&result);
            }
            info!("Topic stability retrieved successfully");
            0
        }
        Err(e) => {
            error!("Failed to get topic stability: {}", e);
            eprintln!("Error: {}", e);
            1
        }
    }
}

/// Handle topic detect command.
async fn handle_detect(args: DetectArgs) -> i32 {
    let client = McpClient::new();

    // Check if server is running
    match client.is_server_running().await {
        Ok(true) => {}
        Ok(false) => {
            eprintln!(
                "Error: MCP server not running at {}",
                client.server_address()
            );
            eprintln!("Start the server with: context-graph-mcp");
            return 1;
        }
        Err(e) => {
            error!("Failed to check server status: {}", e);
            eprintln!("Error: {}", e);
            return 1;
        }
    }

    // Call MCP tool
    match client.detect_topics(args.force).await {
        Ok(result) => {
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result).unwrap_or_default()
                );
            } else {
                print_detection_result(&result);
            }
            info!("Topic detection completed successfully");
            0
        }
        Err(e) => {
            error!("Failed to detect topics: {}", e);
            eprintln!("Error: {}", e);
            1
        }
    }
}

/// Format portfolio as human-readable string.
///
/// TST-M4 FIX: Extracted from `print_portfolio` so tests can assert on content.
fn format_portfolio(result: &serde_json::Value, format: TopicFormat) -> String {
    use std::fmt::Write;
    let mut out = String::new();

    writeln!(out, "Topic Portfolio").unwrap();
    writeln!(out, "===============\n").unwrap();

    if let Some(topics) = result.get("topics").and_then(|t| t.as_array()) {
        if topics.is_empty() {
            writeln!(out, "No topics discovered yet.").unwrap();
            writeln!(out, "Tip: Topics emerge when weighted_agreement >= 2.5").unwrap();
            return out;
        }

        for topic in topics {
            let name = topic
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("Unknown");
            let confidence = topic
                .get("confidence")
                .and_then(|c| c.as_f64())
                .unwrap_or(0.0);
            let weighted_agreement = topic
                .get("weighted_agreement")
                .and_then(|w| w.as_f64())
                .unwrap_or(0.0);

            match format {
                TopicFormat::Brief => {
                    writeln!(out, "  - {} (confidence: {:.2})", name, confidence).unwrap();
                }
                TopicFormat::Standard => {
                    writeln!(out, "Topic: {}", name).unwrap();
                    writeln!(out, "  Confidence: {:.2}", confidence).unwrap();
                    writeln!(out, "  Weighted Agreement: {:.2}", weighted_agreement).unwrap();
                    if let Some(spaces) =
                        topic.get("contributing_spaces").and_then(|s| s.as_array())
                    {
                        let space_names: Vec<&str> =
                            spaces.iter().filter_map(|s| s.as_str()).collect();
                        writeln!(out, "  Contributing Spaces: {}", space_names.join(", ")).unwrap();
                    }
                    writeln!(out).unwrap();
                }
                TopicFormat::Verbose => {
                    writeln!(out, "Topic: {}", name).unwrap();
                    writeln!(out, "  Confidence: {:.2}", confidence).unwrap();
                    writeln!(out, "  Weighted Agreement: {:.2}", weighted_agreement).unwrap();
                    if let Some(spaces) =
                        topic.get("contributing_spaces").and_then(|s| s.as_array())
                    {
                        let space_names: Vec<&str> =
                            spaces.iter().filter_map(|s| s.as_str()).collect();
                        writeln!(out, "  Contributing Spaces: {}", space_names.join(", ")).unwrap();
                    }
                    if let Some(strengths) =
                        topic.get("embedder_strengths").and_then(|s| s.as_object())
                    {
                        writeln!(out, "  Embedder Strengths:").unwrap();
                        for (embedder, strength) in strengths {
                            let val = strength.as_f64().unwrap_or(0.0);
                            if val > 0.0 {
                                writeln!(out, "    {}: {:.3}", embedder, val).unwrap();
                            }
                        }
                    }
                    writeln!(out).unwrap();
                }
            }
        }

        writeln!(out, "Total: {} topics", topics.len()).unwrap();
    } else {
        writeln!(out, "No topic data available.").unwrap();
    }

    out
}

/// Print portfolio in human-readable format.
fn print_portfolio(result: &serde_json::Value, format: TopicFormat) {
    print!("{}", format_portfolio(result, format));
}

/// Format stability metrics as human-readable string.
///
/// TST-M4 FIX: Extracted from `print_stability` so tests can assert on content.
fn format_stability(result: &serde_json::Value) -> String {
    use std::fmt::Write;
    let mut out = String::new();

    writeln!(out, "Topic Stability Metrics").unwrap();
    writeln!(out, "=======================\n").unwrap();

    let churn_rate = result
        .get("churn_rate")
        .and_then(|c| c.as_f64())
        .unwrap_or(0.0);
    let entropy = result
        .get("entropy")
        .and_then(|e| e.as_f64())
        .unwrap_or(0.0);
    let dream_recommended = result
        .get("dream_recommended")
        .and_then(|d| d.as_bool())
        .unwrap_or(false);

    // Churn rate with status indicator
    let churn_status = if churn_rate < 0.3 {
        "Healthy"
    } else if churn_rate < 0.5 {
        "Warning"
    } else {
        "Unstable"
    };
    writeln!(out, "Churn Rate: {:.2} ({})", churn_rate, churn_status).unwrap();

    // Entropy with status
    let entropy_status = if entropy < 0.5 {
        "Low"
    } else if entropy < 0.7 {
        "Moderate"
    } else {
        "High"
    };
    writeln!(out, "Entropy: {:.2} ({})", entropy, entropy_status).unwrap();

    // Phase breakdown
    if let Some(phases) = result.get("phases").and_then(|p| p.as_object()) {
        writeln!(out, "\nPhase Breakdown:").unwrap();
        for (phase, count) in phases {
            let count_val = count.as_u64().unwrap_or(0);
            if count_val > 0 {
                writeln!(out, "  {}: {}", phase, count_val).unwrap();
            }
        }
    }

    // Status
    writeln!(out).unwrap();
    if dream_recommended {
        writeln!(out, "! High entropy/churn detected").unwrap();
        writeln!(out, "  (entropy > 0.7 AND churn > 0.5)").unwrap();
    } else {
        writeln!(out, "Topic structure is stable.").unwrap();
    }

    out
}

/// Print stability metrics in human-readable format.
fn print_stability(result: &serde_json::Value) {
    print!("{}", format_stability(result));
}

/// Print detection result in human-readable format.
fn print_detection_result(result: &serde_json::Value) {
    println!("Topic Detection Results");
    println!("=======================\n");

    let topics_found = result
        .get("topics_found")
        .and_then(|t| t.as_u64())
        .unwrap_or(0);
    let memories_processed = result
        .get("memories_processed")
        .and_then(|m| m.as_u64())
        .unwrap_or(0);

    println!("Memories Processed: {}", memories_processed);
    println!("Topics Found: {}", topics_found);

    if let Some(topics) = result.get("topics").and_then(|t| t.as_array()) {
        if !topics.is_empty() {
            println!("\nDiscovered Topics:");
            for topic in topics {
                let name = topic
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("Unknown");
                let confidence = topic
                    .get("confidence")
                    .and_then(|c| c.as_f64())
                    .unwrap_or(0.0);
                println!("  - {} (confidence: {:.2})", name, confidence);
            }
        }
    }

    if topics_found == 0 {
        println!("\nNo topics emerged. Possible reasons:");
        println!("  - Fewer than 3 memories in the graph");
        println!("  - Memories don't cluster with weighted_agreement >= 2.5");
        println!("  - Try adding more related content");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topic_format_as_str() {
        assert_eq!(TopicFormat::Brief.as_str(), "brief");
        assert_eq!(TopicFormat::Standard.as_str(), "standard");
        assert_eq!(TopicFormat::Verbose.as_str(), "verbose");
    }

    #[test]
    fn test_portfolio_args_defaults() {
        let args = PortfolioArgs {
            format: TopicFormat::default(),
            json: false,
        };
        assert!(!args.json);
        assert!(matches!(args.format, TopicFormat::Standard));
    }

    #[test]
    fn test_stability_args_validation() {
        let args = StabilityArgs {
            hours: 6,
            json: false,
        };
        assert_eq!(args.hours, 6);
    }

    #[test]
    fn test_format_portfolio_empty() {
        let result = serde_json::json!({
            "topics": []
        });
        let output = format_portfolio(&result, TopicFormat::Standard);
        assert!(output.contains("Topic Portfolio"));
        assert!(output.contains("No topics discovered yet."));
        assert!(output.contains("weighted_agreement >= 2.5"));
    }

    #[test]
    fn test_format_portfolio_with_topics() {
        let result = serde_json::json!({
            "topics": [
                {
                    "name": "Authentication",
                    "confidence": 0.85,
                    "weighted_agreement": 3.2,
                    "contributing_spaces": ["E1", "E7"]
                }
            ]
        });
        let output = format_portfolio(&result, TopicFormat::Standard);
        assert!(output.contains("Topic: Authentication"));
        assert!(output.contains("Confidence: 0.85"));
        assert!(output.contains("Weighted Agreement: 3.20"));
        assert!(output.contains("E1, E7"));
        assert!(output.contains("Total: 1 topics"));
    }

    #[test]
    fn test_format_portfolio_brief() {
        let result = serde_json::json!({
            "topics": [{"name": "Auth", "confidence": 0.9}]
        });
        let output = format_portfolio(&result, TopicFormat::Brief);
        assert!(output.contains("Auth (confidence: 0.90)"));
    }

    #[test]
    fn test_format_stability_dream_recommended() {
        let result = serde_json::json!({
            "churn_rate": 0.6,
            "entropy": 0.8,
            "dream_recommended": true,
            "phases": {
                "emerging": 2,
                "stable": 3,
                "declining": 1
            }
        });
        let output = format_stability(&result);
        assert!(output.contains("Topic Stability Metrics"));
        assert!(output.contains("Churn Rate: 0.60 (Unstable)"));
        assert!(output.contains("Entropy: 0.80 (High)"));
        assert!(output.contains("! High entropy/churn detected"));
        assert!(output.contains("Phase Breakdown:"));
        assert!(output.contains("stable: 3"));
        assert!(output.contains("emerging: 2"));
    }

    #[test]
    fn test_format_stability_healthy() {
        let result = serde_json::json!({
            "churn_rate": 0.1,
            "entropy": 0.3,
            "dream_recommended": false
        });
        let output = format_stability(&result);
        assert!(output.contains("Churn Rate: 0.10 (Healthy)"));
        assert!(output.contains("Entropy: 0.30 (Low)"));
        assert!(output.contains("Topic structure is stable."));
    }
}
