# Phase 11 State And Invariants - GitHub Issue Evidence-Link Verification

Phase 11 ships `github_issue_evidence_link_v1` on the existing `github_v1`
connector. It closes the first-wedge ticket proof gap by verifying that an
exact evidence URL is present in a real GitHub issue body and/or issue
comments. There was no schema bump in Phase 11; `LEDGER_SCHEMA_VERSION`
remained `5` then. The current live schema after the reward compiler is `14`.

## Source Of Truth

Primary sources of truth:

- GitHub issue endpoint:
  `GET /repos/{owner}/{repo}/issues/{issue_number}`;
- GitHub issue-comments endpoint:
  `GET /repos/{owner}/{repo}/issues/{issue_number}/comments`;
- local SQLite ledger table `ledger_records` for expected deltas, readback
  snapshots, verifier results, evidence packets, and incident packets.

Manual/automated FSV must read GitHub independently from the CLI result, then
read the physical SQLite table directly. CLI stdout/stderr alone is not proof.

## Shipped CLI Surface

```bash
context-graph blackbox connector test \
  --connector github_v1 \
  --source OWNER/REPO

context-graph blackbox verify \
  --ledger /tmp/bbx.sqlite \
  --input spec.json \
  --tenant TENANT \
  --agent AGENT
```

`BBX_GITHUB_TOKEN` or `GITHUB_TOKEN` may be set for private repositories or
higher rate limits. Public repositories can be read without a token. Tokens
are sent as bearer auth and are never persisted.

## Verifier Spec

`github_issue_evidence_link_v1` requires:

```json
{
  "verifier_id": "github_issue_evidence_link_v1",
  "run_id": "run_...",
  "tool_call_id": "tool_...",
  "source": {
    "owner": "actions",
    "repo": "checkout"
  },
  "expected": {
    "issue_number": 2424,
    "evidence_url": "https://github.com/xxxxx/xxxxx",
    "link_scope": "body_or_comments"
  }
}
```

`expected.link_scope` is optional and defaults to `body_or_comments`. Allowed
values are `body`, `comments`, and `body_or_comments`.

The verifier passes only when:

- the issue endpoint returns an issue;
- the issue payload does not contain `pull_request`;
- `expected.evidence_url` is an absolute `http` or `https` URL;
- the exact evidence URL string appears in the requested scope.

## Invariants

- GitHub issue readback uses the documented REST API, recommended media type,
  explicit API version header, and stable user agent.
- `source.owner` and `source.repo` must be single GitHub path segments without
  slashes, query strings, fragments, whitespace, or path traversal segments.
- `expected.issue_number` must be positive and is blocked before source reads
  when it is `0`.
- Pull requests returned through the Issues API are rejected as
  `GITHUB_ISSUE_IS_PULL_REQUEST`.
- Comment readback uses `per_page=100` and follows `Link rel="next"` until
  complete.
- A full 100-item comments page without a usable next-page signal is
  `GITHUB_PAGINATION_INCOMPLETE`, not a partial pass.
- Malformed pagination links are `GITHUB_PAGINATION_INVALID`.
- Readback snapshots store HTTP status, final URL, rate-limit headers,
  pagination headers, response body SHA-256, issue body SHA/length, comment
  body SHA/length summaries, matched locations, and normalized issue state.
- Invalid evidence URLs and invalid link scopes are blocked before source
  reads, so they write no `readback_snapshot`.
- Successful issue readbacks must contain the requested issue number, a string
  state, and a string/null body; successful comment readbacks must contain a
  numeric id and string body. Schema drift blocks instead of producing a weak
  mismatch.
- Missing or mismatched ticket state is failed and produces an
  `incident_packet`; invalid verifier specs are blocked and produce no
  incident packet.

## Error Codes

- `GITHUB_ISSUE_NOT_FOUND`
- `GITHUB_ISSUE_IS_PULL_REQUEST`
- `GITHUB_ISSUE_EVIDENCE_LINK_NOT_FOUND`
- `GITHUB_ISSUE_EVIDENCE_URL_INVALID`
- `GITHUB_ISSUE_EVIDENCE_SCOPE_INVALID`
- `GITHUB_ISSUE_NUMBER_INVALID`
- `GITHUB_ISSUE_COMMENTS_NOT_FOUND`
- `GITHUB_REPO_LOCATOR_INVALID`
- `GITHUB_PAGINATION_INVALID`
- `GITHUB_PAGINATION_INCOMPLETE`
- `GITHUB_RATE_LIMITED`
- `CONNECTOR_PERMISSION_DENIED`
- `READBACK_SCHEMA_DRIFT`
- `SOURCE_OF_TRUTH_UNAVAILABLE`

## Automated FSV

```bash
cargo test --package context-graph-cli --test blackbox_phase11_github_issue_evidence_fsv -- --nocapture
```

Current result: 1/1 passing.

The test uses real GitHub state. It discovers a public issue whose body
contains an `http(s)` URL with `curl`, drives the real
`context-graph blackbox` binary, then opens the physical SQLite ledger with
`rusqlite` and checks row counts and payloads. Default fixture:
`actions/checkout`. Override with `BBX_GITHUB_TICKET_TEST_REPO` or
`BBX_GITHUB_TEST_REPO`.

Coverage:

- connector list exposes `github_issue_evidence_link_v1` under `github_v1`;
- connector test reads real GitHub repository metadata;
- happy evidence-link verifier passes;
- missing issue fails;
- wrong evidence URL fails;
- issue endpoint returning a pull request fails;
- invalid evidence URL blocks;
- invalid link scope blocks.
- invalid issue number blocks;
- invalid repository source segments block.

## Manual FSV Evidence

Manual run:

- log: `/tmp/bbx-phase11-manual-fsv.zftsp_oa/fsv.log`;
- SQLite source of truth:
  `sqlite:/tmp/bbx-phase11-manual-fsv.zftsp_oa/github-ticket.sqlite#ledger_records`;
- GitHub source of truth:
  `github:actions/checkout#issues/2424/evidence-link`.

Discovered live fixture:

- `owner=actions`;
- `repo=checkout`;
- `issue_number=2424`;
- `issue_state=open`;
- `evidence_url=https://github.com/xxxxx/xxxxx`;
- `body_contains_evidence_url=true`;
- `body_sha256=sha256:06f5ae49bd5152ec5da468bda6d879e37f0e002e59277fcc5f11ebeb659bc4e5`;
- PR edge fixture: `actions/checkout#2418`.

Happy-path proof from physical SQLite:

- `github_issue_evidence_link_v1` verifier row sequence `5`, status
  `passed`;
- readback row sequence `4` stored `api_pages=2`, `comment_pages_read=1`,
  `comments_total=0`, `present=true`, and `matched_locations=1`;
- evidence packet row sequence `6` referenced the expected, readback, and
  verifier physical rows.

Edge-case proof from physical SQLite:

| Case | Expected result | Verifier row |
|---|---|---|
| missing issue number | `failed`, `GITHUB_ISSUE_NOT_FOUND` | sequence `9` |
| wrong evidence URL | `failed`, `GITHUB_ISSUE_EVIDENCE_LINK_NOT_FOUND` | sequence `14` |
| issue endpoint returns a pull request | `failed`, `GITHUB_ISSUE_IS_PULL_REQUEST` | sequence `19` |
| malformed evidence URL | `blocked`, `GITHUB_ISSUE_EVIDENCE_URL_INVALID` | sequence `23` |
| unsupported `link_scope` | `blocked`, `GITHUB_ISSUE_EVIDENCE_SCOPE_INVALID` | sequence `26` |
| `issue_number=0` | `blocked`, `GITHUB_ISSUE_NUMBER_INVALID` | hardened FSV |
| invalid owner/repo source segment | `blocked`, `GITHUB_REPO_LOCATOR_INVALID` | hardened FSV |

Final physical SQLite counts from the original full manual FSV:

```json
{
  "agent_run": 1,
  "tool_call": 1,
  "expected_delta": 6,
  "readback_snapshot": 4,
  "verifier_result": 6,
  "evidence_packet": 6,
  "incident_packet": 3
}
```

Final metadata:

```json
{
  "schema_version": "5",
  "ledger_record_count": "27",
  "ledger_tip_sha256": "sha256:e9c4e7b0af9c3057c4f313bfd24650b4cf26b714f8be82e1d8d93d88107b52c0"
}
```

Invalid URL and invalid scope wrote `expected_delta`, `verifier_result`, and
`evidence_packet` rows, but no `readback_snapshot` rows. That is the intended
fail-closed behavior because the verifier blocked before any source read.

Hardening addendum:

- log: `/tmp/bbx-hardening-fsv.ch1qc2hp/fsv.log`;
- SQLite source of truth:
  `sqlite:/tmp/bbx-hardening-fsv.ch1qc2hp/phase11/github-ticket.sqlite#ledger_records`;
- GitHub source of truth:
  `github:actions/checkout#issues/2424/evidence-link`;
- issue body SHA:
  `sha256:06f5ae49bd5152ec5da468bda6d879e37f0e002e59277fcc5f11ebeb659bc4e5`;
- final focused counts: `agent_run=1`, `tool_call=1`,
  `expected_delta=4`, `readback_snapshot=2`, `verifier_result=4`,
  `evidence_packet=4`, `incident_packet=1`;
- verifier rows: passed evidence link at sequence `5`, blocked
  `GITHUB_ISSUE_NUMBER_INVALID` at sequence `8`, blocked
  `GITHUB_REPO_LOCATOR_INVALID` at sequence `11`, and failed
  `GITHUB_ISSUE_EVIDENCE_LINK_NOT_FOUND` at sequence `15`.

The automated hardened FSV asserts the complete Phase 11 suite final counts:
`expected_delta=8`, `readback_snapshot=4`, `verifier_result=8`,
`evidence_packet=8`, `incident_packet=3`.
