use std::collections::BTreeMap;

use serde_json::Value;

use super::{AccuracyMeasurement, Q4AccuracyMetricKind};

pub(super) fn parse_accuracy_measurement_output(
    raw: &str,
) -> Result<BTreeMap<String, AccuracyMeasurement>, String> {
    if let Ok(value) = serde_json::from_str::<Value>(raw) {
        let rows = parse_json_metrics(&value)?;
        if !rows.is_empty() {
            return Ok(rows);
        }
    }
    parse_text_metrics(raw)
}

fn parse_json_metrics(value: &Value) -> Result<BTreeMap<String, AccuracyMeasurement>, String> {
    if let Some(metrics) = value.get("metrics").and_then(Value::as_array) {
        let mut out = BTreeMap::new();
        for item in metrics {
            let name = item
                .get("metric")
                .or_else(|| item.get("metric_name"))
                .and_then(Value::as_str)
                .ok_or_else(|| "accuracy metric missing metric name".to_string())?;
            let metric_kind = parse_metric_kind(name);
            let value = number_field(item, "value")
                .or_else(|| number_field(item, "score"))
                .or_else(|| number_field(item, "actual"))
                .ok_or_else(|| format!("accuracy metric {name} missing value/score"))?;
            let metric_name = canonical_metric_name(name, metric_kind);
            let source_test = item
                .get("source_test")
                .or_else(|| item.get("test"))
                .and_then(Value::as_str)
                .map(ToString::to_string);
            insert_measurement(
                &mut out,
                AccuracyMeasurement {
                    metric_name,
                    value,
                    metric_kind,
                    source_test,
                    stddev: number_field(item, "stddev").or_else(|| number_field(item, "stdev")),
                },
            )?;
        }
        return Ok(out);
    }
    if let Some(metrics) = value.get("metrics").and_then(Value::as_object) {
        let mut out = BTreeMap::new();
        for (name, value) in metrics {
            let Some(metric_value) = value.as_f64().filter(|raw| raw.is_finite()) else {
                continue;
            };
            let metric_kind = parse_metric_kind(name);
            insert_measurement(
                &mut out,
                AccuracyMeasurement {
                    metric_name: canonical_metric_name(name, metric_kind),
                    value: metric_value,
                    metric_kind,
                    source_test: None,
                    stddev: None,
                },
            )?;
        }
        return Ok(out);
    }
    let mut out = BTreeMap::new();
    for (name, value) in value
        .as_object()
        .into_iter()
        .flat_map(|object| object.iter())
    {
        let metric_kind = parse_metric_kind(name);
        if metric_kind == Q4AccuracyMetricKind::Other {
            continue;
        }
        let Some(metric_value) = value.as_f64().filter(|raw| raw.is_finite()) else {
            continue;
        };
        insert_measurement(
            &mut out,
            AccuracyMeasurement {
                metric_name: canonical_metric_name(name, metric_kind),
                value: metric_value,
                metric_kind,
                source_test: None,
                stddev: None,
            },
        )?;
    }
    Ok(out)
}

fn parse_text_metrics(raw: &str) -> Result<BTreeMap<String, AccuracyMeasurement>, String> {
    let mut out = BTreeMap::new();
    let mut saw_metric_alias = false;
    for line in raw.lines() {
        let lower = line.to_ascii_lowercase();
        for (alias, metric_kind) in metric_aliases() {
            let Some(position) = lower.find(alias) else {
                continue;
            };
            saw_metric_alias = true;
            let tail = &line[position + alias.len()..];
            let value = first_number(tail)
                .ok_or_else(|| format!("metric alias {alias} did not have a numeric value"))?;
            insert_measurement(
                &mut out,
                AccuracyMeasurement {
                    metric_name: canonical_metric_name(alias, *metric_kind),
                    value,
                    metric_kind: *metric_kind,
                    source_test: parse_source_test(line),
                    stddev: None,
                },
            )?;
            break;
        }
    }
    if out.is_empty() && saw_metric_alias {
        return Err("accuracy-like output mentioned a metric but no numeric value".to_string());
    }
    Ok(out)
}

fn metric_aliases() -> &'static [(&'static str, Q4AccuracyMetricKind)] {
    &[
        ("accuracy_score", Q4AccuracyMetricKind::Accuracy),
        ("accuracy", Q4AccuracyMetricKind::Accuracy),
        ("f1_score", Q4AccuracyMetricKind::F1),
        ("f1", Q4AccuracyMetricKind::F1),
        ("precision_score", Q4AccuracyMetricKind::Precision),
        ("precision", Q4AccuracyMetricKind::Precision),
        ("recall_score", Q4AccuracyMetricKind::Recall),
        ("recall", Q4AccuracyMetricKind::Recall),
        ("roc_auc", Q4AccuracyMetricKind::Auc),
        ("auc", Q4AccuracyMetricKind::Auc),
        ("mean_squared_error", Q4AccuracyMetricKind::MeanSquaredError),
        ("mse", Q4AccuracyMetricKind::MeanSquaredError),
        (
            "mean_absolute_error",
            Q4AccuracyMetricKind::MeanAbsoluteError,
        ),
        ("mae", Q4AccuracyMetricKind::MeanAbsoluteError),
        ("rouge-l", Q4AccuracyMetricKind::Rouge),
        ("rougel", Q4AccuracyMetricKind::Rouge),
        ("rouge", Q4AccuracyMetricKind::Rouge),
        ("cross_entropy", Q4AccuracyMetricKind::CrossEntropy),
        ("cross entropy", Q4AccuracyMetricKind::CrossEntropy),
        ("log_loss", Q4AccuracyMetricKind::LogLoss),
        ("logloss", Q4AccuracyMetricKind::LogLoss),
        ("loss", Q4AccuracyMetricKind::Loss),
        ("brier_score", Q4AccuracyMetricKind::BrierScore),
        ("calibration_error", Q4AccuracyMetricKind::CalibrationError),
        ("perplexity", Q4AccuracyMetricKind::Perplexity),
        ("r2_score", Q4AccuracyMetricKind::R2),
        ("r2", Q4AccuracyMetricKind::R2),
    ]
}

fn parse_metric_kind(name: &str) -> Q4AccuracyMetricKind {
    let lower = name.to_ascii_lowercase().replace(' ', "_");
    metric_aliases()
        .iter()
        .find(|(alias, _)| lower.contains(alias))
        .map(|(_, kind)| *kind)
        .unwrap_or(Q4AccuracyMetricKind::Other)
}

fn canonical_metric_name(name: &str, kind: Q4AccuracyMetricKind) -> String {
    if kind == Q4AccuracyMetricKind::Other {
        name.trim().to_ascii_lowercase().replace(' ', "_")
    } else {
        kind.as_str().to_string()
    }
}

fn insert_measurement(
    out: &mut BTreeMap<String, AccuracyMeasurement>,
    measurement: AccuracyMeasurement,
) -> Result<(), String> {
    let key = measurement_key(&measurement.metric_name, measurement.source_test.as_deref());
    if out.insert(key.clone(), measurement).is_some() {
        return Err(format!("duplicate accuracy metric/source_test key {key}"));
    }
    Ok(())
}

fn measurement_key(metric_name: &str, source_test: Option<&str>) -> String {
    match source_test {
        Some(source_test) if !source_test.trim().is_empty() => {
            format!("{metric_name}:source_test:{source_test}")
        }
        _ => metric_name.to_string(),
    }
}

fn number_field(value: &Value, field: &str) -> Option<f64> {
    value
        .get(field)
        .and_then(Value::as_f64)
        .filter(|raw| raw.is_finite())
}

fn first_number(raw: &str) -> Option<f64> {
    let bytes = raw.as_bytes();
    let mut start = None;
    for (idx, byte) in bytes.iter().enumerate() {
        if byte.is_ascii_digit() || *byte == b'-' || *byte == b'+' || *byte == b'.' {
            start = Some(idx);
            break;
        }
    }
    let start = start?;
    let mut end = start;
    while end < bytes.len()
        && (bytes[end].is_ascii_digit() || matches!(bytes[end], b'.' | b'-' | b'+' | b'e' | b'E'))
    {
        end += 1;
    }
    raw[start..end]
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite())
}

fn parse_source_test(line: &str) -> Option<String> {
    line.split_whitespace()
        .find(|token| token.contains("::") || token.ends_with(".py"))
        .map(|token| {
            token
                .trim_matches(|ch: char| ch == ':' || ch == ',')
                .to_string()
        })
}
