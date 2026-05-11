// Inspired by ruvnet/RuVector at HEAD ef5274c2 (clean-room reimplementation).

use std::path::{Path, PathBuf};

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader};

use crate::models::{decode_session_hex32, ShiftEntry, ShiftId};
use crate::{Result, SubscriberError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileIdentity {
    len: u64,
    #[cfg(unix)]
    dev: u64,
    #[cfg(unix)]
    ino: u64,
    #[cfg(unix)]
    ctime_nsec: i64,
}

pub struct ShiftLogTail {
    path: PathBuf,
    offset: u64,
    identity: Option<FileIdentity>,
}

impl ShiftLogTail {
    pub fn new(path: impl Into<PathBuf>, offset: u64) -> Self {
        Self {
            path: path.into(),
            offset,
            identity: None,
        }
    }

    pub fn offset(&self) -> u64 {
        self.offset
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub async fn poll_next_line(&mut self) -> Result<Option<ShiftEntry>> {
        let meta = tokio::fs::metadata(&self.path)
            .await
            .map_err(|err| SubscriberError::io("metadata", self.path.clone(), err))?;
        let next_identity = file_identity(&meta);
        if let Some(prev) = self.identity {
            if rotated(prev, next_identity, self.offset) {
                self.offset = 0;
            }
        }
        self.identity = Some(next_identity);
        if meta.len() <= self.offset {
            return Ok(None);
        }
        let start = self.offset;
        let mut file = tokio::fs::File::open(&self.path)
            .await
            .map_err(|err| SubscriberError::io("open", self.path.clone(), err))?;
        file.seek(std::io::SeekFrom::Start(start))
            .await
            .map_err(|err| SubscriberError::io("seek", self.path.clone(), err))?;
        let mut reader = BufReader::new(file);
        let mut line = Vec::new();
        let read = reader
            .read_until(b'\n', &mut line)
            .await
            .map_err(|err| SubscriberError::io("read_line", self.path.clone(), err))?;
        if read == 0 || !line.ends_with(b"\n") {
            return Ok(None);
        }
        let next_offset = start + line.len() as u64;
        while matches!(line.last(), Some(b'\n' | b'\r')) {
            line.pop();
        }
        let entry = parse_shift_line(&self.path, start, next_offset, &line)?;
        self.offset = next_offset;
        Ok(Some(entry))
    }
}

fn parse_shift_line(
    path: &Path,
    byte_offset: u64,
    next_byte_offset: u64,
    line: &[u8],
) -> Result<ShiftEntry> {
    let raw: RawShiftRecord =
        serde_json::from_slice(line).map_err(|err| SubscriberError::LogParseFail {
            path: path.to_path_buf(),
            byte_offset,
            detail: err.to_string(),
        })?;
    raw.into_entry(path.to_path_buf(), byte_offset, next_byte_offset)
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawShiftRecord {
    shift_id: String,
    timestamp_unix_ns: u128,
    tool_name: String,
    tool_use_id: Option<String>,
    session_id: String,
    subject: Value,
    before: Value,
    after: Value,
    delta_summary: Value,
    verification: Value,
    harness_transition_path: Option<String>,
}

impl RawShiftRecord {
    fn into_entry(
        self,
        source_log_path: PathBuf,
        byte_offset: u64,
        next_byte_offset: u64,
    ) -> Result<ShiftEntry> {
        Ok(ShiftEntry {
            shift_id: ShiftId::parse(self.shift_id)?,
            timestamp_unix_ns: self.timestamp_unix_ns,
            tool_name: self.tool_name,
            tool_use_id: self.tool_use_id,
            session_id: decode_session_hex32(&self.session_id)?,
            subject: self.subject,
            before: self.before,
            after: self.after,
            delta_summary: self.delta_summary,
            verification: self.verification,
            harness_transition_path: self.harness_transition_path,
            byte_offset,
            next_byte_offset,
            source_log_path,
        })
    }
}

fn file_identity(meta: &std::fs::Metadata) -> FileIdentity {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        FileIdentity {
            len: meta.len(),
            dev: meta.dev(),
            ino: meta.ino(),
            ctime_nsec: meta.ctime_nsec(),
        }
    }
    #[cfg(not(unix))]
    {
        FileIdentity { len: meta.len() }
    }
}

fn rotated(previous: FileIdentity, current: FileIdentity, offset: u64) -> bool {
    if current.len < offset {
        return true;
    }
    #[cfg(unix)]
    {
        previous.dev != current.dev
            || previous.ino != current.ino
            || (previous.ctime_nsec != current.ctime_nsec && current.len < previous.len)
    }
    #[cfg(not(unix))]
    {
        current.len < previous.len
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn tail_reads_real_jsonl_offsets() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("11111111111111111111111111111111.jsonl");
        let line = serde_json::to_string(&json!({
            "shift_id": "01J0123456789ABCDEF0123",
            "timestamp_unix_ns": 1u128,
            "tool_name": "unit",
            "tool_use_id": null,
            "session_id": "11111111111111111111111111111111",
            "subject": {"task_id": "unit-task"},
            "before": {},
            "after": {},
            "delta_summary": {},
            "verification": {},
            "harness_transition_path": null
        }))
        .unwrap();
        tokio::fs::write(&path, format!("{line}\n")).await.unwrap();
        let mut tail = ShiftLogTail::new(&path, 0);
        let entry = tail.poll_next_line().await.unwrap().unwrap();
        assert_eq!(entry.byte_offset, 0);
        assert_eq!(entry.next_byte_offset, line.len() as u64 + 1);
        assert!(tail.poll_next_line().await.unwrap().is_none());
    }
}
