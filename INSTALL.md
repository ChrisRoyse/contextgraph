# ContextGraph ME-JEPA Install

1. Create and verify aiwonder-backed state:

```bash
scripts/setup/first_time_setup.sh
```

2. Build the daemon:

```bash
cargo build --release -p context-graph-mcp
```

3. Start it:

```bash
scripts/ensure_daemon.sh
```

4. Verify state:

```bash
curl -fsS http://127.0.0.1:3101/healthz
jq . /zfs/archive/contextgraph/fsv/first-time-setup/setup-readback.json
```

Source of truth lives under `/zfs/archive/contextgraph` (durable) and `/zfs/hot/contextgraph` (hot) on aiwonder per CLAUDE.md §6.5. User config lives under `~/.config/mejepa`.
