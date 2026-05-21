from __future__ import annotations

import hashlib
import os
from dataclasses import dataclass
from pathlib import Path
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.parse import quote
from urllib.request import Request, urlopen

from .core import SwebenchOpsError, utc_now


DEFAULT_PANEL_B_MODELS_ROOT = Path("/zfs/hot/contextgraph/models")
DEFAULT_PANEL_B_FSV_ROOT = Path(
    "/zfs/archive/contextgraph/fsv/task-py-g-111d-panel-b-artifact-materialization-fsv"
)
ALLOWED_MODEL_ROOTS = (
    Path("/zfs/hot/contextgraph"),
    Path("/zfs/archive/contextgraph"),
    Path("/home/croyse/.cache/contextgraph"),
)
USER_AGENT = "contextgraph-panel-b-artifact-materializer/1.0"


@dataclass(frozen=True)
class PanelBSource:
    slot: str
    source_id: str
    revision: str
    relative_dir: str
    scorer_kind: str
    files: tuple[str, ...]

    def root(self, models_root: Path) -> Path:
        return models_root / self.relative_dir


PANEL_B_SOURCES: tuple[PanelBSource, ...] = (
    PanelBSource(
        slot="ast",
        source_id="microsoft/codebert-base",
        revision="3b0952feddeffad0063f274080e3c23d75e7eb39",
        relative_dir="panel-b/codebert-base",
        scorer_kind="codebert_ast_roberta",
        files=("config.json", "merges.txt", "rust_model.ot", "vocab.json"),
    ),
    PanelBSource(
        slot="cfg",
        source_id="microsoft/graphcodebert-base",
        revision="2b0488a7bb0eefc7041f1bb2cad1ab26b0da269d",
        relative_dir="panel-b/graphcodebert-base",
        scorer_kind="graphcodebert_cfg_roberta",
        files=("config.json", "merges.txt", "pytorch_model.bin", "vocab.json"),
    ),
    PanelBSource(
        slot="code_semantic",
        source_id="microsoft/unixcoder-base",
        revision="5604afdc964f6c53782a6813140ade5216b99006",
        relative_dir="panel-b/unixcoder-base",
        scorer_kind="unixcoder_code_semantic",
        files=("config.json", "merges.txt", "pytorch_model.bin", "vocab.json"),
    ),
    PanelBSource(
        slot="text",
        source_id="BAAI/bge-large-en-v1.5",
        revision="d4aa6901d3a41ba39fb536a557fa166f842b0e09",
        relative_dir="panel-b/bge-large-en-v1.5",
        scorer_kind="bge_text_scorer",
        files=("config.json", "tokenizer.json", "model.safetensors"),
    ),
)


def add_arguments(parser: Any) -> None:
    parser.add_argument("--models-root", type=Path, default=DEFAULT_PANEL_B_MODELS_ROOT)
    parser.add_argument("--evidence-root", type=Path, default=DEFAULT_PANEL_B_FSV_ROOT)
    parser.add_argument(
        "--download",
        action="store_true",
        help="Download all pinned files atomically. Without this, only writes an availability plan.",
    )
    parser.add_argument(
        "--reuse-existing",
        action="store_true",
        help="Reuse non-empty existing files instead of replacing them from the pinned source URL.",
    )
    parser.add_argument(
        "--probe-remote",
        action="store_true",
        help="HEAD-probe every pinned Hugging Face URL without downloading payloads.",
    )
    parser.add_argument("--timeout-seconds", type=int, default=1200)
    parser.add_argument("--chunk-bytes", type=int, default=8 * 1024 * 1024)


def command_panel_b_artifacts(args: Any) -> dict[str, Any]:
    models_root = args.models_root.expanduser().resolve(strict=False)
    validate_models_root(models_root)
    if args.timeout_seconds <= 0:
        raise SwebenchOpsError(
            "PANEL_B_ARTIFACT_TIMEOUT_INVALID",
            "--timeout-seconds must be positive",
            "Pass a positive timeout large enough for Hugging Face artifact downloads.",
            {"timeout_seconds": args.timeout_seconds},
        )
    if args.chunk_bytes <= 0:
        raise SwebenchOpsError(
            "PANEL_B_ARTIFACT_CHUNK_BYTES_INVALID",
            "--chunk-bytes must be positive",
            "Pass a positive streaming chunk size.",
            {"chunk_bytes": args.chunk_bytes},
        )

    remote_probe = bool(args.probe_remote or args.download)
    source_reports = []
    required_file_count = 0
    present_file_count = 0
    downloaded_file_count = 0
    missing_files: list[str] = []
    remote_failures: list[dict[str, Any]] = []

    for source in PANEL_B_SOURCES:
        source_root = source.root(models_root)
        file_reports = []
        for filename in source.files:
            required_file_count += 1
            target = source_root / filename
            url = hf_resolve_url(source, filename)
            remote = probe_remote(url, args.timeout_seconds) if remote_probe else None
            if remote is not None and not remote["ok"]:
                remote_failures.append(
                    {
                        "sourceId": source.source_id,
                        "revision": source.revision,
                        "file": filename,
                        "status": remote.get("status"),
                        "error": remote.get("error"),
                    }
                )

            downloaded = False
            if args.download and (not target.is_file() or not args.reuse_existing):
                download_result = download_file(url, target, args.timeout_seconds, args.chunk_bytes)
                downloaded = True
                downloaded_file_count += 1
                size = download_result["sizeBytes"]
                sha256 = download_result["sha256"]
            elif target.is_file():
                size = target.stat().st_size
                sha256 = sha256_file_hex(target) if size > 0 else None
            else:
                size = 0
                sha256 = None

            present = target.is_file() and size > 0
            if present:
                present_file_count += 1
            else:
                missing_files.append(str(target))

            file_reports.append(
                {
                    "name": filename,
                    "path": str(target),
                    "sourceUrl": url,
                    "present": present,
                    "sizeBytes": size,
                    "sha256": sha256,
                    "downloadedThisRun": downloaded,
                    "remote": remote,
                }
            )

        source_reports.append(
            {
                "slot": source.slot,
                "sourceId": source.source_id,
                "revision": source.revision,
                "root": str(source_root),
                "scorerKind": source.scorer_kind,
                "files": file_reports,
            }
        )

    all_present = present_file_count == required_file_count
    payload = {
        "checked_at": utc_now(),
        "artifactKind": "task_py_g_111d_panel_b_artifact_materialization",
        "githubIssue": 572,
        "ok": (all_present and not remote_failures) if args.download else True,
        "downloadAttempted": bool(args.download),
        "reuseExisting": bool(args.reuse_existing),
        "remoteProbeAttempted": remote_probe,
        "modelsRoot": str(models_root),
        "requiredArtifactCount": len(PANEL_B_SOURCES),
        "requiredFileCount": required_file_count,
        "presentFileCount": present_file_count,
        "downloadedFileCount": downloaded_file_count,
        "missingFileCount": len(missing_files),
        "allArtifactsPresent": all_present,
        "missingFiles": missing_files,
        "remoteFailures": remote_failures,
        "sources": source_reports,
        "contractOwner": "crates/context-graph-mejepa-instruments/src/panel_b.rs::default_panel_b_artifact_specs",
        "nextVerification": "cargo run -p context-graph-mejepa-instruments --example task_py_g_111b_panel_b_artifact_preflight_fsv",
        "activePathPolicy": {"mntDActiveReadsOrWrites": False},
    }
    if args.download and (not all_present or remote_failures):
        raise SwebenchOpsError(
            "PANEL_B_ARTIFACT_MATERIALIZATION_INCOMPLETE",
            "Panel B artifact materialization did not produce every required file",
            "Fix the listed source/download failures and rerun this command on aiwonder.",
            payload,
        )
    return payload


def validate_models_root(models_root: Path) -> None:
    if str(models_root).startswith("/mnt/d/"):
        raise SwebenchOpsError(
            "PANEL_B_ARTIFACT_MNT_D_FORBIDDEN",
            f"Panel B models root cannot be on mounted D drive: {models_root}",
            "Use /zfs/hot/contextgraph/models on aiwonder.",
            {"models_root": str(models_root)},
        )
    if not any(models_root.is_relative_to(root) for root in ALLOWED_MODEL_ROOTS):
        raise SwebenchOpsError(
            "PANEL_B_ARTIFACT_ROOT_NOT_AIWONDER",
            f"Panel B models root is outside approved aiwonder roots: {models_root}",
            "Use /zfs/hot/contextgraph/models, /zfs/archive/contextgraph, or the explicit aiwonder cache root.",
            {
                "models_root": str(models_root),
                "allowed_roots": [str(root) for root in ALLOWED_MODEL_ROOTS],
            },
        )


def hf_resolve_url(source: PanelBSource, filename: str) -> str:
    return (
        f"https://huggingface.co/{source.source_id}/resolve/"
        f"{source.revision}/{quote(filename, safe='/')}"
    )


def request_headers() -> dict[str, str]:
    headers = {"User-Agent": USER_AGENT}
    token = os.environ.get("HF_TOKEN") or os.environ.get("HF_HUB_TOKEN")
    if token:
        headers["Authorization"] = f"Bearer {token}"
    return headers


def probe_remote(url: str, timeout_seconds: int) -> dict[str, Any]:
    request = Request(url, headers=request_headers(), method="HEAD")
    try:
        with urlopen(request, timeout=timeout_seconds) as response:
            return {
                "ok": 200 <= response.status < 400,
                "status": response.status,
                "contentLength": int(response.headers["Content-Length"])
                if response.headers.get("Content-Length")
                else None,
                "etag": strip_quotes(response.headers.get("ETag")),
            }
    except HTTPError as exc:
        return {
            "ok": False,
            "status": exc.code,
            "contentLength": None,
            "etag": None,
            "error": str(exc),
        }
    except URLError as exc:
        return {
            "ok": False,
            "status": None,
            "contentLength": None,
            "etag": None,
            "error": str(exc.reason),
        }


def download_file(url: str, target: Path, timeout_seconds: int, chunk_bytes: int) -> dict[str, Any]:
    target.parent.mkdir(parents=True, exist_ok=True)
    temp = target.with_name(f"{target.name}.part-{os.getpid()}")
    if temp.exists():
        temp.unlink()
    request = Request(url, headers=request_headers(), method="GET")
    hasher = hashlib.sha256()
    total = 0
    try:
        with urlopen(request, timeout=timeout_seconds) as response:
            with temp.open("wb") as handle:
                while True:
                    chunk = response.read(chunk_bytes)
                    if not chunk:
                        break
                    total += len(chunk)
                    hasher.update(chunk)
                    handle.write(chunk)
                handle.flush()
                os.fsync(handle.fileno())
            content_length = response.headers.get("Content-Length")
            if content_length is not None and total != int(content_length):
                temp.unlink(missing_ok=True)
                raise SwebenchOpsError(
                    "PANEL_B_ARTIFACT_DOWNLOAD_SIZE_MISMATCH",
                    f"downloaded byte count does not match Content-Length for {target}",
                    "Retry the download; if it repeats, inspect the upstream artifact response.",
                    {
                        "target": str(target),
                        "downloaded_bytes": total,
                        "content_length": int(content_length),
                    },
                )
        temp.replace(target)
        return {"sizeBytes": total, "sha256": hasher.hexdigest()}
    except SwebenchOpsError:
        raise
    except (HTTPError, URLError, OSError) as exc:
        temp.unlink(missing_ok=True)
        raise SwebenchOpsError(
            "PANEL_B_ARTIFACT_DOWNLOAD_FAILED",
            f"failed to download Panel B artifact {target}: {exc}",
            "Fix network/source credentials/storage and rerun on aiwonder.",
            {"target": str(target), "url": url, "error": str(exc)},
        ) from exc


def sha256_file_hex(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def strip_quotes(value: str | None) -> str | None:
    if value is None:
        return None
    return value.strip().strip('"')
