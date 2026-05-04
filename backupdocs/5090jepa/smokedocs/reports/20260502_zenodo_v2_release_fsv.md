# 2026-05-02 Zenodo v2 Release FSV

## Result

Status: `passed`

Published release:

- DOI: `10.5281/zenodo.19977981`
- Record: `https://zenodo.org/records/19977981`
- Concept DOI: `10.5281/zenodo.19953950`
- Version: `v2.0.0-rc1`
- Publication date: `2026-05-02`

## Source Of Truth

Primary source of truth:

- Zenodo public record API: `https://zenodo.org/api/records/19977981`
- Zenodo public archive content URL:
  `https://zenodo.org/api/records/19977981/files/mejepa_5090_artifact_v2.0.0_rc1.tar.gz/content`
- Zenodo public checksum content URL:
  `https://zenodo.org/api/records/19977981/files/checksums.txt/content`

Local release source:

- Archive:
  `tmp/mejepa_local_ci_runs/20260502T_release_d_gate_v3_release_archive_v2/mejepa_5090_artifact_v2.0.0_rc1.tar.gz`
- Checksum sidecar:
  `tmp/mejepa_local_ci_runs/20260502T_release_d_gate_v3_release_archive_v2/checksums.txt`
- Zenodo evidence directory:
  `tmp/mejepa_local_ci_runs/20260502T_release_d_gate_v3_release_archive_v2/zenodo_evidence`

## Before State

The new-version draft `19977981` initially inherited the previous v1 files:

- `mejepa_5090_artifact_v1.0.0_rc5_sif_verified.tar.gz`
- `checksums.txt`

Draft state before v2 upload:

- `submitted=false`
- `state=unsubmitted`
- reserved DOI: `10.5281/zenodo.19977981`

Evidence:

- `zenodo_new_version_summary.json`
- `zenodo_draft_before_upload.json`

## Action

The release script:

1. Deleted inherited draft files.
2. Uploaded `mejepa_5090_artifact_v2.0.0_rc1.tar.gz`.
3. Uploaded v2 `checksums.txt`.
4. Updated Zenodo metadata to `v2.0.0-rc1`.
5. Read the draft back.
6. Compared Zenodo file size and MD5 against local files.
7. Published draft `19977981`.

The release script was hardened before upload to compare Zenodo's remote MD5
readback to local MD5 values instead of checking size alone.

## Draft Readback After Upload

Zenodo draft `19977981` contained exactly:

| File | Size | Remote MD5 | Local SHA-256 |
| --- | ---: | --- | --- |
| `mejepa_5090_artifact_v2.0.0_rc1.tar.gz` | 7,430,206,664 | `a2ffd9f63239bd8e2b9eb667ef12650c` | `91461515e29c851017ef9bdd5fb584ed6bc404abedca52be4b8107fb3d4b1a01` |
| `checksums.txt` | 1,403 | `7407fa7a3c08fcfec6748ba1ad747eac` | `92ebd47bf85a37b139ef6645bdbb686c70aa8b421797fd09d2e98442295c8825` |

Evidence:

- `zenodo_draft_upload_readback_summary.json`
- `zenodo_pre_publish_readback_summary.json`

## Published Readback

Public API readback returned:

```json
{
  "id": 19977981,
  "doi": "10.5281/zenodo.19977981",
  "conceptdoi": "10.5281/zenodo.19953950",
  "title": "Dynamic / ME-JEPA v2.0.0-rc1: Audited World-Model Runtime and Verified Training-Corpus Artifact",
  "version": "v2.0.0-rc1",
  "publication_date": "2026-05-02",
  "files": [
    {
      "key": "mejepa_5090_artifact_v2.0.0_rc1.tar.gz",
      "size": 7430206664,
      "checksum": "md5:a2ffd9f63239bd8e2b9eb667ef12650c"
    },
    {
      "key": "checksums.txt",
      "size": 1403,
      "checksum": "md5:7407fa7a3c08fcfec6748ba1ad747eac"
    }
  ]
}
```

## Public Download Verification

The published `checksums.txt` was downloaded from Zenodo and compared
byte-for-byte against the local checksum sidecar. `cmp` returned success.

The published archive was downloaded from Zenodo and verified:

```text
size:   7430206664 bytes
sha256: 91461515e29c851017ef9bdd5fb584ed6bc404abedca52be4b8107fb3d4b1a01
md5:    a2ffd9f63239bd8e2b9eb667ef12650c
cmp:    local archive and downloaded archive are byte-identical
```

This proves the published Zenodo file content matches the local release archive
and the published checksum sidecar.

## Release-D Evidence Summary

Release-D v3 local CI source:

- `tmp/mejepa_local_ci_runs/20260502T_release_d_gate_v3_local_ci_manifest.json`
- status: `passed`
- tier: `release-d`
- exit code: `0`

Aggregate evidence:

- aggregate manifest status: `passed`
- `passed_count=5`
- `failed_count=0`
- aggregate FSV status: `passed`

Container verification:

- Docker verification: `status=ok`, `diff_count=0`
- Apptainer/SIF verification: `status=ok`, `diff_count=0`

Critical hashes:

- archive: `91461515e29c851017ef9bdd5fb584ed6bc404abedca52be4b8107fb3d4b1a01`
- Docker tar: `830304d56887b54d72c34f51c05176f2c39557e97829468fab379f403ae1940b`
- SIF: `78c87c55406996d711f4ecff7c12fe66610dc8d53110766784cf7ed80f386bad`
- reference: `70eebc518feb8786c006cbc02bd4df00bebaa42ac29fdc6998d60da74acd9b57`
- pairwise MI audit JSON:
  `06506ebc1fffcb171cbbe2e9146f1916c5fb7568772710b86aac0bb56e33c064`

## Notes

No Zenodo access token was written to repository files or evidence files. The
token was passed through process stdin for API calls.
