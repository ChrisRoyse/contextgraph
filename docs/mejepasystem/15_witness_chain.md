# 15 — Witness Chain (SHAKE-256 + ed25519 + Merkle)

Source files covered: `crates/context-graph-witness/src/{lib,cert,cert_merkle,cert_tests,sign}.rs`, `crates/context-graph-mejepa-hygiene/src/witness_compress.rs`, `crates/context-graph-mcp/src/handlers/tools/reality_loop/witness_chain.rs`, `docs/ruvectorfindings/specs/constitution.md §security_requirements`.

> **Clean-room reimplementation.** No code copied, no upstream tracking.

---

## 1. Purpose

Tamper-evident append-only provenance chain binding every persistent artifact (panel, training cert, prediction, feedback event, recommendation) to a cryptographic hash chain.

---

## 2. Architecture

| Component | Purpose | Implementation |
|---|---|---|
| **Chain entry** | 73-byte SHAKE-256 record | `lib.rs::ChainEntry` |
| **Stage 0 certificate** | Initial corpus / model genesis seal | `cert.rs::Stage0Cert` |
| **Stage F certificate** | Final attestation per cycle | `cert.rs::StageFCert` |
| **Merkle compression** | Roll-up of segments > 90 days old | `cert_merkle.rs` + `hygiene::witness_compress` |
| **ed25519 segment signing** | Operator-controlled keyring signing | `sign.rs` (feature `ed25519`) |

---

## 3. Chain entry (73 bytes)

```
[ 64 bytes SHAKE-256(parent_hash || payload) | 8 bytes timestamp (u64 BE) | 1 byte op_type ]
```

| Op type | Meaning |
|---|---|
| 0x01 | Panel append |
| 0x02 | Training cert |
| 0x03 | Prediction |
| 0x04 | Agent feedback (SurpriseEvent) |
| 0x05 | Recommendation |
| 0x06 | Harness transition |
| 0x07 | A/B/C promotion |
| 0x08 | Constellation refit |
| 0x09 | GC / hygiene operation |
| 0x0a | Stage 0 (genesis) |
| 0x0f | Stage F (cycle attestation) |
| 0xff | Merkle compression marker |

Hash chain: `entry_hash_{n+1} = SHAKE-256(entry_hash_n || op_type || timestamp || payload_hash)`.

---

## 4. Stage 0 / Stage F certificates

Source: `cert.rs`.

| Cert | Trigger | Binds |
|---|---|---|
| **Stage 0** | Genesis — corpus locked + initial weights | `corpus_sha`, `embedder_versions`, `frozen_at`, `code_version`, `operator_id` |
| **Stage F** | End of training cycle | full cycle metrics + parent witness hash + final theta SHA |

---

## 5. Merkle compression (`hygiene::witness_compress`)

Source: `crates/context-graph-mejepa-hygiene/src/witness_compress.rs`.

After 90 days, the chain rolls up. Each ~256-entry segment becomes one Merkle root entry:

```
witness_compress_old_segments(db, max_age_days=90):
  - Find segments older than 90 days
  - For each segment of ~256 entries:
      - Build binary Merkle tree over entry hashes
      - Persist Merkle root as a single entry with op_type=0xff
      - Original entries archived to /mnt/d/contextgraph/storage/witness/compressed/
  - verify_witness_integrity() validates round-trip
```

MCP tool: `mejepa_witness_compress`. FSV at `crates/context-graph-mejepa-hygiene/examples/merkle_compression_fsv`.

---

## 6. ed25519 segment signing (feature-gated)

Source: `sign.rs` (Cargo feature `ed25519`).

- **Ed25519 keypair** stored in operator-controlled keyring (e.g., GPG agent, `~/.ssh/`).
- **Private key NEVER leaves operator hardware** per `SEC-MEJEPA-03`.
- **Signs every Stage F + every Merkle compression marker** with ed25519-dalek 2.x.
- **Verification public key** stored in `config/witness_pubkey.pem`.

---

## 7. Integrity verification

| Tool / function | Purpose |
|---|---|
| `optimizer_witness_chain_verify` | Verify chain integrity + Merkle commitments; detect tampering |
| `optimizer_witness_chain_diff` | Diff segment between two offsets |
| `optimizer_witness_chain_repair_legacy` | One-time migration from pre-v2 format |
| `audit_verify_old_entry` | Verify a single old entry against the Merkle-compressed proof |
| `verify_witness_integrity` | Full chain replay; called every 10,000 attempts by `heal::integrity` |

`heal::integrity` triggers a verify every 10,000 attempts. On corruption: attempt auto-repair → if fail, **freeze panel writes** + `MEJEPA_OBSERVE_INTEGRITY_VIOLATION` alert.

---

## 8. Storage layout

```
/mnt/d/contextgraph/storage/witness/
├── witness-chain.bin                  # active append-only chain (recent ≤90d)
├── compressed/
│   └── segment-<N>-merkle-<root>.bin  # Merkle-compressed old segments
└── stage-certs/
    ├── stage0-<corpus_sha>.json       # genesis
    └── stagef-<cycle>-<theta_sha>.json # per-cycle attestation
```

---

## 9. Cross-references

[`04_database_schema.md`](04_database_schema.md) (every CF binds witness_hash), [`10_training_loop_utml.md`](10_training_loop_utml.md) (training cert chain), [`12_self_healing_scheduler.md`](12_self_healing_scheduler.md) (integrity check), [`16_hygiene_tier_compression.md`](16_hygiene_tier_compression.md) (Merkle compression).
