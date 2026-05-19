use std::collections::BTreeMap;

use serde_json::Value;

use super::{PerfMeasurement, Q4PerfCategory, Q4PerfToolKind};

pub(super) fn parse_measurement_json(
    raw: &str,
    tool: Q4PerfToolKind,
) -> Result<BTreeMap<String, PerfMeasurement>, String> {
    let value: Value =
        serde_json::from_str(raw).map_err(|err| format!("perf JSON parse failed: {err}"))?;
    match tool {
        Q4PerfToolKind::PytestBenchmark => parse_pytest_benchmark_json(&value),
        Q4PerfToolKind::CProfile => parse_cprofile_json(&value),
    }
}

fn parse_pytest_benchmark_json(value: &Value) -> Result<BTreeMap<String, PerfMeasurement>, String> {
    let mut out = parse_custom_metrics(value, Q4PerfCategory::WallclockMs)?;
    if !out.is_empty() {
        return Ok(out);
    }
    let benchmarks = value
        .get("benchmarks")
        .and_then(Value::as_array)
        .ok_or_else(|| "pytest-benchmark JSON missing benchmarks array".to_string())?;
    for item in benchmarks {
        let name = item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("benchmark");
        let stats = item.get("stats").unwrap_or(&Value::Null);
        let mean_s = number_field(stats, "mean")
            .ok_or_else(|| format!("benchmark {name} missing stats.mean"))?;
        let stddev_s = number_field(stats, "stddev");
        out.insert(
            name.to_string(),
            PerfMeasurement {
                value_ns: mean_s * 1_000_000_000.0,
                stddev_ns: stddev_s.map(|value| value * 1_000_000_000.0),
                category: Q4PerfCategory::WallclockMs,
            },
        );
    }
    Ok(out)
}

fn parse_cprofile_json(value: &Value) -> Result<BTreeMap<String, PerfMeasurement>, String> {
    let custom = parse_custom_metrics(value, Q4PerfCategory::CpuMs)?;
    if !custom.is_empty() {
        return Ok(custom);
    }
    let Some(total_time_s) = number_field(value, "total_time_s") else {
        return Ok(BTreeMap::new());
    };
    Ok(BTreeMap::from([(
        "cprofile_total_time".to_string(),
        PerfMeasurement {
            value_ns: total_time_s * 1_000_000_000.0,
            stddev_ns: None,
            category: Q4PerfCategory::CpuMs,
        },
    )]))
}

fn parse_custom_metrics(
    value: &Value,
    default_category: Q4PerfCategory,
) -> Result<BTreeMap<String, PerfMeasurement>, String> {
    let mut out = BTreeMap::new();
    let Some(metrics) = value.get("metrics").and_then(Value::as_array) else {
        return Ok(out);
    };
    for metric in metrics {
        let name = metric
            .get("metric")
            .and_then(Value::as_str)
            .ok_or_else(|| "perf metric missing metric name".to_string())?;
        let value_ns = number_field(metric, "value_ns")
            .or_else(|| number_field(metric, "mean_ns"))
            .ok_or_else(|| format!("perf metric {name} missing value_ns/mean_ns"))?;
        out.insert(
            name.to_string(),
            PerfMeasurement {
                value_ns,
                stddev_ns: number_field(metric, "stddev_ns"),
                category: parse_category(metric).unwrap_or(default_category),
            },
        );
    }
    Ok(out)
}

fn parse_category(value: &Value) -> Option<Q4PerfCategory> {
    match value.get("category").and_then(Value::as_str)? {
        "cpu_ms" => Some(Q4PerfCategory::CpuMs),
        "wallclock_ms" => Some(Q4PerfCategory::WallclockMs),
        "alloc_count" => Some(Q4PerfCategory::AllocCount),
        "rss_kb" => Some(Q4PerfCategory::RssKb),
        "wallclock_budget_exceeded" => Some(Q4PerfCategory::WallclockBudgetExceeded),
        "improvement" => Some(Q4PerfCategory::Improvement),
        _ => None,
    }
}

fn number_field(value: &Value, field: &str) -> Option<f64> {
    value
        .get(field)
        .and_then(Value::as_f64)
        .filter(|raw| raw.is_finite())
}
