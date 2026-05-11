use super::error::{EvalError, EvalErrorCode};
use super::types::{EvalReport, OpenResearchQuestionStatus};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;

pub fn write_weekly_report(path: impl AsRef<Path>, report: &EvalReport) -> Result<(), EvalError> {
    report.validate()?;
    write_json_0600(path.as_ref(), report)
}

pub fn seed_open_research_questions() -> Vec<OpenResearchQuestionStatus> {
    vec![
        OpenResearchQuestionStatus {
            id: "q1-conformal-cell-calibration".to_string(),
            question: "Which language/category cells need more calibration mass?".to_string(),
            status: "open".to_string(),
        },
        OpenResearchQuestionStatus {
            id: "q2-ood-shift-boundary".to_string(),
            question: "Where does OOD AUC degrade before ship gates trip?".to_string(),
            status: "open".to_string(),
        },
        OpenResearchQuestionStatus {
            id: "q3-aux-head-distillation".to_string(),
            question: "Which auxiliary heads preserve report correlation after distillation?"
                .to_string(),
            status: "open".to_string(),
        },
    ]
}

pub fn write_json_0600(path: &Path, value: &impl serde::Serialize) -> Result<(), EvalError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let bytes = serde_json::to_vec_pretty(value)?;
    #[cfg(unix)]
    let mut file = {
        use std::os::unix::fs::OpenOptionsExt;
        OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(path)?
    };
    #[cfg(not(unix))]
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    let readback = fs::read(path)?;
    if readback != bytes {
        return Err(EvalError::new(
            EvalErrorCode::ReadbackMismatch,
            format!("{} readback bytes differ", path.display()),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(path)?.permissions().mode() & 0o777;
        if mode != 0o600 {
            return Err(EvalError::new(
                EvalErrorCode::ReadbackMismatch,
                format!("{} mode {mode:o} != 600", path.display()),
            ));
        }
    }
    Ok(())
}
