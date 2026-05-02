#!/usr/bin/env python3
"""Publish the ME-JEPA v2 release archive to Zenodo without storing tokens.

The script reads the access token from stdin or from ZENODO_ACCESS_TOKEN,
prints only sanitized record state, and writes full API readbacks to explicit
evidence files. It intentionally fails closed on missing files, checksum drift,
or unexpected Zenodo state.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
import time
from pathlib import Path
from typing import Any

import requests


class ZenodoError(RuntimeError):
    pass


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
      for chunk in iter(lambda: handle.read(1024 * 1024), b""):
        digest.update(chunk)
    return digest.hexdigest()


def md5_file(path: Path) -> str:
    digest = hashlib.md5(usedforsecurity=False)
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_token(token_stdin: bool) -> str:
    token = os.environ.get("ZENODO_ACCESS_TOKEN", "")
    if token_stdin:
        token = sys.stdin.readline().strip()
    if not token:
        raise ZenodoError("ZENODO_TOKEN_MISSING: provide --token-stdin or ZENODO_ACCESS_TOKEN")
    return token


def request_json(
    session: requests.Session,
    method: str,
    url: str,
    *,
    expected: set[int],
    **kwargs: Any,
) -> tuple[int, Any]:
    response = session.request(method, url, timeout=120, **kwargs)
    text = response.text
    if response.status_code not in expected:
        raise ZenodoError(
            f"ZENODO_HTTP_{response.status_code}: {method} {url} failed: {text[:2000]}"
        )
    if not text.strip():
        return response.status_code, None
    return response.status_code, response.json()


def sanitize_record(record: dict[str, Any]) -> dict[str, Any]:
    metadata = record.get("metadata", {})
    files = record.get("files", [])
    if isinstance(files, dict):
        file_entries = files.get("entries", {})
        files_summary = [
            {
                "key": key,
                "size": value.get("size"),
                "checksum": value.get("checksum"),
            }
            for key, value in sorted(file_entries.items())
        ]
    else:
        files_summary = [
            {
                "filename": item.get("filename") or item.get("key"),
                "filesize": item.get("filesize") or item.get("size"),
                "checksum": item.get("checksum"),
                "id": item.get("id"),
            }
            for item in files
        ]
    return {
        "id": record.get("id"),
        "conceptrecid": record.get("conceptrecid") or record.get("parent", {}).get("id"),
        "doi": record.get("doi") or metadata.get("prereserve_doi", {}).get("doi"),
        "conceptdoi": record.get("conceptdoi"),
        "submitted": record.get("submitted"),
        "state": record.get("state"),
        "title": metadata.get("title"),
        "version": metadata.get("version"),
        "publication_date": metadata.get("publication_date"),
        "bucket": record.get("links", {}).get("bucket"),
        "latest_draft": record.get("links", {}).get("latest_draft"),
        "html": record.get("links", {}).get("html") or record.get("links", {}).get("self_html"),
        "files": files_summary,
    }


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def latest_deposition(session: requests.Session, base_url: str, record_id: str) -> dict[str, Any]:
    _, record = request_json(
        session,
        "GET",
        f"{base_url}/api/deposit/depositions/{record_id}",
        expected={200},
    )
    return record


def create_or_get_new_version(
    session: requests.Session,
    base_url: str,
    record_id: str,
) -> dict[str, Any]:
    status, record = request_json(
        session,
        "POST",
        f"{base_url}/api/deposit/depositions/{record_id}/actions/newversion",
        expected={201, 202},
    )
    latest_draft = record.get("links", {}).get("latest_draft")
    if latest_draft:
        _, draft = request_json(session, "GET", latest_draft, expected={200})
        return draft
    if status in {201, 202} and record.get("submitted") is False:
        return record
    raise ZenodoError("ZENODO_NEW_VERSION_DRAFT_LINK_MISSING: response did not identify a draft")


def list_files(record: dict[str, Any]) -> list[dict[str, Any]]:
    files = record.get("files", [])
    if not isinstance(files, list):
        raise ZenodoError("ZENODO_LEGACY_FILES_EXPECTED: deposition files were not a list")
    return files


def delete_all_files(session: requests.Session, base_url: str, draft: dict[str, Any]) -> None:
    draft_id = str(draft["id"])
    for file_info in list_files(draft):
        file_id = file_info.get("id")
        filename = file_info.get("filename")
        if not file_id:
            raise ZenodoError(f"ZENODO_FILE_ID_MISSING: draft file lacks id: {file_info}")
        response = session.delete(
            f"{base_url}/api/deposit/depositions/{draft_id}/files/{file_id}",
            timeout=120,
        )
        if response.status_code not in {204, 200}:
            raise ZenodoError(
                f"ZENODO_DELETE_FILE_FAILED: {filename} status={response.status_code} body={response.text[:1000]}"
            )


def upload_file_to_bucket(
    session: requests.Session,
    bucket_url: str,
    path: Path,
    remote_name: str,
) -> dict[str, Any]:
    size = path.stat().st_size
    started = time.time()
    print(f"upload_start name={remote_name} size={size}", flush=True)
    with path.open("rb") as handle:
        response = session.put(f"{bucket_url}/{remote_name}", data=handle, timeout=None)
    elapsed = time.time() - started
    if response.status_code not in {200, 201}:
        raise ZenodoError(
            f"ZENODO_UPLOAD_FAILED: {remote_name} status={response.status_code} body={response.text[:2000]}"
        )
    print(f"upload_done name={remote_name} seconds={elapsed:.1f}", flush=True)
    return response.json()


def update_metadata(
    session: requests.Session,
    base_url: str,
    draft_id: str,
    metadata: dict[str, Any],
) -> dict[str, Any]:
    _, updated = request_json(
        session,
        "PUT",
        f"{base_url}/api/deposit/depositions/{draft_id}",
        expected={200},
        json={"metadata": metadata},
    )
    return updated


def edit_published_record(
    session: requests.Session,
    base_url: str,
    record_id: str,
) -> dict[str, Any]:
    _, draft = request_json(
        session,
        "POST",
        f"{base_url}/api/deposit/depositions/{record_id}/actions/edit",
        expected={201, 202},
    )
    return draft


def publish(session: requests.Session, base_url: str, draft_id: str) -> dict[str, Any]:
    _, published = request_json(
        session,
        "POST",
        f"{base_url}/api/deposit/depositions/{draft_id}/actions/publish",
        expected={202, 201, 200},
    )
    return published


def verify_metadata_readback(record: dict[str, Any], expected_metadata: dict[str, Any]) -> None:
    metadata = record.get("metadata", {})
    for field in ["title", "description", "version", "publication_date", "access_right", "license"]:
        if metadata.get(field) != expected_metadata.get(field):
            raise ZenodoError(
                f"ZENODO_METADATA_READBACK_DRIFT: {field} expected={expected_metadata.get(field)!r} actual={metadata.get(field)!r}"
            )
    if metadata.get("keywords") != expected_metadata.get("keywords"):
        raise ZenodoError("ZENODO_METADATA_KEYWORDS_DRIFT")


def public_record_readback(
    base_url: str,
    record_id: str,
    expected_metadata: dict[str, Any],
) -> dict[str, Any]:
    last_record: dict[str, Any] | None = None
    for _ in range(24):
        response = requests.get(f"{base_url}/api/records/{record_id}", timeout=60)
        if response.status_code == 200:
            record = response.json()
            last_record = record
            metadata = record.get("metadata", {})
            if (
                metadata.get("title") == expected_metadata.get("title")
                and metadata.get("description") == expected_metadata.get("description")
                and metadata.get("version") == expected_metadata.get("version")
            ):
                return record
        time.sleep(5)
    raise ZenodoError(
        "ZENODO_PUBLIC_METADATA_READBACK_TIMEOUT: public record did not expose updated metadata"
        if last_record is None
        else f"ZENODO_PUBLIC_METADATA_READBACK_TIMEOUT: last title={last_record.get('metadata', {}).get('title')!r}"
    )


def verify_uploaded_files(
    record: dict[str, Any],
    expected_files: dict[str, dict[str, Any]],
) -> None:
    files_by_name: dict[str, dict[str, Any]] = {}
    for item in list_files(record):
        filename = item.get("filename")
        if filename:
            files_by_name[filename] = item
    missing = sorted(set(expected_files) - set(files_by_name))
    if missing:
        raise ZenodoError(f"ZENODO_UPLOAD_MISSING_FILES: {missing}")
    for name, expected in expected_files.items():
        actual = files_by_name[name]
        actual_size = actual.get("filesize")
        if actual_size != expected["size"]:
            raise ZenodoError(
                f"ZENODO_UPLOAD_SIZE_DRIFT: {name} expected={expected['size']} actual={actual_size}"
            )
        checksum = actual.get("checksum") or ""
        if checksum.startswith("md5:"):
            actual_md5 = checksum.removeprefix("md5:")
        elif checksum:
            actual_md5 = checksum
        else:
            raise ZenodoError(f"ZENODO_UPLOAD_CHECKSUM_MISSING: {name}")
        if actual_md5 != expected["md5"]:
            raise ZenodoError(
                f"ZENODO_UPLOAD_MD5_DRIFT: {name} expected={expected['md5']} actual={actual_md5}"
            )


def cmd_new_version(args: argparse.Namespace) -> None:
    token = read_token(args.token_stdin)
    session = requests.Session()
    session.headers.update({"Authorization": f"Bearer {token}"})
    draft = create_or_get_new_version(session, args.base_url, args.record_id)
    write_json(Path(args.output), draft)
    sanitized = sanitize_record(draft)
    write_json(Path(args.summary_output), sanitized)
    print(json.dumps(sanitized, indent=2, sort_keys=True))


def cmd_publish_release(args: argparse.Namespace) -> None:
    archive = Path(args.archive)
    checksums = Path(args.checksums)
    metadata_path = Path(args.metadata)
    for path in [archive, checksums, metadata_path]:
        if not path.is_file():
            raise ZenodoError(f"ZENODO_INPUT_MISSING: {path}")
    metadata = json.loads(metadata_path.read_text())
    token = read_token(args.token_stdin)
    session = requests.Session()
    session.headers.update({"Authorization": f"Bearer {token}"})

    _, draft = request_json(
        session,
        "GET",
        f"{args.base_url}/api/deposit/depositions/{args.draft_id}",
        expected={200},
    )
    if draft.get("submitted") is not False:
        raise ZenodoError("ZENODO_DRAFT_ALREADY_SUBMITTED: refusing to modify a published deposition")
    write_json(Path(args.evidence_dir) / "zenodo_draft_before_upload.json", draft)
    delete_all_files(session, args.base_url, draft)
    _, draft_after_delete = request_json(
        session,
        "GET",
        f"{args.base_url}/api/deposit/depositions/{args.draft_id}",
        expected={200},
    )
    write_json(Path(args.evidence_dir) / "zenodo_draft_after_delete.json", draft_after_delete)
    if list_files(draft_after_delete):
        raise ZenodoError("ZENODO_DRAFT_FILES_NOT_DELETED: old files remain after delete")

    bucket_url = draft_after_delete.get("links", {}).get("bucket")
    if not bucket_url:
        raise ZenodoError("ZENODO_BUCKET_MISSING: draft did not provide links.bucket")

    upload_results = [
        upload_file_to_bucket(session, bucket_url, archive, archive.name),
        upload_file_to_bucket(session, bucket_url, checksums, checksums.name),
    ]
    write_json(Path(args.evidence_dir) / "zenodo_upload_results.json", upload_results)
    updated = update_metadata(session, args.base_url, args.draft_id, metadata)
    write_json(Path(args.evidence_dir) / "zenodo_draft_after_metadata.json", updated)

    _, readback = request_json(
        session,
        "GET",
        f"{args.base_url}/api/deposit/depositions/{args.draft_id}",
        expected={200},
    )
    expected_files = {
        archive.name: {
            "size": archive.stat().st_size,
            "sha256": sha256_file(archive),
            "md5": md5_file(archive),
        },
        checksums.name: {
            "size": checksums.stat().st_size,
            "sha256": sha256_file(checksums),
            "md5": md5_file(checksums),
        },
    }
    verify_uploaded_files(readback, expected_files)
    readback_summary = sanitize_record(readback)
    readback_summary["expected_files"] = expected_files
    write_json(Path(args.evidence_dir) / "zenodo_draft_upload_readback_summary.json", readback_summary)
    if args.publish:
        published = publish(session, args.base_url, args.draft_id)
        write_json(Path(args.evidence_dir) / "zenodo_publish_response.json", published)
        print(json.dumps(sanitize_record(published), indent=2, sort_keys=True))
    else:
        print(json.dumps(readback_summary, indent=2, sort_keys=True))


def cmd_publish_draft(args: argparse.Namespace) -> None:
    archive = Path(args.archive)
    checksums = Path(args.checksums)
    for path in [archive, checksums]:
        if not path.is_file():
            raise ZenodoError(f"ZENODO_INPUT_MISSING: {path}")
    token = read_token(args.token_stdin)
    session = requests.Session()
    session.headers.update({"Authorization": f"Bearer {token}"})

    _, draft = request_json(
        session,
        "GET",
        f"{args.base_url}/api/deposit/depositions/{args.draft_id}",
        expected={200},
    )
    if draft.get("submitted") is not False:
        raise ZenodoError("ZENODO_DRAFT_ALREADY_SUBMITTED: refusing to publish non-draft deposition")

    expected_files = {
        archive.name: {
            "size": archive.stat().st_size,
            "sha256": sha256_file(archive),
            "md5": md5_file(archive),
        },
        checksums.name: {
            "size": checksums.stat().st_size,
            "sha256": sha256_file(checksums),
            "md5": md5_file(checksums),
        },
    }
    verify_uploaded_files(draft, expected_files)
    readback_summary = sanitize_record(draft)
    readback_summary["expected_files"] = expected_files
    write_json(Path(args.evidence_dir) / "zenodo_pre_publish_readback_summary.json", readback_summary)

    published = publish(session, args.base_url, args.draft_id)
    write_json(Path(args.evidence_dir) / "zenodo_publish_response.json", published)
    published_summary = sanitize_record(published)
    write_json(Path(args.evidence_dir) / "zenodo_publish_summary.json", published_summary)
    print(json.dumps(published_summary, indent=2, sort_keys=True))


def cmd_edit_metadata(args: argparse.Namespace) -> None:
    metadata_path = Path(args.metadata)
    if not metadata_path.is_file():
        raise ZenodoError(f"ZENODO_INPUT_MISSING: {metadata_path}")
    metadata = json.loads(metadata_path.read_text())
    token = read_token(args.token_stdin)
    session = requests.Session()
    session.headers.update({"Authorization": f"Bearer {token}"})

    _, before = request_json(
        session,
        "GET",
        f"{args.base_url}/api/deposit/depositions/{args.record_id}",
        expected={200},
    )
    write_json(Path(args.evidence_dir) / "zenodo_metadata_edit_before.json", before)

    if before.get("state") == "inprogress":
        draft = before
    else:
        draft = edit_published_record(session, args.base_url, args.record_id)
    write_json(Path(args.evidence_dir) / "zenodo_metadata_edit_draft.json", draft)
    draft_id = str(draft.get("id") or args.record_id)
    if draft.get("state") != "inprogress":
        raise ZenodoError(
            f"ZENODO_METADATA_EDIT_DRAFT_NOT_OPEN: expected state=inprogress, got {draft.get('state')!r}"
        )

    updated = update_metadata(session, args.base_url, draft_id, metadata)
    write_json(Path(args.evidence_dir) / "zenodo_metadata_edit_after_put.json", updated)
    verify_metadata_readback(updated, metadata)

    published = publish(session, args.base_url, draft_id)
    write_json(Path(args.evidence_dir) / "zenodo_metadata_edit_publish_response.json", published)
    public_readback = public_record_readback(args.base_url, args.record_id, metadata)
    write_json(Path(args.evidence_dir) / "zenodo_metadata_edit_public_readback.json", public_readback)
    summary = sanitize_record(public_readback)
    summary["description_length"] = len(public_readback.get("metadata", {}).get("description") or "")
    write_json(Path(args.evidence_dir) / "zenodo_metadata_edit_summary.json", summary)
    print(json.dumps(summary, indent=2, sort_keys=True))


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base-url", default="https://zenodo.org")
    parser.add_argument("--token-stdin", action="store_true")
    subparsers = parser.add_subparsers(dest="cmd", required=True)

    new_version = subparsers.add_parser("new-version")
    new_version.add_argument("--record-id", required=True)
    new_version.add_argument("--output", required=True)
    new_version.add_argument("--summary-output", required=True)
    new_version.set_defaults(func=cmd_new_version)

    publish_release = subparsers.add_parser("publish-release")
    publish_release.add_argument("--draft-id", required=True)
    publish_release.add_argument("--archive", required=True)
    publish_release.add_argument("--checksums", required=True)
    publish_release.add_argument("--metadata", required=True)
    publish_release.add_argument("--evidence-dir", required=True)
    publish_release.add_argument("--publish", action="store_true")
    publish_release.set_defaults(func=cmd_publish_release)

    publish_draft = subparsers.add_parser("publish-draft")
    publish_draft.add_argument("--draft-id", required=True)
    publish_draft.add_argument("--archive", required=True)
    publish_draft.add_argument("--checksums", required=True)
    publish_draft.add_argument("--evidence-dir", required=True)
    publish_draft.set_defaults(func=cmd_publish_draft)

    edit_metadata = subparsers.add_parser("edit-metadata")
    edit_metadata.add_argument("--record-id", required=True)
    edit_metadata.add_argument("--metadata", required=True)
    edit_metadata.add_argument("--evidence-dir", required=True)
    edit_metadata.set_defaults(func=cmd_edit_metadata)
    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    try:
        args.func(args)
    except Exception as exc:  # noqa: BLE001 - CLI evidence needs exact failure.
        print(f"{type(exc).__name__}: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
