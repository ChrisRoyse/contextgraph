# Per-tick loop prompt

Continue one ME-JEPA-Code outer-loop increment. Per-turn protocol is in `CLAUDE.md §9` and `docs2/AICodingAgentSuperPrompt.md §12`.

Each tick:

1. Read pinned `type:context` GitHub issues + `docs/futurebuild/08_roadmap_and_open_work.md` for current critical-path work.
2. Pick the highest-priority open issue labeled `status:needs-triage`, `source:agent`, no assignee — or follow operator direction.
3. CLAIM the issue per doctrine §3.4 (atomic `gh issue edit` + CLAIM comment).
4. Audit the task against the live tree per `docs/futurebuild/specs/FSV-PROTOCOL.md §7`. Assume the task description is wrong; read the code first (Linear Sequential Unmasking).
5. Ship one primitive end-to-end with full FSV evidence under `/zfs/archive/contextgraph/fsv/<task>-fsv/` on aiwonder.
6. Verify per doctrine §4: `cargo build --release -p <crate>`, `cargo test -p <crate>`, `cargo clippy --no-deps --tests --examples -- -D warnings`. Independent re-open readback for any RocksDB write.
7. Close out with RESOLVED / PAUSE / BLOCKED comment containing the evidence table.

No `./memory/*.md` writes. Memory = GitHub Issues.
