---
namespace: journal
created: 2026-05-08T23:23:50-05:00
updated: 2026-05-08T23:23:50-05:00
status: active
tags: success, go, install, verification
---

# Success: Go Toolchain Install

## What Happened
Installed Go `go1.26.3 linux/amd64` to `/usr/local/go`, verified the archive checksum before extraction, exposed `go` and `gofmt` through `/usr/local/bin`, and added `/usr/local/go/bin:$HOME/go/bin` via `/etc/profile.d/go.sh`.

Full-state verification read back:

- `command -v go` => `/usr/local/bin/go`
- `go version` => `go version go1.26.3 linux/amd64`
- `go env GOROOT GOPATH GOOS GOARCH GOTOOLCHAIN` => `/usr/local/go`, `/home/cabdru/go`, `linux`, `amd64`, `auto`
- `/home/cabdru/go/bin/hello` exists and runs after `go install ./cmd/hello`

## Why It Matters
The local reality loop can now use the real Go compiler/test runner for Go AST semantic validation.

## Related
- Evidence: `/tmp/contextgraph-go-install-fsv/install-evidence.log`
- Tags: go, install, fsv
