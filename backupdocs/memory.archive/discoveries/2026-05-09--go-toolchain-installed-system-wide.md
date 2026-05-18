---
namespace: discoveries
created: 2026-05-08T23:23:50-05:00
updated: 2026-05-08T23:23:50-05:00
status: active
tags: go, toolchain, install, path, fsv
---

# Go Toolchain Installed System-Wide

## What Happened
The official Go linux/amd64 binary distribution was installed from `https://go.dev/dl/`:

- Version: `go1.26.3`
- Archive: `go1.26.3.linux-amd64.tar.gz`
- Download metadata SHA-256: `2b2cfc7148493da5e73981bffbf3353af381d5f93e789c82c79aff64962eb556`
- Install root: `/usr/local/go`
- PATH exposure:
  - `/usr/local/bin/go -> /usr/local/go/bin/go`
  - `/usr/local/bin/gofmt -> /usr/local/go/bin/gofmt`
  - `/etc/profile.d/go.sh` adds `/usr/local/go/bin` and `$HOME/go/bin`
- GOPATH bin directory created: `/home/cabdru/go/bin`

Full-state verification evidence is stored at:

- `/tmp/contextgraph-go-install-fsv/install-evidence.log`

## Why It Matters
Go ME-JEPA AST/action harness tests can now exercise the real `go` compiler path on this machine instead of only verifying the missing-toolchain fail-closed branch.

## Related
- See also: `memory/discoveries/2026-05-09--go-mejepa-ast-harness-fail-closed-and-test-entrypoints.md`
- Tags: go, toolchain, ast-harness
