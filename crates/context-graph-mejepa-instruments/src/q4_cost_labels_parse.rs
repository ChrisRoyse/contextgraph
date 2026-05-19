use std::collections::BTreeMap;

use serde_json::Value;

use super::{CostMeasurement, Q4CostKind};

pub(super) fn parse_cost_measurements(
    text: &str,
    command: &[String],
) -> Result<BTreeMap<Q4CostKind, CostMeasurement>, String> {
    let mut out = BTreeMap::new();
    let trimmed = text.trim();
    if looks_like_json(trimmed) {
        let value: Value = serde_json::from_str(trimmed).map_err(|err| err.to_string())?;
        parse_top_level(&value, &mut out)?;
        if let Some(metrics) = value.get("metrics").and_then(Value::as_array) {
            for metric in metrics {
                parse_metric_row(metric, &mut out)?;
            }
        }
        if let Some(cost) = value.get("cost").and_then(Value::as_object) {
            parse_top_level(&Value::Object(cost.clone()), &mut out)?;
        }
        return Ok(out);
    }
    parse_pytest_duration(trimmed, &mut out)?;
    parse_requirements_text(trimmed, command, &mut out)?;
    parse_wheel_metadata(trimmed, command, &mut out)?;
    if out.is_empty() {
        return Err("no supported cost metrics found in analyzer output".to_string());
    }
    Ok(out)
}

fn parse_top_level(
    value: &Value,
    out: &mut BTreeMap<Q4CostKind, CostMeasurement>,
) -> Result<(), String> {
    if let Some(minutes) = number_at_any(value, &["ci_minutes", "ci_min", "ci_wall_minutes"]) {
        insert(
            out,
            Q4CostKind::CiMinutes,
            minutes,
            number_at_any(
                value,
                &["ci_minutes_stddev", "ci_stddev_minutes", "stddev_minutes"],
            ),
        )?;
    }
    if let Some(seconds) = number_at_any(
        value,
        &[
            "ci_seconds",
            "ci_wall_seconds",
            "wall_seconds",
            "pytest_wall_seconds",
            "duration_seconds",
        ],
    ) {
        insert(
            out,
            Q4CostKind::CiMinutes,
            seconds / 60.0,
            number_at_any(
                value,
                &["ci_seconds_stddev", "wall_seconds_stddev", "stddev_seconds"],
            )
            .map(|stddev| stddev / 60.0),
        )?;
    }
    if let Some(count) = number_at_any(
        value,
        &[
            "dependency_count",
            "dep_count",
            "requirements_count",
            "new_dependency_count",
        ],
    ) {
        insert(out, Q4CostKind::DependencyCount, count, None)?;
    } else if let Some(count) = array_len_at_any(value, &["dependencies", "requirements"]) {
        insert(out, Q4CostKind::DependencyCount, count as f64, None)?;
    }
    if let Some(bytes) = number_at_any(
        value,
        &[
            "wheel_bytes",
            "bundle_bytes",
            "binary_bytes",
            "storage_bytes",
            "artifact_bytes",
        ],
    ) {
        insert(out, Q4CostKind::WheelBytes, bytes, None)?;
    }
    Ok(())
}

fn parse_metric_row(
    value: &Value,
    out: &mut BTreeMap<Q4CostKind, CostMeasurement>,
) -> Result<(), String> {
    let raw_kind = value
        .get("kind")
        .or_else(|| value.get("metric"))
        .or_else(|| value.get("name"))
        .and_then(Value::as_str)
        .ok_or_else(|| "cost metric row missing kind/metric/name".to_string())?;
    let (kind, scale) =
        parse_kind(raw_kind).ok_or_else(|| format!("unsupported cost metric kind: {raw_kind}"))?;
    let measured = number_at_any(
        value,
        &["value", "mean", "count", "bytes", "minutes", "seconds"],
    )
    .ok_or_else(|| format!("cost metric {raw_kind} missing numeric value"))?;
    let stddev = number_at_any(
        value,
        &[
            "stddev",
            "stddev_value",
            "stddev_minutes",
            "stddev_seconds",
            "ci_stddev",
        ],
    )
    .map(|stddev| stddev * scale);
    insert(out, kind, measured * scale, stddev)
}

fn parse_pytest_duration(
    text: &str,
    out: &mut BTreeMap<Q4CostKind, CostMeasurement>,
) -> Result<(), String> {
    if let Some(seconds) = pytest_summary_seconds(text) {
        insert(out, Q4CostKind::CiMinutes, seconds / 60.0, None)?;
        return Ok(());
    }
    let mut summed = 0.0;
    let mut found = false;
    for line in text.lines() {
        let line = line.trim();
        let Some((raw_seconds, _)) = line.split_once('s') else {
            continue;
        };
        let Ok(seconds) = raw_seconds.trim().parse::<f64>() else {
            continue;
        };
        if line.contains(" call ")
            || line.contains(" setup ")
            || line.contains(" teardown ")
            || line.contains(" total ")
        {
            summed += seconds;
            found = true;
        }
    }
    if found {
        insert(out, Q4CostKind::CiMinutes, summed / 60.0, None)?;
    }
    Ok(())
}

fn pytest_summary_seconds(text: &str) -> Option<f64> {
    for line in text.lines().rev() {
        let lower = line.to_ascii_lowercase();
        if !lower.contains(" in ") {
            continue;
        }
        for segment in lower.rsplit(" in ") {
            let token = segment.split_whitespace().next()?;
            if let Some(seconds) = parse_duration_seconds(token) {
                return Some(seconds);
            }
        }
    }
    None
}

fn parse_duration_seconds(token: &str) -> Option<f64> {
    let token = token.trim_matches(|ch: char| ch == '=' || ch == ')' || ch == '(');
    if let Some(raw) = token.strip_suffix('s') {
        return raw.parse::<f64>().ok();
    }
    let parts = token.split(':').collect::<Vec<_>>();
    if parts.len() == 2 {
        let minutes = parts[0].parse::<f64>().ok()?;
        let seconds = parts[1].parse::<f64>().ok()?;
        return Some(minutes * 60.0 + seconds);
    }
    None
}

fn parse_requirements_text(
    text: &str,
    command: &[String],
    out: &mut BTreeMap<Q4CostKind, CostMeasurement>,
) -> Result<(), String> {
    if out.contains_key(&Q4CostKind::DependencyCount) {
        return Ok(());
    }
    let command_hint = command
        .iter()
        .any(|part| has_requirement_hint(&part.to_ascii_lowercase()));
    let has_pinned_requirements = text.lines().any(has_requirement_operator);
    let count = text
        .lines()
        .filter(|line| {
            let normalized = normalize_requirement_line(line);
            if has_pinned_requirements {
                has_requirement_operator(&normalized)
            } else {
                command_hint && is_requirement_line(&normalized.as_str())
            }
        })
        .count();
    if count > 0 && (command_hint || has_pinned_requirements) {
        insert(out, Q4CostKind::DependencyCount, count as f64, None)?;
    }
    Ok(())
}

fn parse_wheel_metadata(
    text: &str,
    command: &[String],
    out: &mut BTreeMap<Q4CostKind, CostMeasurement>,
) -> Result<(), String> {
    if out.contains_key(&Q4CostKind::WheelBytes) {
        return Ok(());
    }
    for line in text.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("wheel_bytes")
            || lower.contains("artifact_bytes")
            || lower.contains("binary_bytes")
            || lower.contains(".whl")
        {
            if let Some(bytes) = largest_integer(line) {
                insert(out, Q4CostKind::WheelBytes, bytes as f64, None)?;
                return Ok(());
            }
        }
    }
    let command_hint = command.iter().any(|part| {
        let part = part.to_ascii_lowercase();
        part.contains("wheel") || part.contains(".whl") || part.contains("dist/")
    });
    if command_hint {
        if let Some(bytes) = text.lines().find_map(largest_integer) {
            insert(out, Q4CostKind::WheelBytes, bytes as f64, None)?;
        }
    }
    Ok(())
}

fn looks_like_json(text: &str) -> bool {
    text.starts_with('{') || text.starts_with('[')
}

fn has_requirement_hint(value: &str) -> bool {
    value.contains("requirements") || value.contains("pyproject.toml") || value.contains("pip")
}

fn is_requirement_line(line: &&str) -> bool {
    let line = normalize_requirement_line(line);
    if line.is_empty()
        || line.starts_with('#')
        || line.starts_with("---")
        || line.starts_with("+++")
        || line.starts_with("@@")
    {
        return false;
    }
    has_requirement_operator(&line) || bare_package_name(&line)
}

fn normalize_requirement_line(line: &str) -> String {
    line.trim()
        .trim_start_matches('+')
        .trim_start_matches('-')
        .trim()
        .to_string()
}

fn has_requirement_operator(line: impl AsRef<str>) -> bool {
    let line = normalize_requirement_line(line.as_ref()).to_ascii_lowercase();
    if line.starts_with('=')
        || line.contains(" passed in ")
        || line.contains("test session")
        || line.contains("requirements_start")
        || line.contains("requirements_end")
    {
        return false;
    }
    ["==", ">=", "<=", "~=", "!=", " @ "]
        .iter()
        .any(|operator| {
            line.find(operator)
                .map(|idx| looks_like_package_prefix(&line[..idx]))
                .unwrap_or(false)
        })
}

fn bare_package_name(line: &str) -> bool {
    let line = line.trim();
    !line.is_empty()
        && line.len() <= 128
        && line
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
        && line.chars().any(|ch| ch.is_ascii_alphabetic())
}

fn looks_like_package_prefix(prefix: &str) -> bool {
    let prefix = prefix.trim();
    !prefix.is_empty()
        && prefix
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
        && prefix.chars().any(|ch| ch.is_ascii_alphabetic())
}

fn largest_integer(line: &str) -> Option<u64> {
    let mut best = None;
    let mut current = String::new();
    for ch in line.chars().chain(std::iter::once(' ')) {
        if ch.is_ascii_digit() {
            current.push(ch);
        } else if !current.is_empty() {
            if let Ok(value) = current.parse::<u64>() {
                best = Some(best.map_or(value, |prev: u64| prev.max(value)));
            }
            current.clear();
        }
    }
    best
}

fn parse_kind(raw: &str) -> Option<(Q4CostKind, f64)> {
    let key = raw.trim().to_ascii_lowercase().replace(['-', ' '], "_");
    match key.as_str() {
        "ci_minutes" | "ci_min" | "ci_wall_minutes" | "wall_minutes" => {
            Some((Q4CostKind::CiMinutes, 1.0))
        }
        "ci_seconds"
        | "ci_wall_seconds"
        | "wall_seconds"
        | "pytest_wall_seconds"
        | "duration_seconds" => Some((Q4CostKind::CiMinutes, 1.0 / 60.0)),
        "dependency_count" | "dep_count" | "requirements_count" | "dependencies" => {
            Some((Q4CostKind::DependencyCount, 1.0))
        }
        "wheel_bytes" | "bundle_bytes" | "binary_bytes" | "storage_bytes" | "artifact_bytes" => {
            Some((Q4CostKind::WheelBytes, 1.0))
        }
        _ => None,
    }
}

fn insert(
    out: &mut BTreeMap<Q4CostKind, CostMeasurement>,
    kind: Q4CostKind,
    value: f64,
    stddev: Option<f64>,
) -> Result<(), String> {
    if !value.is_finite() {
        return Err(format!("{} value is not finite", kind.as_str()));
    }
    if stddev.map(|v| !v.is_finite()).unwrap_or(false) {
        return Err(format!("{} stddev is not finite", kind.as_str()));
    }
    if out.contains_key(&kind) {
        return Err(format!("duplicate {} cost metric", kind.as_str()));
    }
    out.insert(
        kind,
        CostMeasurement {
            kind,
            value,
            stddev,
        },
    );
    Ok(())
}

fn number_at_any(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(number_value))
}

fn number_value(value: &Value) -> Option<f64> {
    if let Some(number) = value.as_f64() {
        Some(number)
    } else if let Some(text) = value.as_str() {
        text.parse::<f64>().ok()
    } else {
        None
    }
}

fn array_len_at_any(value: &Value, keys: &[&str]) -> Option<usize> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_array).map(Vec::len))
}
