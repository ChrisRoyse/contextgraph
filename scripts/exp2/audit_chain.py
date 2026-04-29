"""Audit-chain writer for EXP-2 artefacts."""

from __future__ import annotations

import subprocess
from pathlib import Path
from typing import Any

from .common import Exp2Error, canonical_json, repo_path, sha256_bytes, sha256_file, utc_now, write_json


class AuditChain:
    """Append-only provenance log for EXP-2 artefacts."""

    def __init__(self, run_id: str, output_path: str | Path, seed: int) -> None:
        self.run_id = run_id
        self.output_path = repo_path(output_path)
        self.seed = seed
        self.artefacts: list[dict[str, Any]] = []
        self.events: list[dict[str, Any]] = []
        self.created_at = utc_now()

    def add_event(self, event_type: str, payload: dict[str, Any]) -> None:
        self.events.append(
            {"event_type": event_type, "timestamp": utc_now(), "payload": payload}
        )

    def add_artefact(
        self,
        artefact_path: str | Path,
        label: str,
        parent_artefact_ids: list[str] | None = None,
        created_by: str | None = None,
    ) -> str:
        path = repo_path(artefact_path)
        artefact_id = f"art_{len(self.artefacts) + 1:04d}"
        self.artefacts.append(
            {
                "artefact_id": artefact_id,
                "label": label,
                "path": str(path.relative_to(repo_path("."))),
                "sha256": sha256_file(path),
                "parent_artefact_ids": parent_artefact_ids or [],
                "created_by": created_by,
                "created_at": utc_now(),
            }
        )
        return artefact_id

    def finalize(self, hyperparameters_sha256: str | None = None) -> str:
        chain = {
            "run_id": self.run_id,
            "creation_timestamp": self.created_at,
            "finalized_at": utc_now(),
            "seed": self.seed,
            "code_commit_sha": self._git_commit(),
            "hyperparameters_sha256": hyperparameters_sha256,
            "artefacts": self.artefacts,
            "events": self.events,
        }
        chain["chain_sha256"] = sha256_bytes(canonical_json(chain))
        write_json(self.output_path, chain)
        return chain["chain_sha256"]

    @staticmethod
    def _git_commit() -> str:
        try:
            return subprocess.check_output(
                ["git", "rev-parse", "HEAD"], cwd=repo_path("."), text=True
            ).strip()
        except Exception as exc:
            raise Exp2Error(f"Unable to resolve git commit for audit chain: {exc}") from exc
