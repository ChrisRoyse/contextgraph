"""Deterministic PyTorch InfoNCE lower-bound estimator."""

from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np
import torch
from torch import nn

from scripts.exp2.common import Exp2Error, repo_path, set_global_seed


class TwoTowerCritic(nn.Module):
    def __init__(self, dim: int, hidden_dim: int = 64) -> None:
        super().__init__()

        def tower() -> nn.Sequential:
            return nn.Sequential(
                nn.Linear(dim, hidden_dim),
                nn.LayerNorm(hidden_dim),
                nn.ReLU(),
                nn.Linear(hidden_dim, hidden_dim),
                nn.LayerNorm(hidden_dim),
                nn.ReLU(),
            )

        self.u = tower()
        self.v = tower()
        self.scale = hidden_dim**0.5

    def logits(self, u: torch.Tensor, v: torch.Tensor) -> torch.Tensor:
        return self.u(u) @ self.v(v).T / self.scale


@dataclass
class InfoNCEResult:
    estimate_nats: float
    train_median_last: float
    val_median_last: float
    train_val_gap: float
    train_history: list[float]
    val_history: list[float]
    stopped_step: int


class InfoNCEEstimator:
    def __init__(self, hyperparams: dict[str, Any], seed: int = 42, device: str | None = None) -> None:
        self.hp = hyperparams
        self.seed = seed
        requested = device or hyperparams.get("device", "cuda")
        if requested == "cuda" and not torch.cuda.is_available():
            raise Exp2Error("InfoNCE requested CUDA but torch.cuda.is_available() is false")
        self.device = torch.device(requested)
        self.critic: TwoTowerCritic | None = None

    def fit(self, u_np: np.ndarray, v_np: np.ndarray) -> InfoNCEResult:
        set_global_seed(self.seed)
        u = self._tensor(u_np, "u")
        v = self._tensor(v_np, "v")
        if u.shape != v.shape:
            raise Exp2Error(f"InfoNCE requires equal projected dimensions, got {u.shape} and {v.shape}")
        n, dim = u.shape
        split = int(n * float(self.hp["train_val_split"]))
        if split <= 1 or split >= n:
            raise Exp2Error(f"Invalid train/val split for n={n}: {split}")
        gen = torch.Generator(device="cpu").manual_seed(self.seed)
        perm = torch.randperm(n, generator=gen)
        train_idx = perm[:split].to(self.device)
        val_idx = perm[split:].to(self.device)
        self.critic = TwoTowerCritic(dim, int(self.hp["critic_hidden_dim"])).to(self.device)
        opt = torch.optim.AdamW(
            self.critic.parameters(),
            lr=float(self.hp["lr"]),
            betas=(float(self.hp.get("beta1", 0.9)), float(self.hp.get("beta2", 0.98))),
            weight_decay=float(self.hp["weight_decay"]),
        )
        train_steps = int(self.hp["train_steps"])
        warmup = int(self.hp["warmup_steps"])
        batch_size = int(self.hp["batch_size"])
        patience = int(self.hp["early_stopping_patience"])
        best_val = -1e30
        best_step = 0
        best_state: dict[str, torch.Tensor] | None = None
        train_hist: list[float] = []
        val_hist: list[float] = []
        for step in range(train_steps):
            lr_scale = min(1.0, (step + 1) / max(1, warmup))
            if step >= warmup:
                progress = (step - warmup) / max(1, train_steps - warmup)
                lr_scale *= 0.5 * (1.0 + np.cos(np.pi * progress))
            for group in opt.param_groups:
                group["lr"] = float(self.hp["lr"]) * lr_scale
            choice = train_idx[torch.randint(0, len(train_idx), (batch_size,), generator=torch.Generator(device=self.device).manual_seed(self.seed + step), device=self.device)]
            estimate = self._estimate_tensor(u[choice], v[choice])
            loss = -estimate
            opt.zero_grad(set_to_none=True)
            loss.backward()
            opt.step()
            if step % 10 == 0 or step == train_steps - 1:
                with torch.no_grad():
                    train_val = float(self._estimate_tensor(u[train_idx], v[train_idx]).cpu())
                    val_val = float(self._estimate_tensor(u[val_idx], v[val_idx]).cpu())
                train_hist.append(train_val)
                val_hist.append(val_val)
                if val_val > best_val:
                    best_val = val_val
                    best_step = step
                    best_state = {k: t.detach().cpu().clone() for k, t in self.critic.state_dict().items()}
                if step - best_step > patience:
                    break
        if best_state is not None:
            self.critic.load_state_dict(best_state)
        tail = min(100, len(val_hist))
        train_median = float(np.median(train_hist[-tail:]))
        val_median = float(np.median(val_hist[-tail:]))
        gap = train_median - val_median
        if gap > float(self.hp["overfit_gap_threshold_nats"]):
            raise Exp2Error(f"InfoNCE overfit gap {gap:.6f} exceeds threshold")
        return InfoNCEResult(val_median, train_median, val_median, gap, train_hist, val_hist, best_step)

    def estimate(self, u_np: np.ndarray, v_np: np.ndarray) -> float:
        if self.critic is None:
            raise Exp2Error("InfoNCE critic is not fitted")
        u = self._tensor(u_np, "u")
        v = self._tensor(v_np, "v")
        with torch.no_grad():
            return float(self._estimate_tensor(u, v).cpu())

    def save(self, path: str | Path) -> None:
        if self.critic is None:
            raise Exp2Error("Cannot save unfitted InfoNCE critic")
        dst = repo_path(path)
        dst.parent.mkdir(parents=True, exist_ok=True)
        torch.save({"state_dict": self.critic.state_dict(), "hp": self.hp, "seed": self.seed}, dst)

    def load(self, path: str | Path, dim: int) -> None:
        data = torch.load(repo_path(path), map_location=self.device)
        self.critic = TwoTowerCritic(dim, int(data["hp"]["critic_hidden_dim"])).to(self.device)
        self.critic.load_state_dict(data["state_dict"])
        self.critic.eval()

    def _tensor(self, x: np.ndarray, label: str) -> torch.Tensor:
        arr = np.asarray(x, dtype=np.float32)
        if arr.ndim != 2 or not np.isfinite(arr).all():
            raise Exp2Error(f"{label}: invalid tensor shape/values {arr.shape}")
        return torch.from_numpy(arr).to(self.device)

    def _estimate_tensor(self, u: torch.Tensor, v: torch.Tensor) -> torch.Tensor:
        assert self.critic is not None
        logits = self.critic.logits(u, v)
        diag = torch.diagonal(logits)
        return torch.log(torch.tensor(float(logits.shape[0]), device=logits.device)) + torch.mean(
            diag - torch.logsumexp(logits, dim=1)
        )
