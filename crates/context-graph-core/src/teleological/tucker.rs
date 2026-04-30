//! Streaming HOSVD for the 13 x 13 x 1024 embedding interaction tensor.
//!
//! Consumes a [`SemanticFingerprint`] and produces a Tucker-1 decomposition at
//! configurable ranks. The conceptual interaction tensor is
//!
//! ```text
//! T[i][j][d] = v_i[d] * v_j[d]     (i, j in 0..13, d in 0..1024)
//! ```
//!
//! We never materialize the full 13 * 13 * 1024 = 173 056-float tensor. The
//! mode-n Gram matrices are built implicitly from the 13 reference-frame
//! vectors so the working set stays at O(13 * 1024) floats + the factor
//! matrices.
//!
//! # Reference frame
//!
//! All 13 embeddings are projected onto a shared 1024-D reference frame:
//!
//! - E1, E8 (source), E9:       already 1024D, used as-is
//! - E2, E3, E4, E5 (cause), E10 (paraphrase), E11:  smaller, zero-padded to 1024D on the right
//! - E7:                        1536D, truncated to the first 1024 entries
//! - E6, E13:                   sparse vocabulary vectors, represented as zeros in the
//!   reference frame (they do not live in a dense 1024-D space)
//! - E12:                       token-level; use the mean over tokens zero-padded to 1024D,
//!   or zeros when there are no tokens
//!
//! This is a deliberate simplification per §6 of the training-data export plan.
//! The 1024-D reference dimension captures the dominant dense embeddings; E6,
//! E12, and E13 contribute zeros and therefore do not distort the factor
//! matrices — they simply show up as low-energy rows in the mode-1 and mode-2
//! Gram matrices.
//!
//! # Algorithm (CPU)
//!
//! Let `V` be the 13 x 1024 matrix whose row k is the reference-frame vector
//! `v_k`.
//!
//! * **Mode-1 Gram `G_1` (13 x 13)** — derived via
//!   `G_1[a, b] = sum_d v_a[d] * v_b[d] * w[d]` where `w[d] = sum_j v_j[d]^2`.
//! * **Mode-2 Gram `G_2` (13 x 13)** — by symmetry in i <-> j this equals `G_1`.
//! * **Mode-3 Gram `G_3` (1024 x 1024)** — let `M = V^T V`; then
//!   `G_3 = M elementwise-squared`.
//!
//! Factor matrices:
//! - `U1` = top `rank_1` eigenvectors of `G_1`, shape 13 x rank_1
//! - `U2` = top `rank_2` eigenvectors of `G_2`, shape 13 x rank_2
//! - `U3` = top `rank_3` eigenvectors of `G_3`, shape 1024 x rank_3
//!
//! For the 13 x 13 Gram matrices we use Jacobi eigendecomposition (exact for
//! symmetric small matrices). For the 1024 x 1024 `G_3` we use power iteration
//! with deflation to extract only the top `rank_3` eigenvectors — avoids the
//! ~1M-element full eigendecomposition.
//!
//! Core tensor (no `T` materialization):
//!
//! ```text
//! core[r1, r2, r3] = sum_d U3[d, r3] * A[r1, d] * B[r2, d]
//! A[r1, d] = sum_i U1[i, r1] * v_i[d]       (rank_1 x 1024)
//! B[r2, d] = sum_j U2[j, r2] * v_j[d]       (rank_2 x 1024)
//! ```
//!
//! Total work for defaults `(4, 4, 128)`: 4 * 4 * 128 * 1024 ≈ 2M FMA.
//!
//! CPU only in Phase 4; see plan §6.3. A GPU path is intentionally out of scope.

use crate::teleological::types::{TuckerCore, NUM_EMBEDDERS};
use crate::types::fingerprint::SemanticFingerprint;

/// Reference embedding dimension used for the Tucker decomposition.
///
/// All 13 embeddings are padded or truncated to this width before the HOSVD
/// runs. See the module-level docstring for the per-embedder mapping.
pub const REFERENCE_EMBEDDING_DIM: usize = 1024;

/// Default Tucker ranks for training export, inherited from
/// `TuckerCore::DEFAULT_RANKS`.
pub const DEFAULT_RANKS: (usize, usize, usize) = (4, 4, 128);

/// Lower bound for detecting an all-zero fingerprint (sum of squared entries
/// across all reference-frame vectors). Anything below this is treated as
/// numerically empty and the compressor returns [`TuckerError::EmptyFingerprint`].
const EMPTY_ENERGY_THRESHOLD: f64 = 1e-24;

/// Max number of sweeps in the Jacobi eigendecomposition. 50 is far more than
/// enough to converge a 13 x 13 symmetric matrix to machine precision.
const JACOBI_MAX_SWEEPS: usize = 50;

/// Tolerance for Jacobi off-diagonal mass (in f64). Below this the matrix is
/// considered diagonal.
const JACOBI_TOL: f64 = 1e-14;

/// Max power iterations per eigenpair before we fail loud.
const POWER_ITER_MAX: usize = 500;

/// Convergence tolerance for power iteration. Relaxed to 1e-5 because we only
/// need a usable top-k eigenbasis (not full machine precision) and closely-
/// spaced eigenvalues otherwise oscillate indefinitely under pure power
/// iteration. This is the standard accuracy/stability tradeoff documented in
/// Golub & Van Loan §7.3: for a simple eigenpair, power iteration converges
/// at rate |lambda_{k+1}/lambda_k|; clustered eigenvalues yield very slow
/// direction convergence, and tightening the tolerance would require
/// block / subspace iteration which is out of scope for Phase 4.
const POWER_ITER_TOL: f64 = 1e-5;

/// Errors produced by the Tucker compressor.
#[derive(thiserror::Error, Debug)]
pub enum TuckerError {
    /// Ranks outside the valid range `1..=dim` for their mode.
    #[error(
        "rank must be >= 1 and <= dim; got rank_1={r1} rank_2={r2} rank_3={r3} \
         for dims ({d1},{d2},{d3})"
    )]
    InvalidRank {
        /// Requested rank_1.
        r1: usize,
        /// Requested rank_2.
        r2: usize,
        /// Requested rank_3.
        r3: usize,
        /// Actual mode-1 dim.
        d1: usize,
        /// Actual mode-2 dim.
        d2: usize,
        /// Actual mode-3 dim.
        d3: usize,
    },
    /// Every reference-frame vector is (essentially) zero — nothing to decompose.
    #[error("fingerprint has no dense embeddings suitable for Tucker decomposition")]
    EmptyFingerprint,
    /// Power iteration failed to converge within the budgeted number of
    /// iterations, or another numerical failure occurred.
    #[error("numerical failure in HOSVD: {0}")]
    Numerical(String),
}

/// Trait for pluggable Tucker compressors. Phase 4 ships only the CPU
/// implementation; a GPU path is out of scope per the plan.
pub trait TuckerCompressor {
    /// Compress a fingerprint into a Tucker-1 decomposition.
    fn compress(
        &self,
        fingerprint: &SemanticFingerprint,
        ranks: (usize, usize, usize),
    ) -> Result<TuckerCore, TuckerError>;
}

/// Pure-Rust CPU compressor. No threading, no heavy deps, no GPU.
pub struct CpuTuckerCompressor;

impl TuckerCompressor for CpuTuckerCompressor {
    fn compress(
        &self,
        fingerprint: &SemanticFingerprint,
        ranks: (usize, usize, usize),
    ) -> Result<TuckerCore, TuckerError> {
        let (r1, r2, r3) = ranks;
        if r1 < 1
            || r2 < 1
            || r3 < 1
            || r1 > NUM_EMBEDDERS
            || r2 > NUM_EMBEDDERS
            || r3 > REFERENCE_EMBEDDING_DIM
        {
            return Err(TuckerError::InvalidRank {
                r1,
                r2,
                r3,
                d1: NUM_EMBEDDERS,
                d2: NUM_EMBEDDERS,
                d3: REFERENCE_EMBEDDING_DIM,
            });
        }

        let views = extract_reference_view(fingerprint);

        // Energy check: total squared mass across all reference-frame entries.
        let mut total_energy: f64 = 0.0;
        for row in views.iter() {
            for &x in row.iter() {
                total_energy += (x as f64) * (x as f64);
            }
        }
        if !total_energy.is_finite() || total_energy < EMPTY_ENERGY_THRESHOLD {
            return Err(TuckerError::EmptyFingerprint);
        }

        // Mode-1/2 Gram (13 x 13, identical by i<->j symmetry).
        // G_1[a,b] = sum_d v_a[d] * v_b[d] * w[d], w[d] = sum_j v_j[d]^2
        let mut w = [0.0f64; REFERENCE_EMBEDDING_DIM];
        for row in views.iter() {
            for (d, &x) in row.iter().enumerate() {
                w[d] += (x as f64) * (x as f64);
            }
        }
        let mut g1 = [[0.0f64; NUM_EMBEDDERS]; NUM_EMBEDDERS];
        for a in 0..NUM_EMBEDDERS {
            for b in a..NUM_EMBEDDERS {
                let mut s = 0.0f64;
                for d in 0..REFERENCE_EMBEDDING_DIM {
                    s += (views[a][d] as f64) * (views[b][d] as f64) * w[d];
                }
                g1[a][b] = s;
                g1[b][a] = s;
            }
        }

        let (_eig_a, u1_cols) = jacobi_eigen_13(g1, r1)?;
        // G_2 == G_1, so U2 can be derived from the same decomposition. We still
        // compute it separately so callers can later ask for different rank_2.
        // For correctness we just clone the eigenbasis.
        let (_eig_b, u2_cols) = jacobi_eigen_13(g1, r2)?;

        // Mode-3 Gram: G_3[p,q] = (sum_i v_i[p] * v_i[q])^2 = M[p,q]^2
        // where M = V^T V (1024 x 1024).
        // Power iteration with deflation extracts the top r3 eigenvectors
        // without materializing G_3 explicitly — we compute G_3 @ x lazily.
        //
        // G_3 @ x  =  (M elementwise_sq) @ x
        //          =  for each p: sum_q (M[p,q])^2 * x[q]
        //
        // That still needs M, but M itself is tractable: 13 * 1024 * 1024 FMA
        // ≈ 13M ops, ~52 MB of f32 storage. We store it as f32 to cap memory.
        let m_flat: Vec<f32> = {
            let mut m = vec![0.0f32; REFERENCE_EMBEDDING_DIM * REFERENCE_EMBEDDING_DIM];
            for i in 0..NUM_EMBEDDERS {
                for p in 0..REFERENCE_EMBEDDING_DIM {
                    let vp = views[i][p];
                    if vp == 0.0 {
                        continue;
                    }
                    let row =
                        &mut m[p * REFERENCE_EMBEDDING_DIM..(p + 1) * REFERENCE_EMBEDDING_DIM];
                    for q in 0..REFERENCE_EMBEDDING_DIM {
                        row[q] += vp * views[i][q];
                    }
                }
            }
            m
        };

        let u3_cols = power_iteration_top_k(&m_flat, REFERENCE_EMBEDDING_DIM, r3)?;

        // Core tensor: core[r1, r2, r3] = sum_d U3[d, r3] * A[r1, d] * B[r2, d]
        // A[r1, d] = sum_i U1[i, r1] * v_i[d]
        // B[r2, d] = sum_j U2[j, r2] * v_j[d]
        let a_mat = project_factor_onto_views(&u1_cols, &views); // r1 x 1024
        let b_mat = project_factor_onto_views(&u2_cols, &views); // r2 x 1024

        let mut core = vec![0.0f32; r1 * r2 * r3];
        for i in 0..r1 {
            for j in 0..r2 {
                for k in 0..r3 {
                    // dot_d(U3[:,k] * A[i,:] * B[j,:])
                    let u3_col = &u3_cols[k];
                    let a_row =
                        &a_mat[i * REFERENCE_EMBEDDING_DIM..(i + 1) * REFERENCE_EMBEDDING_DIM];
                    let b_row =
                        &b_mat[j * REFERENCE_EMBEDDING_DIM..(j + 1) * REFERENCE_EMBEDDING_DIM];
                    let mut s = 0.0f64;
                    for d in 0..REFERENCE_EMBEDDING_DIM {
                        s += (u3_col[d] as f64) * (a_row[d] as f64) * (b_row[d] as f64);
                    }
                    core[i * r2 * r3 + j * r3 + k] = s as f32;
                }
            }
        }

        // Flatten factor matrices into column-major packing
        // U1[i, r1] -> u1[i*r1 + col]
        let u1_flat = flatten_columns(&u1_cols, NUM_EMBEDDERS, r1);
        let u2_flat = flatten_columns(&u2_cols, NUM_EMBEDDERS, r2);
        let u3_flat = flatten_columns(&u3_cols, REFERENCE_EMBEDDING_DIM, r3);

        Ok(TuckerCore {
            ranks: (r1, r2, r3),
            data: core,
            u1: u1_flat,
            u2: u2_flat,
            u3: u3_flat,
        })
    }
}

// =============================================================================
// Reference-frame extraction
// =============================================================================

/// Project a [`SemanticFingerprint`] onto the shared 1024-D reference frame.
///
/// For asymmetric embedders (E5/E8/E10) this uses the cause / source /
/// paraphrase side. Smaller vectors are zero-padded; larger vectors (E7=1536)
/// are truncated to the first 1024 entries. Sparse vectors (E6, E13) and
/// token-level (E12, when empty) become all-zero rows.
pub fn extract_reference_view(
    fp: &SemanticFingerprint,
) -> Box<[[f32; REFERENCE_EMBEDDING_DIM]; NUM_EMBEDDERS]> {
    // Allocate on the heap to avoid a 13*1024*4 = 52KB stack frame
    let mut boxed: Box<[[f32; REFERENCE_EMBEDDING_DIM]; NUM_EMBEDDERS]> =
        vec![[0.0f32; REFERENCE_EMBEDDING_DIM]; NUM_EMBEDDERS]
            .into_boxed_slice()
            .try_into()
            .expect("constant size");

    copy_dense_to_row(&fp.e1_semantic, &mut boxed[0]); // E1 1024D
    copy_dense_to_row(&fp.e2_temporal_recent, &mut boxed[1]); // E2 512D pad
    copy_dense_to_row(&fp.e3_temporal_periodic, &mut boxed[2]); // E3 512D pad
    copy_dense_to_row(&fp.e4_temporal_positional, &mut boxed[3]); // E4 512D pad
    copy_dense_to_row(fp.get_e5_as_cause(), &mut boxed[4]); // E5 cause 768D pad
                                                            // E6 sparse -> zero row (index 5)
    copy_dense_to_row(&fp.e7_code, &mut boxed[6]); // E7 1536D truncate
    copy_dense_to_row(fp.get_e8_as_source(), &mut boxed[7]); // E8 source 1024D
    copy_dense_to_row(&fp.e9_hdc, &mut boxed[8]); // E9 1024D
    copy_dense_to_row(&fp.e10_multimodal_paraphrase, &mut boxed[9]); // E10 para 768D pad
    copy_dense_to_row(&fp.e11_entity, &mut boxed[10]); // E11 768D pad
    copy_tokens_mean_to_row(&fp.e12_late_interaction, &mut boxed[11]); // E12 mean-pool
                                                                       // E13 sparse -> zero row (index 12)

    boxed
}

/// Copy `src` into `dst` with right-padding (if `src` shorter) or truncation
/// (if `src` longer). Non-finite entries are coerced to 0.0 to keep the
/// downstream eigendecomposition well-defined.
fn copy_dense_to_row(src: &[f32], dst: &mut [f32; REFERENCE_EMBEDDING_DIM]) {
    let n = src.len().min(REFERENCE_EMBEDDING_DIM);
    for d in 0..n {
        let x = src[d];
        dst[d] = if x.is_finite() { x } else { 0.0 };
    }
    // entries [n..REFERENCE_EMBEDDING_DIM] stay at 0.0
}

/// Mean-pool a token-level embedding (E12 ColBERT-style: variable tokens x
/// 128D each) into a 1024-D row. We pool each token up to 128 entries into the
/// row (the remaining 1024-128 = 896 entries stay zero), then divide by the
/// token count. Empty input yields the zero row.
fn copy_tokens_mean_to_row(tokens: &[Vec<f32>], dst: &mut [f32; REFERENCE_EMBEDDING_DIM]) {
    if tokens.is_empty() {
        return;
    }
    let mut count: u32 = 0;
    for tok in tokens {
        let n = tok.len().min(REFERENCE_EMBEDDING_DIM);
        let mut any_finite = false;
        for d in 0..n {
            let x = tok[d];
            if x.is_finite() {
                dst[d] += x;
                any_finite = true;
            }
        }
        if any_finite {
            count += 1;
        }
    }
    if count == 0 {
        for d in 0..REFERENCE_EMBEDDING_DIM {
            dst[d] = 0.0;
        }
        return;
    }
    let inv = 1.0f32 / count as f32;
    for d in 0..REFERENCE_EMBEDDING_DIM {
        dst[d] *= inv;
    }
}

// =============================================================================
// Small-matrix Jacobi eigendecomposition (13 x 13)
// =============================================================================

/// Jacobi eigendecomposition for a 13 x 13 symmetric matrix. Returns the top
/// `k` eigenvalues (descending by magnitude) and their eigenvectors as columns.
///
/// Result is a `(eigenvalues: Vec<f64>, eigenvectors: Vec<Vec<f32>>)` where
/// eigenvectors has length `k` and each inner vec has length 13 (one column).
fn jacobi_eigen_13(
    mat: [[f64; NUM_EMBEDDERS]; NUM_EMBEDDERS],
    k: usize,
) -> Result<(Vec<f64>, Vec<Vec<f32>>), TuckerError> {
    assert!((1..=NUM_EMBEDDERS).contains(&k));

    let mut a = mat;
    // Eigenvectors as columns of v (13 x 13); initialize to identity.
    let mut v = [[0.0f64; NUM_EMBEDDERS]; NUM_EMBEDDERS];
    for i in 0..NUM_EMBEDDERS {
        v[i][i] = 1.0;
    }

    // Scale-aware tolerance: compare off-diagonal Frobenius mass to the
    // diagonal mass. A symmetric matrix is considered diagonalized when the
    // off-diagonal mass is negligible relative to the diagonal mass.
    let mut converged = false;
    for _sweep in 0..JACOBI_MAX_SWEEPS {
        // Off-diagonal and diagonal Frobenius masses
        let mut off = 0.0f64;
        let mut diag = 0.0f64;
        for p in 0..NUM_EMBEDDERS {
            diag += a[p][p] * a[p][p];
            for q in (p + 1)..NUM_EMBEDDERS {
                off += a[p][q] * a[p][q];
            }
        }
        // off is half the symmetric mass (upper triangle only). Scale against
        // total Frobenius = diag + 2*off. Convergence when off / (diag + 1) is tiny.
        if off < JACOBI_TOL * (diag + 1.0) {
            converged = true;
            break;
        }

        for p in 0..NUM_EMBEDDERS {
            for q in (p + 1)..NUM_EMBEDDERS {
                let apq = a[p][q];
                if apq == 0.0 {
                    continue;
                }
                let app = a[p][p];
                let aqq = a[q][q];
                // Guard against near-zero 2*apq; when the diagonal difference
                // is already huge relative to apq, a tiny rotation keeps things
                // stable. theta = (aqq - app) / (2*apq)
                let theta = (aqq - app) / (2.0 * apq);
                let t = if theta >= 0.0 {
                    1.0 / (theta + (1.0 + theta * theta).sqrt())
                } else {
                    1.0 / (theta - (1.0 + theta * theta).sqrt())
                };
                let c = 1.0 / (1.0 + t * t).sqrt();
                let s = t * c;

                // Update A
                a[p][p] = app - t * apq;
                a[q][q] = aqq + t * apq;
                a[p][q] = 0.0;
                a[q][p] = 0.0;
                for r in 0..NUM_EMBEDDERS {
                    if r != p && r != q {
                        let arp = a[r][p];
                        let arq = a[r][q];
                        a[r][p] = c * arp - s * arq;
                        a[p][r] = a[r][p];
                        a[r][q] = s * arp + c * arq;
                        a[q][r] = a[r][q];
                    }
                }

                // Update V (rotate columns p and q)
                for r in 0..NUM_EMBEDDERS {
                    let vrp = v[r][p];
                    let vrq = v[r][q];
                    v[r][p] = c * vrp - s * vrq;
                    v[r][q] = s * vrp + c * vrq;
                }
            }
        }
    }
    if !converged {
        return Err(TuckerError::Numerical(format!(
            "Jacobi 13x13 did not converge within {} sweeps",
            JACOBI_MAX_SWEEPS
        )));
    }

    // Collect eigenvalues (diagonal of a) and sort indices by |eigenvalue| desc.
    let mut pairs: Vec<(usize, f64)> = (0..NUM_EMBEDDERS).map(|i| (i, a[i][i])).collect();
    pairs.sort_by(|x, y| {
        y.1.abs()
            .partial_cmp(&x.1.abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let eigenvalues: Vec<f64> = pairs.iter().take(k).map(|&(_, e)| e).collect();
    let eigenvectors: Vec<Vec<f32>> = pairs
        .iter()
        .take(k)
        .map(|&(idx, _)| {
            (0..NUM_EMBEDDERS)
                .map(|r| v[r][idx] as f32)
                .collect::<Vec<f32>>()
        })
        .collect();

    Ok((eigenvalues, eigenvectors))
}

// =============================================================================
// Power iteration with deflation for the mode-3 Gram (1024 x 1024)
// =============================================================================

/// Extract the top `k` eigenvectors of `G_3 = M elementwise_squared` where `M`
/// is a symmetric 1024 x 1024 matrix stored row-major in `m_flat`. We never
/// materialize `G_3`; we compute `G_3 @ x` by summing `(M[p,q])^2 * x[q]`.
///
/// Deflation: after extracting eigenpair `(lambda_k, u_k)`, subtract
/// `lambda_k * u_k u_k^T` from the working operator so subsequent iterations
/// converge to the next-largest eigenvalue. We implement this by keeping an
/// explicit list of already-extracted `(lambda, u)` pairs and subtracting
/// their contribution from `G_3 @ x` at each multiply.
fn power_iteration_top_k(m_flat: &[f32], n: usize, k: usize) -> Result<Vec<Vec<f32>>, TuckerError> {
    assert_eq!(m_flat.len(), n * n);

    let mut deflated: Vec<(f64, Vec<f32>)> = Vec::with_capacity(k);
    let mut out: Vec<Vec<f32>> = Vec::with_capacity(k);

    // Deterministic seed vector (no RNG dep): alternating signs + index.
    // Any non-zero non-degenerate vector works.
    let init_vec = |seed: usize| -> Vec<f32> {
        let mut x = vec![0.0f32; n];
        for i in 0..n {
            // Mix seed and index so different eigenpair attempts start from
            // different directions (helps when deflated is empty).
            let s = ((i.wrapping_add(seed * 7919)) % 1024) as f32 + 1.0;
            x[i] = if (i + seed).is_multiple_of(2) { s } else { -s };
        }
        normalize_f32(&mut x);
        x
    };

    for eig_idx in 0..k {
        let mut x = init_vec(eig_idx);
        // Orthogonalize against previously-extracted eigenvectors so we don't
        // immediately re-converge to them.
        reorthogonalize(&mut x, &deflated);
        normalize_f32(&mut x);

        let mut lambda = 0.0f64;
        let mut prev_lambda = f64::NAN;
        let mut converged = false;

        for _iter in 0..POWER_ITER_MAX {
            // y = G_3 @ x. We perform explicit Gram-Schmidt reorthogonalization
            // against the previously-extracted eigenvectors after the apply to
            // stay in the orthogonal complement of the already-seen subspace
            // (near-degenerate eigenvalues cause pure deflation to drift).
            let mut y = g3_apply(m_flat, n, &x);
            reorthogonalize(&mut y, &deflated);

            // Rayleigh quotient estimate = <y, x> / <x, x> ; x is already
            // unit-normalized.
            lambda = dot_f32(&y, &x) as f64;

            // Normalize y -> new x
            let norm_sq: f64 = y.iter().map(|v| (*v as f64) * (*v as f64)).sum();
            if !norm_sq.is_finite() {
                return Err(TuckerError::Numerical(format!(
                    "power iteration produced non-finite norm for eigenpair {}",
                    eig_idx
                )));
            }
            if norm_sq.sqrt() < POWER_ITER_TOL.sqrt() {
                // The residual subspace is exhausted — the remaining eigen-
                // values are effectively zero. Emit a zero eigenvector; the
                // core tensor row multiplying it will be zero too.
                x = vec![0.0f32; n];
                lambda = 0.0;
                converged = true;
                break;
            }
            let inv = (1.0 / norm_sq.sqrt()) as f32;
            for i in 0..n {
                y[i] *= inv;
            }

            // Direction convergence: distance between x and y (up to sign).
            let d_plus: f64 = x
                .iter()
                .zip(y.iter())
                .map(|(a, b)| {
                    let d = *a as f64 - *b as f64;
                    d * d
                })
                .sum();
            let d_minus: f64 = x
                .iter()
                .zip(y.iter())
                .map(|(a, b)| {
                    let d = *a as f64 + *b as f64;
                    d * d
                })
                .sum();
            let delta = d_plus.min(d_minus);

            x = y;

            // Eigenvalue convergence: Rayleigh quotient stabilized.
            let lambda_delta = if prev_lambda.is_finite() {
                ((lambda - prev_lambda).abs()) / (prev_lambda.abs() + 1.0)
            } else {
                f64::INFINITY
            };
            prev_lambda = lambda;

            if delta < POWER_ITER_TOL && lambda_delta < POWER_ITER_TOL {
                converged = true;
                break;
            }
        }

        if !converged {
            return Err(TuckerError::Numerical(format!(
                "power iteration did not converge for eigenpair {} within {} iterations \
                 (last lambda={}, last delta≈{})",
                eig_idx, POWER_ITER_MAX, lambda, prev_lambda
            )));
        }

        deflated.push((lambda, x.clone()));
        out.push(x);
    }

    Ok(out)
}

/// Gram-Schmidt reorthogonalize `x` against every already-extracted eigen-
/// vector in `deflated`. Runs two passes for numerical stability (classical
/// Gram-Schmidt with reorthogonalization).
fn reorthogonalize(x: &mut [f32], deflated: &[(f64, Vec<f32>)]) {
    for _pass in 0..2 {
        for (_, u_prev) in deflated.iter() {
            let dot = dot_f32(u_prev, x);
            if dot == 0.0 {
                continue;
            }
            for i in 0..x.len() {
                x[i] -= dot * u_prev[i];
            }
        }
    }
}

/// Compute `y = G_3 @ x` where `G_3[p,q] = (M[p,q])^2`. The 1024-vector `y`
/// is returned fresh on every call.
fn g3_apply(m_flat: &[f32], n: usize, x: &[f32]) -> Vec<f32> {
    let mut y = vec![0.0f32; n];
    for p in 0..n {
        let mut s = 0.0f64;
        let row = &m_flat[p * n..(p + 1) * n];
        for q in 0..n {
            let m_pq = row[q] as f64;
            s += m_pq * m_pq * (x[q] as f64);
        }
        y[p] = s as f32;
    }
    y
}

fn dot_f32(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    let mut s = 0.0f64;
    for i in 0..a.len() {
        s += (a[i] as f64) * (b[i] as f64);
    }
    s as f32
}

fn normalize_f32(x: &mut [f32]) {
    let norm_sq: f64 = x.iter().map(|v| (*v as f64) * (*v as f64)).sum();
    if norm_sq <= 0.0 {
        return;
    }
    let inv = (1.0 / norm_sq.sqrt()) as f32;
    for xi in x.iter_mut() {
        *xi *= inv;
    }
}

// =============================================================================
// Factor projection / flattening helpers
// =============================================================================

/// Compute `out[r, d] = sum_i U[:, r][i] * views[i][d]` as a flat
/// row-major matrix of shape `r x 1024`.
fn project_factor_onto_views(
    u_cols: &[Vec<f32>],
    views: &[[f32; REFERENCE_EMBEDDING_DIM]; NUM_EMBEDDERS],
) -> Vec<f32> {
    let r = u_cols.len();
    let mut out = vec![0.0f32; r * REFERENCE_EMBEDDING_DIM];
    for (col, col_vec) in u_cols.iter().enumerate() {
        for i in 0..NUM_EMBEDDERS {
            let coeff = col_vec[i];
            if coeff == 0.0 {
                continue;
            }
            let row = &mut out[col * REFERENCE_EMBEDDING_DIM..(col + 1) * REFERENCE_EMBEDDING_DIM];
            let src = &views[i];
            for d in 0..REFERENCE_EMBEDDING_DIM {
                row[d] += coeff * src[d];
            }
        }
    }
    out
}

/// Flatten `cols[c][r]` into a row-major `rows x cols_count` matrix where
/// cell `(i, c)` is `cols[c][i]`. Used to convert the internal
/// column-of-eigenvectors representation into `TuckerCore`'s packed layout
/// where `u1[i*rank_1 + r1]` gives `U1[i, r1]`.
fn flatten_columns(cols: &[Vec<f32>], rows: usize, cols_count: usize) -> Vec<f32> {
    debug_assert_eq!(cols.len(), cols_count);
    let mut out = vec![0.0f32; rows * cols_count];
    for c in 0..cols_count {
        debug_assert_eq!(cols[c].len(), rows);
        for i in 0..rows {
            out[i * cols_count + c] = cols[c][i];
        }
    }
    out
}

// =============================================================================
// Test helpers (public so downstream FSV can call them)
// =============================================================================

/// Reconstruct the full 13 x 13 x 1024 interaction tensor from a TuckerCore.
///
/// Output is flat row-major with `idx(i, j, d) = i * 13 * 1024 + j * 1024 + d`.
/// Length is `13 * 13 * 1024 = 173_056` floats. Primarily used in tests to
/// verify the Frobenius reconstruction error bound (FSV-16).
pub fn reconstruct_tensor(core: &TuckerCore) -> Vec<f32> {
    let (r1, r2, r3) = core.ranks;
    let mut out = vec![0.0f32; NUM_EMBEDDERS * NUM_EMBEDDERS * REFERENCE_EMBEDDING_DIM];
    // T_hat[i,j,d] = sum_{a,b,c} core[a,b,c] * U1[i,a] * U2[j,b] * U3[d,c]
    // Contract in this order: first core x U3 -> tmp_ab[d], then fold over a,b.
    for a in 0..r1 {
        for b in 0..r2 {
            // Build tmp_d = sum_c core[a,b,c] * U3[d, c]  (length 1024)
            let mut tmp_d = vec![0.0f64; REFERENCE_EMBEDDING_DIM];
            for c in 0..r3 {
                let coeff = core.data[a * r2 * r3 + b * r3 + c] as f64;
                if coeff == 0.0 {
                    continue;
                }
                for d in 0..REFERENCE_EMBEDDING_DIM {
                    tmp_d[d] += coeff * (core.u3[d * r3 + c] as f64);
                }
            }
            // Now add U1[i, a] * U2[j, b] * tmp_d[d] into out[i, j, d]
            for i in 0..NUM_EMBEDDERS {
                let u1_ia = core.u1[i * r1 + a] as f64;
                if u1_ia == 0.0 {
                    continue;
                }
                for j in 0..NUM_EMBEDDERS {
                    let u2_jb = core.u2[j * r2 + b] as f64;
                    if u2_jb == 0.0 {
                        continue;
                    }
                    let coef = u1_ia * u2_jb;
                    let base =
                        i * NUM_EMBEDDERS * REFERENCE_EMBEDDING_DIM + j * REFERENCE_EMBEDDING_DIM;
                    for d in 0..REFERENCE_EMBEDDING_DIM {
                        out[base + d] += (coef * tmp_d[d]) as f32;
                    }
                }
            }
        }
    }
    out
}

/// Relative Frobenius error: `||a - b||_F / ||a||_F`. Returns 0 if `a` is zero.
pub fn relative_frobenius_error(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len());
    let mut num = 0.0f64;
    let mut den = 0.0f64;
    for i in 0..a.len() {
        let ai = a[i] as f64;
        let bi = b[i] as f64;
        let diff = ai - bi;
        num += diff * diff;
        den += ai * ai;
    }
    if den == 0.0 {
        return 0.0;
    }
    (num.sqrt() / den.sqrt()) as f32
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::fingerprint::{SemanticFingerprint, SparseVector};

    /// Build a fingerprint with all reference-frame vectors equal to
    /// `[c, 0, 0, ..., 0]` (a single direction along the first axis). E6/E12/E13
    /// stay empty; all asymmetric dual-vector fields are populated on the cause/source/paraphrase
    /// side as this is the side extract_reference_view uses.
    fn single_direction_fingerprint(c: f32) -> SemanticFingerprint {
        let mk = |dim: usize, first: f32| -> Vec<f32> {
            let mut v = vec![0.0f32; dim];
            if dim > 0 {
                v[0] = first;
            }
            v
        };
        SemanticFingerprint {
            e1_semantic: mk(1024, c),
            e2_temporal_recent: mk(512, c),
            e3_temporal_periodic: mk(512, c),
            e4_temporal_positional: mk(512, c),
            e5_causal_as_cause: mk(768, c),
            e5_causal_as_effect: Vec::new(),
            e5_causal: Vec::new(),
            e6_sparse: SparseVector::empty(),
            e7_code: mk(1536, c),
            e8_graph_as_source: mk(1024, c),
            e8_graph_as_target: Vec::new(),
            e8_graph: Vec::new(),
            e9_hdc: mk(1024, c),
            e10_multimodal_paraphrase: mk(768, c),
            e10_multimodal_as_context: Vec::new(),
            e11_entity: mk(768, c),
            e12_late_interaction: Vec::new(),
            e13_splade: SparseVector::empty(),
            e14_bge_m3_dense: mk(1024, c),
        }
    }

    /// Deterministic structured fingerprint for reconstruction-error testing.
    ///
    /// Constructs each row k as a low-rank combination of two fixed 1024-D
    /// basis functions f(d) = cos(d / 50.0), g(d) = sin(d / 30.0):
    ///
    /// ```text
    /// v_k[d] = a_k * f(d) + b_k * g(d)
    /// ```
    ///
    /// Truncated/padded to each embedder's native dimension, the effective
    /// mode-3 rank is 2 (plus boundary effects from truncation to varying
    /// native dims), and the mode-1/mode-2 Gram matrices have rank ≤ 2. At
    /// Tucker rank (4, 4, 128) reconstruction should be near-perfect.
    fn structured_fingerprint() -> SemanticFingerprint {
        let a_coeffs: [f32; 14] = [
            1.0, 0.7, -0.3, 0.5, -0.2, 0.0, 0.9, 0.1, -0.4, 0.6, -0.1, 0.0, 0.0, 0.0,
        ];
        let b_coeffs: [f32; 14] = [
            0.2, -0.6, 0.4, -0.8, 0.3, 0.0, -0.5, 0.7, 0.1, -0.2, 0.5, 0.0, 0.0, 0.0,
        ];
        let make_row = |k: usize, native_dim: usize| -> Vec<f32> {
            let a = a_coeffs[k];
            let b = b_coeffs[k];
            (0..native_dim)
                .map(|d| {
                    let fd = (d as f32 / 50.0).cos();
                    let gd = (d as f32 / 30.0).sin();
                    a * fd + b * gd
                })
                .collect()
        };
        SemanticFingerprint {
            e1_semantic: make_row(0, 1024),
            e2_temporal_recent: make_row(1, 512),
            e3_temporal_periodic: make_row(2, 512),
            e4_temporal_positional: make_row(3, 512),
            e5_causal_as_cause: make_row(4, 768),
            e5_causal_as_effect: Vec::new(),
            e5_causal: Vec::new(),
            e6_sparse: SparseVector::empty(),
            e7_code: make_row(6, 1536),
            e8_graph_as_source: make_row(7, 1024),
            e8_graph_as_target: Vec::new(),
            e8_graph: Vec::new(),
            e9_hdc: make_row(8, 1024),
            e10_multimodal_paraphrase: make_row(9, 768),
            e10_multimodal_as_context: Vec::new(),
            e11_entity: make_row(10, 768),
            e12_late_interaction: Vec::new(),
            e13_splade: SparseVector::empty(),
            e14_bge_m3_dense: make_row(13, 1024),
        }
    }

    /// Build the full 13 x 13 x 1024 interaction tensor from the same views
    /// `extract_reference_view` produces. Used to ground-truth the
    /// reconstruction error in FSV-16.
    fn materialize_full_tensor(fp: &SemanticFingerprint) -> Vec<f32> {
        let views = extract_reference_view(fp);
        let mut out = vec![0.0f32; NUM_EMBEDDERS * NUM_EMBEDDERS * REFERENCE_EMBEDDING_DIM];
        for i in 0..NUM_EMBEDDERS {
            for j in 0..NUM_EMBEDDERS {
                for d in 0..REFERENCE_EMBEDDING_DIM {
                    out[i * NUM_EMBEDDERS * REFERENCE_EMBEDDING_DIM
                        + j * REFERENCE_EMBEDDING_DIM
                        + d] = views[i][d] * views[j][d];
                }
            }
        }
        out
    }

    #[test]
    fn degenerate_single_direction_yields_rank_1_structure() {
        // All 10 dense embedders (indices 0,1,2,3,4,6,7,8,9,10 — E12 token is
        // empty in this fixture) have (c, 0, 0, ...) in the 1024D frame;
        // E6/E13/E12 are zero rows. Mode-1 Gram is therefore a rank-1 matrix
        // whose only non-zero block is a 10x10 "constant" block on the dense
        // indices. Its top eigenvector must concentrate on those rows, with
        // equal magnitude 1/sqrt(10) per dense row and 0 on the sparse/token
        // rows.
        let fp = single_direction_fingerprint(0.5);
        let core = CpuTuckerCompressor
            .compress(&fp, (4, 4, 128))
            .expect("compress succeeds");

        assert_eq!(core.ranks, (4, 4, 128));

        let dense_rows = [0usize, 1, 2, 3, 4, 6, 7, 8, 9, 10];
        let sparse_rows = [5usize, 11, 12];
        let expected_mag = 1.0f32 / (dense_rows.len() as f32).sqrt();

        // Column 0 of U1 is the top eigenvector. Each dense row should have
        // magnitude ~1/sqrt(10); sparse/token rows should be ~0.
        for &i in dense_rows.iter() {
            let entry = core.u1[i * 4].abs();
            assert!(
                (entry - expected_mag).abs() < 1e-3,
                "U1 col0 row {} should have |{}| ≈ 1/sqrt(10) = {}, got {}",
                i,
                entry,
                expected_mag,
                entry,
            );
        }
        for &i in sparse_rows.iter() {
            let entry = core.u1[i * 4].abs();
            assert!(
                entry < 1e-5,
                "U1 col0 row {} (sparse) should be ~0, got {}",
                i,
                entry,
            );
        }

        // Because the rank of G_1 is 1, the top eigenvalue dominates and the
        // corresponding eigenvector lives entirely in the dense subspace.
        // Column 0 is unique (up to sign). Its squared-L2 norm should be ~1.
        let col0_norm_sq: f32 = (0..NUM_EMBEDDERS)
            .map(|i| {
                let v = core.u1[i * 4];
                v * v
            })
            .sum::<f32>();
        assert!(
            (col0_norm_sq - 1.0).abs() < 1e-4,
            "U1 col0 should be unit-norm, got |.|^2 = {}",
            col0_norm_sq,
        );
    }

    #[test]
    fn reconstruction_error_bound_synthetic() {
        let fp = structured_fingerprint();
        let core = CpuTuckerCompressor
            .compress(&fp, (4, 4, 128))
            .expect("compress succeeds");
        let full = materialize_full_tensor(&fp);
        let reconstructed = reconstruct_tensor(&core);

        let err = relative_frobenius_error(&full, &reconstructed);
        println!(
            "[FSV-16] relative Frobenius error = {:.6} (target < 0.15)",
            err
        );
        assert!(
            err < 0.15,
            "expected reconstruction error < 0.15, got {}",
            err
        );
    }

    #[test]
    fn bincode_roundtrip_tucker_core() {
        let fp = structured_fingerprint();
        let core = CpuTuckerCompressor
            .compress(&fp, (4, 4, 128))
            .expect("compress succeeds");

        let bytes = bincode::serialize(&core).expect("serialize");
        let decoded: TuckerCore = bincode::deserialize(&bytes).expect("deserialize");

        assert_eq!(core.ranks, decoded.ranks);
        assert_eq!(core.data.len(), decoded.data.len());
        assert_eq!(core.u1.len(), decoded.u1.len());
        assert_eq!(core.u2.len(), decoded.u2.len());
        assert_eq!(core.u3.len(), decoded.u3.len());
        // Byte-for-byte equality of the data vectors.
        for (a, b) in core.data.iter().zip(decoded.data.iter()) {
            assert_eq!(a.to_bits(), b.to_bits());
        }
        for (a, b) in core.u1.iter().zip(decoded.u1.iter()) {
            assert_eq!(a.to_bits(), b.to_bits());
        }
        for (a, b) in core.u2.iter().zip(decoded.u2.iter()) {
            assert_eq!(a.to_bits(), b.to_bits());
        }
        for (a, b) in core.u3.iter().zip(decoded.u3.iter()) {
            assert_eq!(a.to_bits(), b.to_bits());
        }
    }

    #[test]
    fn empty_fingerprint_returns_empty_error() {
        let fp = SemanticFingerprint {
            e1_semantic: Vec::new(),
            e2_temporal_recent: Vec::new(),
            e3_temporal_periodic: Vec::new(),
            e4_temporal_positional: Vec::new(),
            e5_causal_as_cause: Vec::new(),
            e5_causal_as_effect: Vec::new(),
            e5_causal: Vec::new(),
            e6_sparse: SparseVector::empty(),
            e7_code: Vec::new(),
            e8_graph_as_source: Vec::new(),
            e8_graph_as_target: Vec::new(),
            e8_graph: Vec::new(),
            e9_hdc: Vec::new(),
            e10_multimodal_paraphrase: Vec::new(),
            e10_multimodal_as_context: Vec::new(),
            e11_entity: Vec::new(),
            e12_late_interaction: Vec::new(),
            e13_splade: SparseVector::empty(),
            e14_bge_m3_dense: Vec::new(),
        };
        let err = CpuTuckerCompressor
            .compress(&fp, (4, 4, 128))
            .expect_err("should reject empty");
        assert!(matches!(err, TuckerError::EmptyFingerprint));
    }

    #[test]
    fn invalid_rank_returns_invalid_rank() {
        let fp = single_direction_fingerprint(0.5);
        // rank_1 = 0
        let err = CpuTuckerCompressor
            .compress(&fp, (0, 4, 128))
            .expect_err("rank_1=0 must fail");
        assert!(matches!(err, TuckerError::InvalidRank { .. }));

        // rank_1 > NUM_EMBEDDERS (NUM_EMBEDDERS=14, so 15 is the first invalid rank)
        let err = CpuTuckerCompressor
            .compress(&fp, (15, 4, 128))
            .expect_err("rank_1>14 must fail");
        assert!(matches!(err, TuckerError::InvalidRank { .. }));

        // rank_3 > REFERENCE_EMBEDDING_DIM
        let err = CpuTuckerCompressor
            .compress(&fp, (4, 4, 1025))
            .expect_err("rank_3>1024 must fail");
        assert!(matches!(err, TuckerError::InvalidRank { .. }));
    }

    #[test]
    fn extract_reference_view_asymmetric_uses_cause_source_paraphrase_sides() {
        let mut fp = SemanticFingerprint {
            e1_semantic: vec![0.0; 1024],
            e2_temporal_recent: vec![0.0; 512],
            e3_temporal_periodic: vec![0.0; 512],
            e4_temporal_positional: vec![0.0; 512],
            e5_causal_as_cause: vec![7.0; 768],
            e5_causal_as_effect: vec![99.0; 768], // should NOT be used
            e5_causal: Vec::new(),
            e6_sparse: SparseVector::empty(),
            e7_code: vec![0.0; 1536],
            e8_graph_as_source: vec![5.0; 1024],
            e8_graph_as_target: vec![88.0; 1024], // should NOT be used
            e8_graph: Vec::new(),
            e9_hdc: vec![0.0; 1024],
            e10_multimodal_paraphrase: vec![3.0; 768],
            e10_multimodal_as_context: vec![77.0; 768], // should NOT be used
            e11_entity: vec![0.0; 768],
            e12_late_interaction: Vec::new(),
            e13_splade: SparseVector::empty(),
            e14_bge_m3_dense: Vec::new(),
        };
        // make lints happy that we don't read the fp later
        fp.e5_causal = Vec::new();

        let views = extract_reference_view(&fp);
        // E5 row (index 4) must be 7.0 in first 768 entries, 0.0 after.
        for d in 0..768 {
            assert_eq!(views[4][d], 7.0, "E5 cause side not used at d={}", d);
        }
        for d in 768..1024 {
            assert_eq!(views[4][d], 0.0, "E5 pad zero expected at d={}", d);
        }
        // E8 row (index 7) must be 5.0 across full 1024.
        for d in 0..1024 {
            assert_eq!(views[7][d], 5.0, "E8 source side not used at d={}", d);
        }
        // E10 row (index 9) must be 3.0 in first 768, 0.0 after.
        for d in 0..768 {
            assert_eq!(views[9][d], 3.0, "E10 paraphrase side not used at d={}", d);
        }
        for d in 768..1024 {
            assert_eq!(views[9][d], 0.0, "E10 pad zero expected at d={}", d);
        }
    }

    #[test]
    fn extract_reference_view_truncates_e7_to_1024() {
        let fp = SemanticFingerprint {
            e1_semantic: Vec::new(),
            e2_temporal_recent: Vec::new(),
            e3_temporal_periodic: Vec::new(),
            e4_temporal_positional: Vec::new(),
            e5_causal_as_cause: Vec::new(),
            e5_causal_as_effect: Vec::new(),
            e5_causal: Vec::new(),
            e6_sparse: SparseVector::empty(),
            e7_code: vec![1.0; 1536],
            e8_graph_as_source: Vec::new(),
            e8_graph_as_target: Vec::new(),
            e8_graph: Vec::new(),
            e9_hdc: Vec::new(),
            e10_multimodal_paraphrase: Vec::new(),
            e10_multimodal_as_context: Vec::new(),
            e11_entity: Vec::new(),
            e12_late_interaction: Vec::new(),
            e13_splade: SparseVector::empty(),
            e14_bge_m3_dense: Vec::new(),
        };
        let views = extract_reference_view(&fp);
        for d in 0..1024 {
            assert_eq!(views[6][d], 1.0, "E7 truncated row must be 1.0 at d={}", d);
        }
        // views[6].len() is 1024 by type, no "index 1024..1536" exists.
    }

    #[test]
    fn extract_reference_view_zero_pads_small_embedder() {
        let fp = SemanticFingerprint {
            e1_semantic: Vec::new(),
            e2_temporal_recent: vec![1.0; 512],
            e3_temporal_periodic: Vec::new(),
            e4_temporal_positional: Vec::new(),
            e5_causal_as_cause: Vec::new(),
            e5_causal_as_effect: Vec::new(),
            e5_causal: Vec::new(),
            e6_sparse: SparseVector::empty(),
            e7_code: Vec::new(),
            e8_graph_as_source: Vec::new(),
            e8_graph_as_target: Vec::new(),
            e8_graph: Vec::new(),
            e9_hdc: Vec::new(),
            e10_multimodal_paraphrase: Vec::new(),
            e10_multimodal_as_context: Vec::new(),
            e11_entity: Vec::new(),
            e12_late_interaction: Vec::new(),
            e13_splade: SparseVector::empty(),
            e14_bge_m3_dense: Vec::new(),
        };
        let views = extract_reference_view(&fp);
        for d in 0..512 {
            assert_eq!(views[1][d], 1.0, "E2 head must be 1.0 at d={}", d);
        }
        for d in 512..1024 {
            assert_eq!(views[1][d], 0.0, "E2 tail must be 0.0 at d={}", d);
        }
    }
}
