//! Divergence detection commands
//!
//! Commands for checking divergence from recent activity patterns.
//!
//! # Commands
//!
//! - `divergence check`: Check for divergence alerts
//!
//! # Constitution Compliance
//!
//! - ARCH-10: Divergence detection uses SEMANTIC embedders only (E1,E5-E7,E10,E12,E13)
//! - AP-62: Divergence alerts MUST only use SEMANTIC embedders
//! - AP-63: Temporal embedders (E2-E4) are EXCLUDED from divergence detection
//! - AP-26: Exit code 1 on error, 2 on corruption

use clap::{Args, Subcommand};
use tracing::{error, info};

use crate::mcp_client::McpClient;

/// Divergence detection subcommands.
///
/// These commands check for divergence from recent activity using SEMANTIC
/// embedders only. Temporal embedders (E2-E4) are excluded per constitution.
#[derive(Subcommand)]
pub enum DivergenceCommands {
    /// Check for divergence from recent activity
    ///
    /// Detects when current work diverges from recent patterns using only
    /// SEMANTIC embedders (E1, E5, E6, E7, E10, E12, E13). Temporal embedders
    /// are excluded per AP-62, AP-63.
    ///
    /// # Examples
    ///
    /// ```bash
    /// # Default 2-hour lookback
    /// context-graph-cli divergence check
    ///
    /// # Custom lookback period
    /// context-graph-cli divergence check --hours 4
    ///
    /// # JSON output
    /// context-graph-cli divergence check --json
    /// ```
    Check(CheckArgs),
}

/// Arguments for divergence check command.
#[derive(Args)]
pub struct CheckArgs {
    /// Lookback period in hours (1-48)
    #[arg(short = 'H', long, default_value = "2")]
    pub hours: u32,

    /// Output as JSON instead of human-readable
    #[arg(long)]
    pub json: bool,

    /// Show detailed embedder-level divergence scores
    #[arg(short, long)]
    pub verbose: bool,
}

/// Handle divergence subcommands.
///
/// Routes to appropriate handler based on subcommand.
/// Returns exit code per AP-26: 0=success, 1=error, 2=corruption.
pub async fn handle_divergence_command(cmd: DivergenceCommands) -> i32 {
    match cmd {
        DivergenceCommands::Check(args) => handle_check(args).await,
    }
}

/// Handle divergence check command.
async fn handle_check(args: CheckArgs) -> i32 {
    // CLI-M1 FIX: Validate hours range (1-48) before any server interaction.
    // Documented range is 1-48 (daily). Reject 0 and values > 48.
    if args.hours == 0 || args.hours > 48 {
        eprintln!(
            "Error: --hours must be between 1 and 48, got {}",
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
    match client.get_divergence_alerts(Some(args.hours)).await {
        Ok(result) => {
            if args.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result).unwrap_or_default()
                );
            } else {
                print_divergence(&result, args.verbose);
            }
            info!("Divergence check completed successfully");
            0
        }
        Err(e) => {
            error!("Failed to check divergence: {}", e);
            eprintln!("Error: {}", e);
            1
        }
    }
}

/// Format divergence alerts as human-readable string.
///
/// TST-M4 FIX: Extracted from `print_divergence` so tests can assert on content.
fn format_divergence(result: &serde_json::Value, verbose: bool) -> String {
    use std::fmt::Write;
    let mut out = String::new();

    writeln!(out, "Divergence Analysis").unwrap();
    writeln!(out, "===================\n").unwrap();

    // Get overall divergence status
    let is_divergent = result
        .get("is_divergent")
        .and_then(|d| d.as_bool())
        .unwrap_or(false);
    let divergence_score = result
        .get("divergence_score")
        .and_then(|s| s.as_f64())
        .unwrap_or(0.0);
    let lookback_hours = result
        .get("lookback_hours")
        .and_then(|h| h.as_u64())
        .unwrap_or(2);

    writeln!(out, "Lookback Period: {} hours", lookback_hours).unwrap();
    writeln!(out, "Divergence Score: {:.2}", divergence_score).unwrap();
    writeln!(out).unwrap();

    if is_divergent {
        writeln!(out, "STATUS: DIVERGENT").unwrap();
        writeln!(out, "Current activity differs from recent patterns.").unwrap();
        writeln!(out).unwrap();

        // Show alerts if any
        if let Some(alerts) = result.get("alerts").and_then(|a| a.as_array()) {
            if !alerts.is_empty() {
                writeln!(out, "Alerts:").unwrap();
                for alert in alerts {
                    let msg = alert
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("Unknown alert");
                    let severity = alert
                        .get("severity")
                        .and_then(|s| s.as_str())
                        .unwrap_or("info");
                    let icon = match severity {
                        "high" | "critical" => "!!",
                        "medium" | "warning" => "!",
                        _ => "-",
                    };
                    writeln!(out, "  {} {}", icon, msg).unwrap();
                }
                writeln!(out).unwrap();
            }
        }

        // Verbose: show per-embedder scores
        if verbose {
            if let Some(scores) = result
                .get("embedder_divergences")
                .and_then(|s| s.as_object())
            {
                writeln!(out, "Embedder-Level Divergence (SEMANTIC only):").unwrap();
                // Sort by divergence score descending
                let mut score_vec: Vec<_> = scores.iter().collect();
                score_vec.sort_by(|a, b| {
                    let a_val = a.1.as_f64().unwrap_or(0.0);
                    let b_val = b.1.as_f64().unwrap_or(0.0);
                    b_val.partial_cmp(&a_val).unwrap()
                });

                for (embedder, score) in score_vec {
                    let val = score.as_f64().unwrap_or(0.0);
                    let bar_len = (val * 20.0) as usize;
                    let bar = "#".repeat(bar_len.min(20));
                    writeln!(out, "  {:15} {:.3} |{}|", embedder, val, bar).unwrap();
                }
                writeln!(out).unwrap();
            }
        }

        writeln!(
            out,
            "Note: Divergence may be intentional (new topic exploration)"
        )
        .unwrap();
        writeln!(
            out,
            "      or indicate context switch that should be captured."
        )
        .unwrap();
    } else {
        writeln!(out, "STATUS: ALIGNED").unwrap();
        writeln!(out, "Current activity aligns with recent patterns.").unwrap();

        if verbose {
            // Show per-embedder scores even when aligned
            if let Some(scores) = result
                .get("embedder_divergences")
                .and_then(|s| s.as_object())
            {
                writeln!(out, "\nEmbedder-Level Divergence (SEMANTIC only):").unwrap();
                for (embedder, score) in scores {
                    let val = score.as_f64().unwrap_or(0.0);
                    writeln!(out, "  {:15} {:.3}", embedder, val).unwrap();
                }
            }
        }
    }

    // Note about temporal exclusion
    writeln!(out).unwrap();
    writeln!(out, "(Temporal embedders E2-E4 excluded per AP-62, AP-63)").unwrap();

    out
}

/// Print divergence alerts in human-readable format.
fn print_divergence(result: &serde_json::Value, verbose: bool) {
    print!("{}", format_divergence(result, verbose));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_args_defaults() {
        let args = CheckArgs {
            hours: 2,
            json: false,
            verbose: false,
        };
        assert_eq!(args.hours, 2);
        assert!(!args.json);
        assert!(!args.verbose);
    }

    #[test]
    fn test_format_divergence_aligned() {
        let result = serde_json::json!({
            "is_divergent": false,
            "divergence_score": 0.15,
            "lookback_hours": 2,
            "alerts": []
        });
        let output = format_divergence(&result, false);
        assert!(output.contains("Divergence Analysis"));
        assert!(output.contains("Lookback Period: 2 hours"));
        assert!(output.contains("Divergence Score: 0.15"));
        assert!(output.contains("STATUS: ALIGNED"));
        assert!(output.contains("Current activity aligns with recent patterns."));
        assert!(output.contains("Temporal embedders E2-E4 excluded"));
    }

    #[test]
    fn test_format_divergence_divergent_verbose() {
        let result = serde_json::json!({
            "is_divergent": true,
            "divergence_score": 0.75,
            "lookback_hours": 2,
            "alerts": [
                {"message": "Significant topic shift detected", "severity": "high"},
                {"message": "Code patterns differ from recent work", "severity": "medium"}
            ],
            "embedder_divergences": {
                "E1_semantic": 0.65,
                "E5_causal": 0.45,
                "E7_code": 0.85,
                "E10_multimodal": 0.30
            }
        });
        let output = format_divergence(&result, true);
        assert!(output.contains("STATUS: DIVERGENT"));
        assert!(output.contains("Divergence Score: 0.75"));
        assert!(output.contains("Alerts:"));
        // High severity gets "!!" prefix
        assert!(output.contains("!! Significant topic shift detected"));
        // Medium severity gets "!" prefix
        assert!(output.contains("! Code patterns differ from recent work"));
        // Verbose shows per-embedder scores
        assert!(output.contains("Embedder-Level Divergence (SEMANTIC only):"));
        assert!(output.contains("E7_code"));
        assert!(output.contains("0.850"));
        assert!(output.contains("E1_semantic"));
        assert!(output.contains("0.650"));
    }

    #[test]
    fn test_format_divergence_non_verbose_hides_embedders() {
        let result = serde_json::json!({
            "is_divergent": true,
            "divergence_score": 0.75,
            "lookback_hours": 4,
            "alerts": [],
            "embedder_divergences": {
                "E1_semantic": 0.65
            }
        });
        let output = format_divergence(&result, false);
        assert!(output.contains("STATUS: DIVERGENT"));
        // Non-verbose should NOT show embedder-level scores
        assert!(!output.contains("Embedder-Level Divergence"));
    }
}
