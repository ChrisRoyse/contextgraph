use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{EvalError, EvalErrorCode};

const CELL_SIZE: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairwiseMiHeatmapMatrix {
    pub slots: Vec<String>,
    pub values: Vec<Vec<f32>>,
}

impl PairwiseMiHeatmapMatrix {
    pub fn validate(&self) -> Result<(), EvalError> {
        if self.slots.is_empty() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                "pairwise MI heatmap requires at least one slot",
            ));
        }
        if self.values.len() != self.slots.len() {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "pairwise MI heatmap row count {} does not match slot count {}",
                    self.values.len(),
                    self.slots.len()
                ),
            ));
        }
        for (row_idx, row) in self.values.iter().enumerate() {
            if row.len() != self.slots.len() {
                return Err(EvalError::new(
                    EvalErrorCode::InvalidInput,
                    format!(
                        "pairwise MI heatmap row {row_idx} has {} columns; expected {}",
                        row.len(),
                        self.slots.len()
                    ),
                ));
            }
            for (col_idx, value) in row.iter().enumerate() {
                if !value.is_finite() || !(0.0..=1.0).contains(value) {
                    return Err(EvalError::new(
                        EvalErrorCode::InvalidInput,
                        format!(
                            "pairwise MI heatmap value [{row_idx}][{col_idx}]={value} must be finite in [0,1]"
                        ),
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairwiseMiHeatmapRender {
    pub source_csv_path: PathBuf,
    pub markdown_path: PathBuf,
    pub png_path: PathBuf,
    pub slot_count: usize,
    pub width_px: usize,
    pub height_px: usize,
    pub max_off_diagonal: f32,
    pub readback_equal: bool,
}

pub fn render_pairwise_mi_heatmap(
    csv_path: impl AsRef<Path>,
    markdown_path: impl AsRef<Path>,
    png_path: impl AsRef<Path>,
) -> Result<PairwiseMiHeatmapRender, EvalError> {
    let csv_path = csv_path.as_ref();
    let markdown_path = markdown_path.as_ref();
    let png_path = png_path.as_ref();
    let matrix = load_pairwise_mi_heatmap_csv(csv_path)?;
    let markdown = render_markdown_table(&matrix);
    write_0600(markdown_path, markdown.as_bytes())?;
    let png = render_png_heatmap(&matrix)?;
    write_0600(png_path, &png)?;
    let markdown_readback = fs::read(markdown_path)?;
    let png_readback = fs::read(png_path)?;
    let readback_equal = markdown_readback == markdown.as_bytes() && png_readback == png;
    if !readback_equal {
        return Err(EvalError::new(
            EvalErrorCode::ReadbackMismatch,
            "pairwise MI heatmap artifact readback mismatch",
        ));
    }
    let side = matrix.slots.len() * CELL_SIZE;
    Ok(PairwiseMiHeatmapRender {
        source_csv_path: csv_path.to_path_buf(),
        markdown_path: markdown_path.to_path_buf(),
        png_path: png_path.to_path_buf(),
        slot_count: matrix.slots.len(),
        width_px: side,
        height_px: side,
        max_off_diagonal: max_off_diagonal(&matrix),
        readback_equal,
    })
}

pub fn load_pairwise_mi_heatmap_csv(
    path: impl AsRef<Path>,
) -> Result<PairwiseMiHeatmapMatrix, EvalError> {
    let path = path.as_ref();
    let text = fs::read_to_string(path).map_err(|err| {
        EvalError::new(
            EvalErrorCode::Store,
            format!(
                "failed to read pairwise MI heatmap CSV {}: {err}",
                path.display()
            ),
        )
    })?;
    let mut lines = text.lines();
    let header = lines.next().ok_or_else(|| {
        EvalError::new(
            EvalErrorCode::InvalidInput,
            "pairwise MI heatmap CSV is empty",
        )
    })?;
    let header_cols = header.split(',').collect::<Vec<_>>();
    if header_cols.len() < 2 || header_cols[0] != "slot" {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            "pairwise MI heatmap CSV header must start with slot",
        ));
    }
    let slots = header_cols[1..]
        .iter()
        .map(|slot| slot.trim().to_string())
        .collect::<Vec<_>>();
    if slots.iter().any(|slot| slot.is_empty()) {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            "pairwise MI heatmap CSV slot names must be non-empty",
        ));
    }
    let mut row_slots = Vec::new();
    let mut values = Vec::new();
    for (line_idx, line) in lines.enumerate() {
        let cols = line.split(',').collect::<Vec<_>>();
        if cols.len() != slots.len() + 1 {
            return Err(EvalError::new(
                EvalErrorCode::InvalidInput,
                format!(
                    "pairwise MI heatmap CSV line {} has {} columns; expected {}",
                    line_idx + 2,
                    cols.len(),
                    slots.len() + 1
                ),
            ));
        }
        row_slots.push(cols[0].trim().to_string());
        let mut row = Vec::with_capacity(slots.len());
        for raw in &cols[1..] {
            let parsed = raw.trim().parse::<f32>().map_err(|err| {
                EvalError::new(
                    EvalErrorCode::InvalidInput,
                    format!("invalid pairwise MI heatmap value {raw}: {err}"),
                )
            })?;
            row.push(parsed);
        }
        values.push(row);
    }
    if row_slots != slots {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            "pairwise MI heatmap CSV row labels must match header slots",
        ));
    }
    let matrix = PairwiseMiHeatmapMatrix { slots, values };
    matrix.validate()?;
    Ok(matrix)
}

fn render_markdown_table(matrix: &PairwiseMiHeatmapMatrix) -> String {
    let mut out = String::from("| slot |");
    for slot in &matrix.slots {
        out.push(' ');
        out.push_str(slot);
        out.push_str(" |");
    }
    out.push('\n');
    out.push_str("|---|");
    for _ in &matrix.slots {
        out.push_str("---:|");
    }
    out.push('\n');
    for (slot, row) in matrix.slots.iter().zip(&matrix.values) {
        out.push_str("| ");
        out.push_str(slot);
        out.push_str(" |");
        for value in row {
            out.push_str(&format!(" {value:.3} |"));
        }
        out.push('\n');
    }
    out
}

fn render_png_heatmap(matrix: &PairwiseMiHeatmapMatrix) -> Result<Vec<u8>, EvalError> {
    matrix.validate()?;
    let width = matrix.slots.len() * CELL_SIZE;
    let height = width;
    let mut raw = Vec::with_capacity((width * 3 + 1) * height);
    for y in 0..height {
        raw.push(0);
        let row = y / CELL_SIZE;
        for x in 0..width {
            let col = x / CELL_SIZE;
            let [r, g, b] = heat_color(matrix.values[row][col]);
            raw.extend_from_slice(&[r, g, b]);
        }
    }
    encode_png_rgb(width as u32, height as u32, &raw)
}

fn heat_color(value: f32) -> [u8; 3] {
    let clamped = value.clamp(0.0, 1.0);
    let red = (244.0 - 64.0 * clamped).round() as u8;
    let green = (247.0 - 150.0 * clamped).round() as u8;
    let blue = (251.0 - 180.0 * clamped).round() as u8;
    [red, green, blue]
}

fn encode_png_rgb(width: u32, height: u32, raw_scanlines: &[u8]) -> Result<Vec<u8>, EvalError> {
    let expected = (width as usize * 3 + 1) * height as usize;
    if raw_scanlines.len() != expected {
        return Err(EvalError::new(
            EvalErrorCode::InvalidInput,
            format!(
                "raw scanline length {} does not match expected {}",
                raw_scanlines.len(),
                expected
            ),
        ));
    }
    let mut out = Vec::new();
    out.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    write_png_chunk(&mut out, b"IHDR", &ihdr);
    write_png_chunk(&mut out, b"IDAT", &zlib_stored(raw_scanlines));
    write_png_chunk(&mut out, b"IEND", &[]);
    Ok(out)
}

fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01];
    let mut offset = 0usize;
    while offset < data.len() {
        let remaining = data.len() - offset;
        let len = remaining.min(u16::MAX as usize);
        let final_block = offset + len == data.len();
        out.push(if final_block { 0x01 } else { 0x00 });
        let len_u16 = len as u16;
        out.extend_from_slice(&len_u16.to_le_bytes());
        out.extend_from_slice(&(!len_u16).to_le_bytes());
        out.extend_from_slice(&data[offset..offset + len]);
        offset += len;
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

fn write_png_chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let mut crc_input = Vec::with_capacity(kind.len() + data.len());
    crc_input.extend_from_slice(kind);
    crc_input.extend_from_slice(data);
    out.extend_from_slice(&crc32(&crc_input).to_be_bytes());
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

fn adler32(bytes: &[u8]) -> u32 {
    const MOD: u32 = 65_521;
    let mut a = 1u32;
    let mut b = 0u32;
    for &byte in bytes {
        a = (a + u32::from(byte)) % MOD;
        b = (b + a) % MOD;
    }
    (b << 16) | a
}

fn max_off_diagonal(matrix: &PairwiseMiHeatmapMatrix) -> f32 {
    let mut max_value = 0.0;
    for (row_idx, row) in matrix.values.iter().enumerate() {
        for (col_idx, value) in row.iter().enumerate() {
            if row_idx != col_idx && *value > max_value {
                max_value = *value;
            }
        }
    }
    max_value
}

fn write_0600(path: &Path, bytes: &[u8]) -> Result<(), EvalError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}
