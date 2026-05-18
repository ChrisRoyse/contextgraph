use crate::cli::write_json_0600;
use crate::gates::sha256_bytes;
use crate::synthetic_stress::{
    synthetic_stress_invalid, SyntheticExpectedVerdict, SyntheticStressCase, SyntheticStressError,
    SyntheticStressKind, SYNTHETIC_STRESS_SCHEMA_VERSION,
};
use crate::synthetic_stress_cases::synthetic_stress_templates;
use std::fs;
use std::path::Path;

pub fn materialize_synthetic_stress_corpus(
    root: &Path,
    overwrite: bool,
) -> Result<Vec<SyntheticStressCase>, SyntheticStressError> {
    if root.exists() && overwrite {
        fs::remove_dir_all(root).map_err(|source| SyntheticStressError::Io {
            path: root.display().to_string(),
            source,
        })?;
    }
    fs::create_dir_all(root).map_err(|source| SyntheticStressError::Io {
        path: root.display().to_string(),
        source,
    })?;

    let mut out = Vec::new();
    for template in synthetic_stress_templates() {
        let case_dir = root.join(&template.case_id);
        fs::create_dir_all(&case_dir).map_err(|source| SyntheticStressError::Io {
            path: case_dir.display().to_string(),
            source,
        })?;
        let code_path = case_dir.join("code.py");
        let test_path = case_dir.join("test.py");
        let expected_path = case_dir.join("expected_verdict.json");
        write_file(&code_path, &template.code)?;
        write_file(&test_path, &template.test)?;
        write_json_0600(&expected_path, &template.expected)?;
        out.push(case_from_paths(
            template.case_id,
            template.kind,
            template.title,
            &case_dir,
            &code_path,
            &test_path,
            &expected_path,
        )?);
    }
    Ok(out)
}

pub fn read_synthetic_stress_corpus(
    root: &Path,
) -> Result<Vec<SyntheticStressCase>, SyntheticStressError> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut cases = Vec::new();
    for entry in fs::read_dir(root).map_err(|source| SyntheticStressError::Io {
        path: root.display().to_string(),
        source,
    })? {
        let entry = entry.map_err(|source| SyntheticStressError::Io {
            path: root.display().to_string(),
            source,
        })?;
        if !entry
            .file_type()
            .map_err(|source| SyntheticStressError::Io {
                path: entry.path().display().to_string(),
                source,
            })?
            .is_dir()
        {
            continue;
        }
        let case_dir = entry.path();
        let case_id = case_dir
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| {
                synthetic_stress_invalid(
                    "case_id",
                    format!("invalid case dir {}", case_dir.display()),
                )
            })?
            .to_string();
        let Some(kind) = SyntheticStressKind::from_case_id(&case_id) else {
            continue;
        };
        let expected_path = case_dir.join("expected_verdict.json");
        let expected = read_expected(&expected_path)?;
        let code_path = case_dir.join("code.py");
        let test_path = case_dir.join("test.py");
        cases.push(case_from_paths_with_expected(
            case_id,
            kind,
            "existing synthetic stress case".to_string(),
            &case_dir,
            &code_path,
            &test_path,
            &expected_path,
            expected,
        )?);
    }
    cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    Ok(cases)
}

fn case_from_paths(
    case_id: String,
    kind: SyntheticStressKind,
    title: String,
    case_dir: &Path,
    code_path: &Path,
    test_path: &Path,
    expected_path: &Path,
) -> Result<SyntheticStressCase, SyntheticStressError> {
    let expected = read_expected(expected_path)?;
    case_from_paths_with_expected(
        case_id,
        kind,
        title,
        case_dir,
        code_path,
        test_path,
        expected_path,
        expected,
    )
}

fn case_from_paths_with_expected(
    case_id: String,
    kind: SyntheticStressKind,
    title: String,
    case_dir: &Path,
    code_path: &Path,
    test_path: &Path,
    expected_path: &Path,
    expected: SyntheticExpectedVerdict,
) -> Result<SyntheticStressCase, SyntheticStressError> {
    validate_case_id(&case_id)?;
    Ok(SyntheticStressCase {
        schema_version: SYNTHETIC_STRESS_SCHEMA_VERSION,
        case_id,
        kind,
        title,
        case_dir: case_dir.display().to_string(),
        code_path: code_path.display().to_string(),
        test_path: test_path.display().to_string(),
        expected_verdict_path: expected_path.display().to_string(),
        code_sha256: sha256_file(code_path)?,
        test_sha256: sha256_file(test_path)?,
        expected_verdict_sha256: sha256_file(expected_path)?,
        expected,
    })
}

fn read_expected(path: &Path) -> Result<SyntheticExpectedVerdict, SyntheticStressError> {
    let bytes = fs::read(path).map_err(|source| SyntheticStressError::Io {
        path: path.display().to_string(),
        source,
    })?;
    let expected: SyntheticExpectedVerdict = serde_json::from_slice(&bytes).map_err(|source| {
        SyntheticStressError::MalformedExpectation {
            path: path.display().to_string(),
            detail: source.to_string(),
        }
    })?;
    if expected.schema_version != SYNTHETIC_STRESS_SCHEMA_VERSION {
        return Err(SyntheticStressError::MalformedExpectation {
            path: path.display().to_string(),
            detail: format!(
                "expected schema_version {SYNTHETIC_STRESS_SCHEMA_VERSION}; got {}",
                expected.schema_version
            ),
        });
    }
    Ok(expected)
}

fn validate_case_id(value: &str) -> Result<(), SyntheticStressError> {
    if value.len() <= 96
        && !value.is_empty()
        && value
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
    {
        Ok(())
    } else {
        Err(synthetic_stress_invalid(
            "case_id",
            format!("invalid case id {value:?}"),
        ))
    }
}

fn write_file(path: &Path, content: &str) -> Result<(), SyntheticStressError> {
    fs::write(path, content).map_err(|source| SyntheticStressError::Io {
        path: path.display().to_string(),
        source,
    })
}

fn sha256_file(path: &Path) -> Result<String, SyntheticStressError> {
    let bytes = fs::read(path).map_err(|source| SyntheticStressError::Io {
        path: path.display().to_string(),
        source,
    })?;
    Ok(hex::encode(sha256_bytes(&bytes)))
}
