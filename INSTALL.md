# ContextGraph ME-JEPA Install

1. Create and verify D-backed state:

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
jq . /mnt/d/contextgraph/fsv/first-time-setup/setup-readback.json
```

Source of truth lives under `/mnt/d/contextgraph`. User config lives under `~/.config/mejepa`.
