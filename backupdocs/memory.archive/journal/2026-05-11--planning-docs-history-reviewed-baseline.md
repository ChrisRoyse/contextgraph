# 2026-05-11 - Planning docs history-reviewed baseline update

Reviewed git history before updating futurebuild planning docs.

History anchors:

- `e628fb5b` shipped MCP daemon supervisor shutdown, atomic state writes, launch-lock handling, `scripts/ensure_daemon.sh`, and `scripts/systemd/mejepa-daemon.service`.
- `14ee42cb` shipped the heal-crate self-optimization scheduler, scheduler state, and scheduler FSV harness.
- `23593272` wired the daemon self-optimization task to the heal-crate scheduler and added embedder-backed instrument heads plus neural forward-wrapper FSV.

Docs updated to reflect:

- `TASK-FLYWHEEL-001` is code/FSV complete, with fresh-host install/runbook proof still needed.
- `TASK-FLYWHEEL-002` is complete and daemon-wired; Phase E still owns full autonomous policy behavior.
- Weekly eval scheduler/report writer exists in the daemon; production holdout feed, dashboard, and runbook remain.
- Neural `EmbedderForward` exists for E1/E6/E7/E8/E10/E12/E13/E14; remaining work is full model-artifact FSV coverage, cache/latency integration, and shift-to-panel integration.
