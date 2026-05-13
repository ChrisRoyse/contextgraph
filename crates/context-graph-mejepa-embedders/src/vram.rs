use crate::error::{EmbedError, EmbedResult};
use crate::types::VramBudgetReport;
use context_graph_cuda::safe::GpuDevice;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

pub const GB: u64 = 1024 * 1024 * 1024;
const NVIDIA_SMI_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VramBudget {
    pub required_bytes: u64,
    pub device_ordinal: i32,
}

impl VramBudget {
    pub const fn content_set_rtx5090() -> Self {
        Self {
            required_bytes: 18 * GB,
            device_ordinal: 0,
        }
    }

    pub const fn full_phase1_rtx5090() -> Self {
        Self {
            required_bytes: 29 * GB,
            device_ordinal: 0,
        }
    }
}

pub fn query_vram_budget(budget: VramBudget) -> EmbedResult<VramBudgetReport> {
    let device = GpuDevice::new(budget.device_ordinal).map_err(|err| EmbedError::GpuUnavailable {
        message: format!("{err}"),
        remediation: "fix Windows/WSL CUDA access and verify `nvidia-smi` works in this shell before loading embedders",
    })?;
    let (free, total) = device
        .memory_info()
        .map_err(|err| EmbedError::GpuUnavailable {
            message: format!("{err}"),
            remediation:
                "fix CUDA context creation before running cuMemGetInfo_v2-backed budget checks",
        })?;
    let (major, minor) = device
        .compute_capability()
        .map_err(|err| EmbedError::GpuUnavailable {
            message: format!("{err}"),
            remediation:
                "fix CUDA device-attribute access before running compute-capability checks",
        })?;
    let gpu_name = device.name().map_err(|err| EmbedError::GpuUnavailable {
        message: format!("{err}"),
        remediation: "fix CUDA device-name access before loading embedders",
    })?;
    let nvidia_smi_total = nvidia_smi_total_mb(budget.device_ordinal);
    let nvidia_smi_status = nvidia_smi_status(&nvidia_smi_total);
    let report = VramBudgetReport {
        required_bytes: budget.required_bytes,
        free_bytes: free as u64,
        total_bytes: total as u64,
        gpu_name,
        compute_capability: format!("{major}.{minor}"),
        telemetry_source: "cuda_driver_cuMemGetInfo_v2".to_string(),
        nvidia_smi_status,
        nvidia_smi_total_mb: nvidia_smi_total.ok(),
        passes: (free as u64) >= budget.required_bytes,
    };
    if !report.passes {
        return Err(EmbedError::VramExceeded {
            required_bytes: report.required_bytes,
            free_bytes: report.free_bytes,
            total_bytes: report.total_bytes,
            remediation: "stop other GPU workloads or reduce hard-warmed learner-state embedders before loading the ensemble",
        });
    }
    Ok(report)
}

pub fn nvidia_smi_status(result: &Result<u64, String>) -> String {
    match result {
        Ok(total) => format!("diagnostic_available_total_mb={total}"),
        Err(message) => format!("diagnostic_unavailable_not_authoritative: {message}"),
    }
}

pub fn nvidia_smi_total_mb(device_ordinal: i32) -> Result<u64, String> {
    nvidia_smi_total_mb_from_command("nvidia-smi", device_ordinal)
}

pub fn nvidia_smi_total_mb_from_command(
    command: impl AsRef<Path>,
    device_ordinal: i32,
) -> Result<u64, String> {
    nvidia_smi_total_mb_from_command_with_timeout(command, device_ordinal, NVIDIA_SMI_TIMEOUT)
}

pub fn nvidia_smi_total_mb_from_command_with_timeout(
    command: impl AsRef<Path>,
    device_ordinal: i32,
    timeout: Duration,
) -> Result<u64, String> {
    let command = command.as_ref();
    let ordinal = device_ordinal.to_string();
    let output = run_nvidia_smi_with_timeout(
        command,
        &[
            "--query-gpu=memory.total",
            "--format=csv,noheader,nounits",
            "-i",
            &ordinal,
        ],
        timeout,
    )?;
    if !output.status.success() {
        return Err(format!(
            "{} exited with status {:?}: {}",
            command.display(),
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|err| format!("nvidia-smi stdout was not UTF-8: {err}"))?;
    parse_nvidia_smi_memory_total_mb(&stdout)
}

fn run_nvidia_smi_with_timeout(
    command: &Path,
    args: &[&str],
    timeout: Duration,
) -> Result<Output, String> {
    if timeout.is_zero() {
        return Err("nvidia-smi timeout must be greater than zero".to_string());
    }
    let mut child = Command::new(command)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("failed to execute {}: {err}", command.display()))?;
    let deadline = Instant::now() + timeout;
    loop {
        if child
            .try_wait()
            .map_err(|err| format!("failed to poll {}: {err}", command.display()))?
            .is_some()
        {
            return child
                .wait_with_output()
                .map_err(|err| format!("failed to collect {} output: {err}", command.display()));
        }
        if Instant::now() >= deadline {
            let kill_result = child.kill();
            let _ = child.wait_with_output();
            return match kill_result {
                Ok(()) => Err(format!(
                    "{} timed out after {} ms and was killed",
                    command.display(),
                    timeout.as_millis()
                )),
                Err(err) => Err(format!(
                    "{} timed out after {} ms and kill failed: {err}",
                    command.display(),
                    timeout.as_millis()
                )),
            };
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

pub fn parse_nvidia_smi_memory_total_mb(stdout: &str) -> Result<u64, String> {
    let first = stdout
        .lines()
        .next()
        .ok_or_else(|| "nvidia-smi returned no rows".to_string())?
        .trim();
    let normalized = first
        .trim_matches(['[', ']'])
        .trim()
        .to_ascii_lowercase()
        .replace('-', " ");
    if first.is_empty()
        || normalized == "not supported"
        || normalized == "unsupported"
        || normalized == "n/a"
        || normalized == "na"
    {
        return Err(format!(
            "nvidia-smi returned unsupported memory.total row {first:?}"
        ));
    }
    first
        .parse::<u64>()
        .map_err(|err| format!("failed to parse nvidia-smi memory.total row {first:?}: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nvidia_smi_memory_total() {
        assert_eq!(parse_nvidia_smi_memory_total_mb("32607\n").unwrap(), 32607);
    }

    #[test]
    fn rejects_wsl_unsupported_sentinels() {
        for row in [
            "N/A\n",
            "[Not Supported]\n",
            "Not-Supported\n",
            "unsupported\n",
        ] {
            let err = parse_nvidia_smi_memory_total_mb(row).unwrap_err();
            assert!(err.contains("unsupported memory.total row"));
        }
    }

    #[test]
    fn rejects_malformed_nvidia_smi_memory_total() {
        let err = parse_nvidia_smi_memory_total_mb("32607 MiB\n").unwrap_err();
        assert!(err.contains("failed to parse nvidia-smi memory.total row"));
    }

    #[test]
    fn diagnostic_status_marks_nvidia_smi_non_authoritative() {
        let status = nvidia_smi_status(&Err("failed to execute nvidia-smi".to_string()));
        assert!(status.starts_with("diagnostic_unavailable_not_authoritative"));
    }

    #[cfg(unix)]
    #[test]
    fn times_out_hung_nvidia_smi_command() {
        use std::io::Write;
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().unwrap();
        let command = root.path().join("nvidia-smi-hang");
        {
            let mut file = std::fs::File::create(&command).unwrap();
            writeln!(file, "#!/usr/bin/env bash").unwrap();
            writeln!(file, "exec sleep 5").unwrap();
            file.sync_all().unwrap();
        }
        std::fs::set_permissions(&command, std::fs::Permissions::from_mode(0o700)).unwrap();
        let err =
            nvidia_smi_total_mb_from_command_with_timeout(&command, 0, Duration::from_millis(100))
                .unwrap_err();
        assert!(err.contains("timed out"));
    }
}
