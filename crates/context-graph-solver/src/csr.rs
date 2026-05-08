// Inspired by ruvnet/RuVector crates/ruvector-solver/src/types.rs at HEAD ef5274c2 (read 2026-05-08).
// Clean-room reimplementation; no code copied, no upstream tracking. See
// memory/decisions/agent-141-coordinator--upstream-reference-only-clean-room.md
// for the policy.

use crate::error::{SolverError, SolverResult};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_ROWS: usize = 10_000_000;
pub const MAX_NNZ: usize = 100_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatrixKind {
    General,
    NonNegativeAdjacency,
    SymmetricPositiveDefinite,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CsrMatrix {
    pub rows: usize,
    pub cols: usize,
    pub row_ptr: Vec<usize>,
    pub col_indices: Vec<usize>,
    pub values: Vec<f64>,
    pub kind: MatrixKind,
}

impl CsrMatrix {
    pub fn from_edges(
        rows: usize,
        cols: usize,
        kind: MatrixKind,
        edges: &[(usize, usize, f64)],
    ) -> SolverResult<Self> {
        validate_dimensions(rows, cols, edges.len())?;
        let mut by_row: Vec<BTreeMap<usize, f64>> = (0..rows).map(|_| BTreeMap::new()).collect();
        for (idx, &(row, col, value)) in edges.iter().enumerate() {
            if row >= rows {
                return Err(SolverError::invalid(
                    "edges.row",
                    format!("edge {idx} row {row} is outside row count {rows}"),
                    "fix graph construction before invoking the solver",
                ));
            }
            if col >= cols {
                return Err(SolverError::invalid(
                    "edges.col",
                    format!("edge {idx} col {col} is outside column count {cols}"),
                    "fix graph construction before invoking the solver",
                ));
            }
            if !value.is_finite() {
                return Err(SolverError::invalid(
                    "edges.value",
                    format!("edge {idx} has non-finite value {value}"),
                    "remove NaN/Inf values at the graph-builder boundary",
                ));
            }
            if value == 0.0 {
                return Err(SolverError::invalid(
                    "edges.value",
                    format!("edge {idx} has zero weight"),
                    "omit zero-weight edges instead of persisting meaningless entries",
                ));
            }
            if matches!(kind, MatrixKind::NonNegativeAdjacency) && value < 0.0 {
                return Err(SolverError::invalid(
                    "edges.value",
                    format!("edge {idx} has negative adjacency weight {value}"),
                    "PPR requires non-negative transition weights",
                ));
            }
            *by_row[row].entry(col).or_insert(0.0) += value;
        }

        let mut row_ptr = Vec::with_capacity(rows + 1);
        let mut col_indices = Vec::with_capacity(edges.len());
        let mut values = Vec::with_capacity(edges.len());
        row_ptr.push(0);
        for row in by_row {
            for (col, value) in row {
                if !value.is_finite() || value == 0.0 {
                    return Err(SolverError::invalid(
                        "edges.value",
                        "duplicate edge aggregation produced a non-finite or zero value",
                        "inspect duplicate edge generation before solving",
                    ));
                }
                col_indices.push(col);
                values.push(value);
            }
            row_ptr.push(col_indices.len());
        }

        let matrix = Self {
            rows,
            cols,
            row_ptr,
            col_indices,
            values,
            kind,
        };
        matrix.validate()?;
        Ok(matrix)
    }

    pub fn validate(&self) -> SolverResult<()> {
        validate_dimensions(self.rows, self.cols, self.values.len())?;
        if self.row_ptr.len() != self.rows + 1 {
            return Err(SolverError::invalid(
                "row_ptr",
                format!(
                    "row_ptr length {} does not equal rows + 1 ({})",
                    self.row_ptr.len(),
                    self.rows + 1
                ),
                "rebuild the CSR matrix from validated edges",
            ));
        }
        if self.row_ptr.first().copied() != Some(0) {
            return Err(SolverError::invalid(
                "row_ptr[0]",
                "row_ptr[0] must be zero",
                "rebuild the CSR matrix from validated edges",
            ));
        }
        if self.row_ptr[self.rows] != self.values.len() {
            return Err(SolverError::invalid(
                "row_ptr[rows]",
                format!(
                    "row_ptr[rows] {} does not equal values length {}",
                    self.row_ptr[self.rows],
                    self.values.len()
                ),
                "rebuild the CSR matrix from validated edges",
            ));
        }
        if self.col_indices.len() != self.values.len() {
            return Err(SolverError::invalid(
                "col_indices",
                format!(
                    "col_indices length {} does not equal values length {}",
                    self.col_indices.len(),
                    self.values.len()
                ),
                "rebuild the CSR matrix from validated edges",
            ));
        }
        for idx in 1..self.row_ptr.len() {
            if self.row_ptr[idx] < self.row_ptr[idx - 1] {
                return Err(SolverError::invalid(
                    "row_ptr",
                    format!("row_ptr is non-monotonic at index {idx}"),
                    "rebuild the CSR matrix from validated edges",
                ));
            }
        }
        for row in 0..self.rows {
            let mut prev_col = None;
            for idx in self.row_ptr[row]..self.row_ptr[row + 1] {
                let col = self.col_indices[idx];
                if col >= self.cols {
                    return Err(SolverError::invalid(
                        "col_indices",
                        format!(
                            "column {col} in row {row} is outside column count {}",
                            self.cols
                        ),
                        "fix graph construction before invoking the solver",
                    ));
                }
                if let Some(prev) = prev_col {
                    if col <= prev {
                        return Err(SolverError::invalid(
                            "col_indices",
                            format!("columns in row {row} are not strictly sorted"),
                            "rebuild the CSR matrix through CsrMatrix::from_edges",
                        ));
                    }
                }
                prev_col = Some(col);
                let value = self.values[idx];
                if !value.is_finite() || value == 0.0 {
                    return Err(SolverError::invalid(
                        "values",
                        format!("matrix value at row {row}, col {col} is {value}"),
                        "omit zero entries and reject NaN/Inf at ingestion",
                    ));
                }
                if matches!(self.kind, MatrixKind::NonNegativeAdjacency) && value < 0.0 {
                    return Err(SolverError::invalid(
                        "values",
                        format!("adjacency value at row {row}, col {col} is negative"),
                        "PPR requires non-negative transition weights",
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn row_range(&self, row: usize) -> std::ops::Range<usize> {
        self.row_ptr[row]..self.row_ptr[row + 1]
    }

    pub fn row_entries(&self, row: usize) -> impl Iterator<Item = (usize, f64)> + '_ {
        self.row_range(row)
            .map(|idx| (self.col_indices[idx], self.values[idx]))
    }

    pub fn row_sum(&self, row: usize) -> f64 {
        self.row_entries(row).map(|(_, value)| value).sum()
    }

    pub fn nnz(&self) -> usize {
        self.values.len()
    }

    pub fn spmv(&self, x: &[f64]) -> SolverResult<Vec<f64>> {
        if x.len() != self.cols {
            return Err(SolverError::invalid(
                "x",
                format!(
                    "input vector length {} does not equal matrix cols {}",
                    x.len(),
                    self.cols
                ),
                "pass a vector whose length matches the CSR matrix",
            ));
        }
        if let Some((idx, value)) = x
            .iter()
            .copied()
            .enumerate()
            .find(|(_, value)| !value.is_finite())
        {
            return Err(SolverError::invalid(
                "x",
                format!("input vector x[{idx}] is non-finite: {value}"),
                "remove NaN/Inf before invoking sparse matrix-vector multiplication",
            ));
        }
        let mut y = vec![0.0; self.rows];
        for (row, out) in y.iter_mut().enumerate() {
            let mut sum = 0.0;
            for (col, value) in self.row_entries(row) {
                sum += value * x[col];
            }
            *out = sum;
        }
        Ok(y)
    }

    pub fn diagonal(&self) -> Vec<f64> {
        let mut diagonal = vec![0.0; self.rows.min(self.cols)];
        for (row, slot) in diagonal.iter_mut().enumerate() {
            for (col, value) in self.row_entries(row) {
                if row == col {
                    *slot = value;
                    break;
                }
            }
        }
        diagonal
    }
}

fn validate_dimensions(rows: usize, cols: usize, nnz: usize) -> SolverResult<()> {
    if rows == 0 || cols == 0 {
        return Err(SolverError::invalid(
            "matrix.dimensions",
            format!("matrix dimensions must be non-zero, got {rows}x{cols}"),
            "build a graph with at least one node before solving",
        ));
    }
    if rows > MAX_ROWS || cols > MAX_ROWS {
        return Err(SolverError::invalid(
            "matrix.dimensions",
            format!("matrix dimensions {rows}x{cols} exceed max rows/cols {MAX_ROWS}"),
            "split the graph or raise the hard limit after capacity planning",
        ));
    }
    if nnz > MAX_NNZ {
        return Err(SolverError::invalid(
            "matrix.nnz",
            format!("matrix nnz {nnz} exceeds max nnz {MAX_NNZ}"),
            "split the graph or raise the hard limit after capacity planning",
        ));
    }
    Ok(())
}
