# Complete System Hardening Guide

**Audience:** AI agent / developer / team told "harden the system." **Promise:** exhaustive checklist + playbook + mental model.

---

## 1. What "harden" really means — 5 axes

| Axis | Covers | Neglect failure |
|---|---|---|
| **Security** | config, attack surface, IAM, secrets, supply chain | breach, data loss, ransomware |
| **Code quality** | bugs, dead code, redundancy, complexity, tech debt | slow velocity, regressions, brittle features |
| **Architectural** | coupling, cohesion, SPOFs, scalability | "death by 1000 cuts," costly rewrites |
| **Operational** | observability, runbooks, deploy/rollback, capacity | long outages, on-call burnout |
| **Verification** | provable correctness via FSV, manual+synthetic tests | "worked on my machine," ghost regressions |

All five required. Secure + dead-code = rots. Clean + no observability = silent failure. Green tests but no SoT verification = liability. **Hardening is a forever-discipline, not a sprint.**

---

## 2. Mental models (install before tooling)

### 2.1 Trigger → Process → Outcome
```
[Trigger Event]  ──►  [Process X]  ──►  [Outcome Y]
   observable        measurable       verifiable
```
- Trigger observable: log/count/time it.
- Process measurable: latency, error rate, branch taken, exception.
- Outcome verifiable: there is **always** a physical place where state lives — the Source of Truth (SoT).

If you can't point at all three, you don't understand the feature well enough to harden it.

### 2.2 First-principles thinking
Decompose to smallest physical/logical units, rebuild reasoning from there. Symptoms cluster; causes don't.

Prompt sequence:
1. What is *literally* happening at byte/SQL/HTTP/syscall level?
2. What invariant is being violated?
3. What single fact, if changed, would make the symptom impossible?
4. Why is that fact currently false?
5. What's the smallest change that makes it permanently true?

### 2.3 Source of Truth (non-negotiable)
Per state, exactly one authoritative location: DB row/col, file on disk, queue/topic message, cache (only when it IS the SoT — rare), TSDB metric/counter, external system record (Stripe charge ID, Auth0 user, etc.). UI rendered from above is almost never the SoT.

**A function returning `True` / `200 OK` is NOT evidence the work happened. Inspect the SoT directly.**

### 2.4 Symptom vs cause vs root cause
- **Symptom** — what user notices ("page is slow").
- **Cause** — immediate trigger ("query takes 4s").
- **Root cause** — structural reason it could happen ("no index AND no review catches missing indexes").

Symptom fix = patch. Cause fix = fix. Root cause fix = hardening.

### 2.5 Fail-closed vs fail-open
Design failure modes so safe path is default. Fail-open into undefined state = security + correctness bomb.

### 2.6 Defense in depth
Never trust a single control. Input validation **and** parameterized queries **and** least-priv DB user **and** WAF **and** logging. Layers survive partial compromise.

---

## 3. Security hardening (by layer)

Cross-ref OWASP Top 10:2025 + CIS Benchmarks (§12).

### 3.1 Asset inventory & threat modeling (first!)
- [ ] Full asset inventory: every host/container/function/repo/queue/bucket/secret/3rd-party/domain/cert/identity. Can't harden what you don't know exists.
- [ ] SBOM (CycloneDX or SPDX) on every artifact.
- [ ] Threat model per data flow: **STRIDE** (Spoofing/Tampering/Repudiation/Info-disclosure/DoS/Elevation) or PASTA.
- [ ] Classify data (public/internal/confidential/regulated). Priority follows classification.
- [ ] Define trust boundaries explicitly. Every diagram line crossing one needs auth+validation+logging.

### 3.2 OWASP Top 10:2025 (released May 2025, Barcelona)

| # | Category | Core controls |
|---|---|---|
| A01 | Broken Access Control | deny-by-default, server-side authz every request, ABAC/RBAC, IDOR tests, no client-side checks |
| A02 | Security Misconfiguration | repeatable hardening, minimal platform, identical dev/QA/prod, CSP/HSTS, no default creds |
| A03 | Software Supply Chain Failures (NEW, replaces "Vulnerable Components") | SBOM, signed artifacts (Sigstore/Cosign), pinned deps+hashes, SCA in CI, SLSA provenance |
| A04 | Cryptographic Failures | TLS 1.2+, AES-GCM or ChaCha20-Poly1305, Argon2/scrypt passwords, no homemade crypto, key rotation |
| A05 | Injection (SQL/NoSQL/LDAP/OS/ORM/template/log/prompt) | parameterized queries, allow-list inputs, output encoding, ORM-safe APIs |
| A06 | Insecure Design | threat modeling, abuse cases, secure design patterns, security requirements alongside functional |
| A07 | Identification & Auth Failures | MFA, session mgmt, no credential stuffing exposure |
| A08 | Software & Data Integrity Failures | signed updates, integrity checks, no insecure deserialization, CI/CD hardening |
| A09 | Security Logging & Monitoring Failures | centralized logs, alerting on auth/authz, retention, tamper-evident |
| A10 | Mishandling of Exceptional Conditions (NEW) | no fail-open, timeouts everywhere, malformed-input fuzzing, race tests, structured error handling |

Notable 2025 changes: Misconfig #5→#2; Supply Chain #6→#3; A10 and A03 are new categories.

### 3.3 OS hardening (apply CIS Benchmark L1 general / L2 sensitive / STIG gov)
- [ ] Disable/remove unused services, daemons, packages.
- [ ] Mount options `nodev,nosuid,noexec` on `/tmp`, `/var/tmp`, `/dev/shm`, `/home`.
- [ ] Kernel: ASLR on, `kernel.randomize_va_space=2`, `kernel.kptr_restrict=2`, `net.ipv4.conf.all.rp_filter=1`, disable IP forwarding unless required.
- [ ] MAC: SELinux enforcing or AppArmor enforce profile.
- [ ] Patch SLA: critical ≤7d, high ≤30d; unattended-upgrades for security only.
- [ ] auditd/EventLog → SIEM, retained ≥90d.
- [ ] Disable USB/autorun/automount on servers.
- [ ] cron/at restricted to allow-listed users.
- [ ] Authenticated NTP (chrony w/ NTS).
- [ ] Bootloader password; secure boot where supported.
- [ ] Full-disk encryption (LUKS/BitLocker/FileVault) on mobile/non-datacenter.

### 3.4 Identity & access (IAM)
- [ ] Remove default accounts/passwords.
- [ ] MFA on every interactive account; prefer FIDO2/WebAuthn > TOTP > SMS.
- [ ] Passwords per NIST SP 800-63B: ≥12 chars, no composition rules, no forced rotation, breach-corpus check (HIBP).
- [ ] **Least privilege**, reviewed quarterly.
- [ ] Centralized RBAC/ABAC. No scattered `if user.is_admin`.
- [ ] Just-in-time elevation (sudo+log, IAM Identity Center, PAM).
- [ ] Service accounts: no human login, rotated creds, scoped, not shared across systems.
- [ ] Off-boarding: disable within 1hr; weekly orphan audit.
- [ ] PAM for break-glass; vault sealed; access logged.

### 3.5 Network
- [ ] **Default deny** at every layer: egress, ingress, east-west.
- [ ] Segmentation: prod vs non-prod, app vs data, DMZ vs internal.
- [ ] No `0.0.0.0/0` rules except explicitly justified and documented.
- [ ] WAF in front of every public HTTP endpoint (tuned, not defaults).
- [ ] DDoS protection (CloudFront/Cloudflare/Shield).
- [ ] DNS: DNSSEC, CAA pinned, registrar locked + MFA.
- [ ] Admin planes (SSH/RDP/K8s API/DB) behind VPN or zero-trust proxy (Tailscale/Cloudflare Access/Teleport). **Never** publicly exposed.
- [ ] mTLS service-to-service in zero-trust nets.
- [ ] Egress filtering — only known-good destinations (stops most exfil + C2).

### 3.6 Cloud (AWS/Azure/GCP — apply CIS Cloud Foundations)
- [ ] Root account: hardware MFA, no API keys, alarms on use.
- [ ] CloudTrail/Activity Log/Audit Log: all regions/services, log validation on, separate-account immutable bucket.
- [ ] Zero public storage unless explicitly designated; Block Public Access at account level.
- [ ] Encryption at rest with CMK where data class requires.
- [ ] Default VPC unused, flow logs on, private/VPC endpoints for service traffic.
- [ ] IAM: no inline policies, no `*` action in prod, permission boundaries, org-level SCPs.
- [ ] Every resource tagged with owner/env/data-class/cost-center. Untagged = quarantined.
- [ ] CSPM continuously scanning (Wiz/Prisma/Security Hub/Defender for Cloud).
- [ ] Region restriction via SCP — block unused regions.

### 3.7 Container & Kubernetes
- [ ] Base images: distroless/Alpine/Chainguard; scanned in CI; signed.
- [ ] No root user; `USER` directive set; capabilities dropped.
- [ ] Read-only root FS; explicit writable paths.
- [ ] CPU/memory limits + requests on every workload.
- [ ] Pod Security Admission `restricted` for non-system namespaces.
- [ ] NetworkPolicies default-deny; explicit allow.
- [ ] Secrets via external manager (Vault/AWS Secrets Manager/Sealed Secrets) — never raw `Secret` in Git.
- [ ] Image admission control (Kyverno/OPA Gatekeeper/Connaisseur) — block unsigned, block CVE-over-threshold.
- [ ] kube-apiserver: no anonymous auth, RBAC enforced, audit log on.
- [ ] etcd encrypted at rest; access restricted to control plane.
- [ ] Service mesh (Istio/Linkerd) for mTLS if multi-tenant.
- [ ] Runtime threat detection (Falco/Tetragon).

### 3.8 Application
- [ ] Input validation on **every** boundary via allow-lists, not deny-lists.
- [ ] Output encoding contextual to sink (HTML/attribute/JS/URL/SQL).
- [ ] Parameterized queries / prepared statements for **every** DB interaction.
- [ ] CSRF tokens on every cookie-auth state-changing endpoint.
- [ ] CORS: explicit origins, no `*` with credentials.
- [ ] Security headers: `Content-Security-Policy`, `Strict-Transport-Security` (preload), `X-Content-Type-Options: nosniff`, `Referrer-Policy: strict-origin-when-cross-origin`, `Permissions-Policy`, `X-Frame-Options: DENY`.
- [ ] Session cookies: `HttpOnly`, `Secure`, `SameSite=Lax`|`Strict`.
- [ ] Rate limiting per endpoint (esp. auth, password reset, search).
- [ ] Account lockout / progressive delay on auth failures.
- [ ] No verbose errors to client; correlation IDs for support.
- [ ] File uploads: MIME sniff, magic-byte check, size cap, AV scan, store outside web root, never serve same-origin.
- [ ] SSRF defenses: block link-local (`169.254.169.254`), private CIDRs by default in URL fetchers. SSRF test every URL-taking endpoint.
- [ ] No insecure deserialization (no `pickle`/`unserialize` on untrusted input).
- [ ] Vetted crypto libs (`libsodium`, `cryptography`); never MD5/SHA1/DES/RC4/ECB/custom protocols.

### 3.9 API
- [ ] OpenAPI is the contract; reject outside-spec.
- [ ] Per-endpoint authn + authz — never "the gateway will catch it."
- [ ] Resource-level authz (BOLA/IDOR).
- [ ] Bounds on array/string/depth/batch size.
- [ ] Pagination required on list endpoints; no `?limit=10000`.
- [ ] Versioning strategy (URL or header) with sunset policy.
- [ ] Token lifetime minimal; rotation supported.
- [ ] Replay protection (nonce+timestamp) on sensitive endpoints.
- [ ] Webhook signing (HMAC-SHA256) outbound; verify inbound.

### 3.10 Database
- [ ] DB never on public internet — private subnet + SG.
- [ ] App connects with **least-priv** user scoped to schemas/tables it needs. No `GRANT ALL`.
- [ ] Separate users for migrations / app / reporting.
- [ ] Encryption at rest (TDE/KMS).
- [ ] TLS in transit; cert pinning where feasible.
- [ ] Backups encrypted, separate account/region, **restore tested quarterly**.
- [ ] Audit log: DDL, privileged queries, failed logins.
- [ ] Schema constraints: `NOT NULL`, `CHECK`, `UNIQUE`, `FOREIGN KEY` — push integrity to DB.
- [ ] Slow query log on; reviewed weekly.
- [ ] RLS for multi-tenant data.

### 3.11 Secrets
- [ ] Centralized store (Vault/AWS-GCP-Azure Secret Manager/1Password Connect/Doppler).
- [ ] No secrets in code/config/Dockerfile env/CI logs/chat.
- [ ] Pre-commit `gitleaks`/`trufflehog` blocks.
- [ ] Rotation: short-lived dynamic creds where possible; static rotated on schedule.
- [ ] Every secret has owner/purpose/cadence/last-rotated.
- [ ] Audit log on every fetch.

### 3.12 Supply chain (OWASP A03:2025)
- [ ] SBOM per build, stored with artifact.
- [ ] Deps pinned by version **and** hash (`pip --require-hashes`, npm lockfile integrity, `go.sum`).
- [ ] Private registry / dependency proxy — no direct public-registry pulls into prod.
- [ ] SCA in CI; block PRs with critical/high CVEs.
- [ ] SLSA L3+ where feasible; signed attestations (Sigstore Cosign, in-toto).
- [ ] Reproducible builds where possible.
- [ ] Code signing for distributed binaries.
- [ ] Verify upstream maintainers — typo-squatting (`reqeusts`, `colourama`) is common.
- [ ] Register internal package names on public registries (defeat dependency confusion).

### 3.13 Logging, monitoring, observability
- [ ] Structured logs (JSON), consistent schema across services.
- [ ] Required fields: UTC timestamp, service, version, env, request ID, user/tenant ID, severity.
- [ ] **Never log** secrets, full PII, full tokens, full card numbers. Redact at logger layer.
- [ ] Centralized aggregation (ELK/Loki/Splunk/Datadog) with retention by class.
- [ ] Metrics: RED (rate/errors/duration) + USE (utilization/saturation/errors).
- [ ] Distributed tracing (OpenTelemetry) — propagate trace context.
- [ ] Alerts tied to SLOs, not raw metrics; alert on symptom not cause.
- [ ] Tamper-evident logs (write-once/hash-chain/separate account).
- [ ] SIEM correlation rules: brute force, lateral movement, exfiltration patterns.

### 3.14 Backup / DR / BCP
- [ ] **3-2-1**: 3 copies, 2 media, 1 off-site/account/cloud.
- [ ] Backups encrypted; **actual restore drill ≥ quarterly**. Untested backup ≠ backup.
- [ ] RPO and RTO documented per system; gap measured.
- [ ] Immutable backups (S3 Object Lock / Azure immutable blobs) for ransomware defense.
- [ ] DR runbooks rehearsed annually.

### 3.15 Incident response
- [ ] Documented IR plan: detection/triage/containment/eradication/recovery/lessons.
- [ ] On-call rotation; pager works; escalation clear.
- [ ] Forensic capability: snapshot host, preserve memory, pull logs, freeze credential — *fast*.
- [ ] Pre-written comms templates (internal/customer/regulator).
- [ ] Blameless PIR within 5 business days; action items tracked.

### 3.16 Compliance mapping (in scope?)
NIST CSF 2.0 (Identify/Protect/Detect/Respond/Recover/Govern); NIST SP 800-53/-171; ISO 27001:2022; PCI DSS 4.0; HIPAA Security Rule; SOC 2 Type II; GDPR/CCPA. CIS Benchmarks map cleanly to most — make CIS your baseline.

---

## 4. Code quality hardening

### 4.1 Tech-debt taxonomy
1. **Intentional/strategic** — knowingly taken to ship; documented payback.
2. **Unintentional** — inexperience/missing context/oversight. Most debt.
3. **Outdated design** — yesterday's clever design is today's bottleneck.
4. **Environmental** — frameworks/libs/runtimes/OSes aged out.
5. **Architectural** — structural; the expensive kind.

### 4.2 Bug hunting (run all — they overlap on purpose)
- [ ] **SAST**: `semgrep`, `ruff`, `mypy --strict`, `eslint`, `tsc --noEmit`, `clippy`, `go vet`, `staticcheck`, `SonarQube`, `CodeQL`. Tune for codebase; don't disable inconvenient rules.
- [ ] **Strict types**: TS strict, Python `mypy --strict`, Go w/o `interface{}` leakage.
- [ ] **DAST**: OWASP ZAP / Burp / Nuclei against running app, per release.
- [ ] **Fuzzing**: `libFuzzer`, `AFL++`, `go-fuzz`, `Atheris`, `cargo-fuzz`, `Jazzer`. Every parser, every untrusted-input boundary, every deserializer.
- [ ] **Property-based**: Hypothesis, fast-check, Quickcheck, proptest.
- [ ] **Mutation testing**: Stryker, Pitest, mutmut. Weak tests pass after mutation.
- [ ] **Race/concurrency**: Go `-race`, ThreadSanitizer, Loom (Rust), `pytest-xdist` repeated.
- [ ] **Memory safety**: ASan, MSan, Valgrind, leak detectors.
- [ ] **PR review checklist** (§11.3) applied to every PR.

### 4.3 Dead code — find and delete
Liability with no benefit: bigger attack surface, slower builds, false coverage.
- [ ] Unused-symbol tooling: `ts-prune`, `deadcode` (Go), `vulture` (Py), `cargo-udeps`, `knip`.
- [ ] **Runtime coverage**: Coverity, JaCoCo prod agent, Python `coverage` in canary. Endpoints with 0 hits in 30d → candidates.
- [ ] Feature flags 100% on or off >90d → decide, delete flag + one branch.
- [ ] Delete commented-out code blocks; git remembers, comments lie.
- [ ] `TODO`/`FIXME`/`XXX`/`HACK` >12mo → convert to issues or delete.
- [ ] Orphaned files (not imported, not entry points, not tests).
- [ ] Orphaned DB columns/tables/indexes (query `pg_stat_user_tables` etc. for zero reads/writes).
- [ ] **Caveat:** indirect consumers (reflection, DI, dynamic dispatch, generated SDKs, dashboards, scripts) — verify before deleting; delete behind a flag if uncertain.

### 4.4 Duplication
- [ ] Detectors: `jscpd`, `simian`, SonarQube duplication, `dupl` (Go).
- [ ] Distinguish coincidental dup (looks same now, will diverge) from semantic dup (same intent — extract).
- [ ] Apply DRY judiciously; premature abstraction is debt too. 3+ occurrences with proven shared intent → extract.
- [ ] Watch cross-service redundancy in microservices (same regex in 12 places).

### 4.5 Code smells → refactorings (Fowler catalog)

| Smell | Refactoring |
|---|---|
| Long method (>30 lines or >1 reason to change) | Extract Method |
| Long parameter list (>4) | Introduce Parameter Object/Builder |
| Large class / God Object | Extract Class / split responsibilities |
| Feature envy | Move Method |
| Data clumps (3-4 fields traveling together) | Extract Class |
| Primitive obsession | Replace with Value Object |
| Switch/if cascades on type | Replace Conditional with Polymorphism |
| Shotgun surgery | Move Method/Field, consolidate |
| Divergent change | Extract Class |
| Parallel inheritance hierarchies | Move members, merge hierarchies |
| Lazy class | Inline Class |
| Speculative generality | Inline/collapse |
| Temporary field | Replace with Method / Null Object |
| Message chains (`a.getB().getC().getD()`) | Hide Delegate |
| Middle man | Remove Middle Man |
| Inappropriate intimacy | Move Method / extract module |
| Comments explaining what code does | Rewrite code |
| Magic numbers/strings | Replace with Named Constant |
| Mutable shared state | Immutable, encapsulate, channels/locks |

### 4.6 Complexity targets
- Cyclomatic per function: ≤10 target, alarm >15.
- Cognitive complexity (SonarQube): captures human readability.
- Function length ≤30 lines. File ≤500 lines. Class ≤200 lines / ≤7 public methods.
- Halstead for math-heavy modules. Maintainability Index as rollup.
- Coupling (afferent/efferent), instability `I = Ce/(Ca+Ce)` at module level.
- Coverage: line is weak, branch stronger, mutation score strongest.

### 4.7 Refactoring catalog (Fowler/Kerievsky)
Extract/Inline (Method/Variable); Rename (highest leverage); Move Method/Field/Class; Extract/Inline Class; Replace Conditional w/ Polymorphism; Replace Temp w/ Query; Introduce Parameter Object; Replace Magic Number; Encapsulate Field/Collection; Replace Inheritance w/ Delegation; Replace Type Code w/ State/Strategy; Decompose Conditional; Consolidate Conditional Expression; Replace Error Code w/ Exception; Strangler Fig (incremental legacy replacement).

### 4.8 Boy-Scout rule (codified, per touch)
Rename one bad name; delete one stale comment / commented-out block; extract one too-long method; tighten one type/assertion; replace one magic number. Compounded across team = debt repayment.

### 4.9 Tooling stack (minimum viable)
- Linter + formatter — pre-commit + CI gate.
- Type checker in CI on strict mode.
- SAST (Semgrep/CodeQL/SonarQube).
- SCA (Snyk/Dependabot/OSV-Scanner/Trivy).
- Secret scan (gitleaks/trufflehog) pre-commit + CI + historical.
- Test runner w/ coverage + branch coverage + periodic mutation.
- Code quality dashboard (SonarQube/CodeClimate/Codacy) w/ debt trend.

---

## 5. Architecture hardening

### 5.1 Six dimensions (score 1–5; ≤2 = priority)
1. **Scalability** — handle 10× traffic / 100× data? Next bottleneck?
2. **Security** — explicit trust boundaries, layered controls?
3. **Maintainability** — new engineer ships feature in week 1?
4. **Performance** — within p95/p99 latency budget under realistic load?
5. **Deployability** — roll out, roll back, multi-version safe?
6. **Observability** — 3 AM page → root cause visible in 5 min?

### 5.2 Anti-pattern watchlist (find one → found hardening work)
- **Big Ball of Mud** — no structure; everything talks to everything.
- **Distributed monolith** — microservices that must deploy together. Often the worst.
- **Shared DB across services** — multiple services R/W same tables, hidden coupling.
- **God service / God class** — one component owns 60%+ of logic/traffic.
- **Stovepipe** — point-to-point integrations, no shared contract.
- **Missing circuit breakers** — one slow downstream takes whole graph down.
- **Synchronous chains ≥4 deep** — p99 + failure prob multiply.
- **No bulkheads** — one tenant starves all (no quotas, shared thread pools).
- **The Blob** — class w/ hundreds of methods or thousands of lines.
- **Spaghetti** — control flow unfollowable without a debugger.
- **Lava flow** — old code nobody understands, "do not touch."
- **Golden hammer** — one tool/pattern for every problem.
- **Accidental vendor lock-in** — proprietary APIs woven through business logic.
- **Overengineering / Inner-Platform Effect** — configurable system reinventing its platform.
- **Reinvented wheel** — handwritten crypto/retry/queue/ORM.
- **Cargo cult** — "Netflix does it" w/o matching constraints.
- **Premature optimization** — pre-profile micro-opts.
- **Premature abstraction** — single-impl interfaces, no-need factories.
- **Boat anchor** — deprecated component "just in case."
- **Magic Pushbutton** — UI coupled to business logic; can't script/test.
- **Anemic Domain Model** — domain objects are data bags; logic in service classes.

### 5.3 Coupling & cohesion
- High cohesion (module does one thing well) ✓
- Low coupling (minimal, stable-interface dependencies) ✓
- Acyclic dependency graph at module/package/service level — required.
- Stable Dependencies Principle: depend toward stability.
- Stable Abstractions Principle: stable = abstract; concrete = unstable.

### 5.4 FMEA-lite per external dep (DB/queue/3rd-party/DNS/time/network)
For each: what happens if **slow**? **unavailable**? returns **wrong** data? **partially available** (50% errors)? **flaky** (1% errors)?
Each answer = control (timeout/circuit breaker/retry+backoff+jitter/bulkhead/fallback) OR documented accepted risk.

### 5.5 SPOFs
- [ ] Single DB primary, no replica? Single AZ? Single region?
- [ ] Single load balancer w/o failover?
- [ ] Single CI server holding deploy creds?
- [ ] Single human who knows X (bus factor)?
- [ ] Single shared cache that takes down everything when cold?
- [ ] Single auth provider w/o fallback?

Document accepted SPOFs; runbook each.

### 5.6 Capacity & scaling
- [ ] Load test at 2× current peak; ID next bottleneck.
- [ ] Latency budget allocated per hop; sums to user SLA.
- [ ] Horizontal scaling story per tier; vertical ceiling known.
- [ ] DB growth projected 12mo: storage/IOPS/connection limits.
- [ ] Cache hit rates monitored; stampede protection (singleflight, jittered TTL).
- [ ] Job backlogs alerted; workers scale independently.

### 5.7 Data architecture
- [ ] Data ownership clear: one service per table.
- [ ] Schema evolution: backward-compatible migrations, expand-contract.
- [ ] Consistency model documented (strong vs eventual, txn boundaries).
- [ ] Idempotency keys on every retryable mutating endpoint.
- [ ] Soft vs hard delete decided per table; retention aligned to legal.
- [ ] PII map: where, who, how deleted on request.

### 5.8 Deployability
- [ ] Build reproducible.
- [ ] Deploys automated; manual steps = bugs.
- [ ] Blue-green / canary / rolling; never big-bang.
- [ ] Rollback tested, <5 min.
- [ ] DB migrations forward+backward compatible across ≥1 version.
- [ ] Feature flags for risky changes; retirement tracked.

---

## 6. Full State Verification (FSV) — non-negotiable

> **Return values lie. Logs lie. The SoT does not lie. Always inspect the SoT.**

### 6.1 Four steps

**1. Define SoT.** Write down: *what* state changes, *where* it lives (DB table+col / file path / queue / S3 key / metric / external ID), *how* you'll read it (SQL/file/API/dashboard), *expected value* (exact / range / schema / count delta). If you can't answer all four — you don't understand the feature.

**2. Capture BEFORE.** Read SoT first, log value.
```sql
-- BEFORE
SELECT id, status, updated_at FROM orders WHERE id = 'test-order-1';
```

**3. Execute trigger.** Run the action; capture response. Response = evidence of *attempt*, not *success*.

**4. Capture AFTER and assert.** Read SoT again; compare to expected; record delta.
```sql
-- AFTER
SELECT id, status, updated_at FROM orders WHERE id = 'test-order-1';
-- Expected: status='paid', updated_at > BEFORE.updated_at
```

`200 OK` + unchanged row = test **failed**, even with no errors. Most common silent bug class in modern systems.

### 6.2 Verification chain (most features write multiple SoTs — verify ALL)

Example: "user submits order":
- DB `orders` row inserted w/ correct fields.
- DB `order_items` rows inserted, count matches cart.
- DB `inventory.available` decremented correctly.
- Queue `order.created` event w/ correct payload.
- External: Stripe charge w/ correct amount.
- Email `outbox` row queued, eventually sent.
- Metric `orders_created_total` incremented.
- Log entry w/ order ID + user ID.

Each = separate read against separate SoT. Skip any → prod bug.

### 6.3 Mandatory edge-case audit (≥3 categories per code path)
1. **Empty** — `""`, `[]`, `{}`, `null`, missing field.
2. **Single item** — off-by-ones surface here.
3. **Max allowed** — documented upper.
4. **Max+1** — must reject cleanly.
5. **Min allowed** — `0`, `1`, documented lower.
6. **Min−1** — `-1`, `0`.
7. **Wrong type** — string where int expected.
8. **Malformed** — invalid JSON/UTF-8/email.
9. **Unicode edges** — emoji, RTL, combining, NULL bytes, very long strings.
10. **Duplicate/replay** — same idempotency key twice; same form resubmit.
11. **Out-of-order** — event B before event A.
12. **Concurrent** — two actors same instant.
13. **Authorization variants** — owner, non-owner, admin, anonymous.
14. **Tenant scope** — A can't see B's data.
15. **Time edges** — DST, leap second, negative offset, end-of-month.

Per case log: synthetic input, SoT **before**, action, SoT **after**, assertion PASS/FAIL w/ expected vs actual.

### 6.4 Evidence artifact (deliverable for every FSV run)
```
=== FSV Run: feature_x — 2026-05-08T22:15:03Z ===

[Test 1: happy path] PASS
  SoT: orders.status
  Before: NULL
  Trigger: POST /orders {amount:100, ...}
  After:  'paid'
  Latency: 230ms
  Side effects verified:
    - order_items: 2 rows inserted ✓
    - inventory: SKU-42 went 50 → 48 ✓
    - queue 'order.created': 1 message ✓
    - stripe: charge ch_xyz created, amount=100 ✓
    - email outbox: 1 row ✓

[Test 2: empty cart] PASS
  Trigger: POST /orders {items:[]}
  Expected: 400 Bad Request, no rows written
  Response: 400 ✓
  SoT check: 0 new rows in orders ✓
...
```
No artifact = hardening incomplete.

### 6.5 What FSV is NOT
Not a unit test that mocks the DB. Not code review. Not "CI passed." Not "tested on my machine." It is a **read against the real SoT** showing the real change.

---

## 7. Manual testing methodology

### 7.1 Why manual matters
Automated tests verify what you thought to test; manual exploration finds what you didn't. Edge cases at feature *intersections* rarely have auto coverage. FSV (§6) is most rigorous by hand first, then partly automated.

### 7.2 Synthetic test data properties
- **Deterministic seed** — same input → same expected output every run.
- **Distinguishable** — values like `synthetic_test_user_2026_05_08` so you can find + clean.
- **Representative** — same shape/distribution as real data.
- **Boundary-rich** — includes §6.3 cases.
- **Privacy-safe** — generated, never copied from prod.
- **Cleanup-tagged** — deletable via single query.

Principle: *if `2+2=4`, then `POST /add {a:2,b:2}` must return `4` AND insert `calculations.result=4`.* Synthetic gives expected; FSV verifies SoT.

### 7.3 Happy path
For each user story / endpoint / job:
1. Set typical preconditions.
2. Perform canonical action.
3. Verify canonical outcome via FSV.

Cover every primary user story first. Broken happy paths → edge cases are noise.

### 7.4 Edge case sweep
**Boundary value analysis (BVA)** — errors cluster at boundaries. For range `[min, max]`: `min-1` (invalid), `min`, `min+1`, typical mid, `max-1`, `max`, `max+1` (invalid).
**Equivalence class partitioning (ECP)** — split input domain into classes; pick one rep per class + BVA values.

Also cover: all of §6.3; negative testing (rejects what it should?); authz (each role sees exactly what it should); concurrency (two actors same instant); network failures (kill DB/queue/API mid-flow → resume → verify consistency); partial failures (retries/idempotency/dedup); time travel (clock skew/jumps/DST); resource exhaustion (full disk/OOM/conn pool exhausted/rate-limit hit).

### 7.5 Trigger→Process→Outcome per test
1. Identify trigger (HTTP/cron/queue/file watch/timer).
2. Identify what makes it observable (logs/metrics/traces).
3. Identify process (code path, what it touches).
4. Identify outcome (state changes — where = SoT).
5. Identify how outcome is verifiable (query/file/dashboard/API).
6. Run trigger with synthetic data.
7. Read SoT.
8. Assert outcome matches expected.

Can't answer any step → hardening item (missing log, missing metric, unclear data ownership).

### 7.6 When a test fails
**Stop.** Don't "rerun and hope." Don't patch over. Apply §8 RCA *before* fixing.
After fix: update failing test to catch the bug now; add regression test pinned to bug ID; re-run **all** related FSV scenarios (fixes break adjacent paths); verify fix in SoT, not just green test.

### 7.7 Disposable test data
Either: isolated test DB/namespace dropped at end-of-run; tag every synthetic record + delete by query at end; or always-rollback txns (only when clean support). Document cleanup; verify cleanup ran (FSV applies to cleanup).

---

## 8. Root cause analysis

Goal is never "make symptom stop" — it's *the smallest structural change that makes this class of failure impossible.*

### 8.1 5 Whys (Toyota — Sakichi Toyoda / Taiichi Ohno)
> 1. Machine stopped? — Fuse blew from overload.
> 2. Overload? — Bearing not lubricated enough.
> 3. Not lubricated? — Lubrication pump not pumping enough.
> 4. Not pumping? — Pump shaft worn and rattling.
> 5. Shaft worn? — No strainer; metal scraps got in.

W/o the fifth Why you replace the fuse and it recurs. With it you install a strainer.

Rules: use evidence not opinion (logs/timestamps/code); 5 is approximate (3-7); "someone forgot" → go deeper to *why did the system allow human memory to be the only safeguard?*; multiple branches → fishbone; validate by asking "would fixing this have prevented the symptom?" — if not, dig more.

### 8.2 First-principles debugging (rebuilding method, complements 5 Whys)
1. State symptom precisely w/ evidence.
2. List every assumption that, if true, makes symptom impossible.
3. Verify each by inspection (logs/traces/SoT) — not reasoning.
4. First failed assumption is suspect; recurse.
5. Once found: *what fact, structurally true, makes this assumption self-evidently safe?* (e.g., "request must reach server" → health check + circuit breaker + DLQ on client.)

### 8.3 Fishbone (Ishikawa)
Spine = problem; major bones = categories (classic 6 Ms: Methods/Machines/Materials/Measurement/Environment/People; software: Code/Data/Config/Infra/Process/People); sub-bones = factors; run mini-5-Whys per plausible factor.

### 8.4 Fault tree analysis (high-stakes)
Top event = failure; AND/OR gates decompose conditions; leaves = basic events w/ probabilities. Quantifies risk and finds non-obvious AND-paths where low individual probs combine.

### 8.5 RCA anti-patterns
- Stopping at "human error" — ask why system permitted it.
- Stopping at first plausible cause — multiple causes coexist.
- Confusing correlation with causation — verify mechanism, not just timing.
- No follow-up — check at 30-60d: did fix hold?
- Blame — blameless is not optional; changes whether info surfaces.

### 8.6 RCA output
Timeline (timestamps + evidence sources); symptoms; causal chain; root cause stated as **system property** ("the system allowed X because Y"); action items (immediate fix / root-cause fix / detection improvement / prevention improvement) w/ owner+due-date; re-verification plan.

---

## 9. Capability optimization

### 9.1 Four strategic questions per component
1. What does this contribute (user-visible terms)?
2. What does the system need from this (SLA/throughput/accuracy/security/extensibility)?
3. What capability was originally intended (vs drift from intent)?
4. What's the max it could deliver if optimized? Gap = roadmap.

### 9.2 Capability-extraction checklist
- [ ] All originally-intended capabilities exposed + used?
- [ ] Accidental capabilities not exposed (caches, indexes that could serve more)?
- [ ] Needed capabilities this could provide but doesn't?
- [ ] API at right abstraction — not harder/easier-to-misuse than necessary?
- [ ] Cleanest seam to extend for anticipated features?
- [ ] Latency/throughput/cost aligned to what system needs, or accidental?
- [ ] Observability emits signals others would benefit from?
- [ ] Capabilities currently provided that **shouldn't** be (over-broad APIs invite misuse)?

### 9.3 Alignment with project intent
Subtle failure: team optimizes for easy/interesting, not what project needs. Re-anchor each pass to project goals. Write down project's top 3 user-visible promises; check each change against them.

---

## 10. Operational workflow

### 10.1 Hardening session end-to-end

**Phase 1 — Discover (read-only).** Verify asset inventory; read docs/ADRs/READMEs/runbooks; generate dep graph (`madge`/`pydeps`); 90-day metrics dashboard; read incidents+PIRs; run scanners (§3+§4) for baselines; walk codebase visually noting smells/dead code/anti-patterns/SoTs.

**Phase 2 — Threat-model & risk-rank.** STRIDE per data flow; arch review on six dimensions (§5.1); list all findings; rank by *likelihood × impact × ease-of-fix*; group into must/should/nice/accept.

**Phase 3 — Plan & sequence.** Order to maximize safety: never refactor w/o tests, never add tests w/o understanding behavior. Establish **safety net first**: characterization tests around current behavior. Slice into PR-sized increments — each shippable + reversible. Define FSV evidence per increment up front.

**Phase 4 — Execute loop (per increment).**
1. Snapshot state (SoT before, metrics before).
2. Make change.
3. Run all relevant tests.
4. Run manual FSV w/ synthetic data — happy path + edge cases.
5. Inspect every SoT; capture evidence.
6. Deploy to staging; repeat FSV.
7. Deploy to prod behind flag; canary; monitor; full rollout.
8. Verify post-deploy in prod SoTs (read-only).
9. Update docs + runbooks.

**Phase 5 — Re-verify & report.** Re-run Phase 1 baselines; quantify improvement; document accepted risks w/ owners + re-review schedule; hand-off/changelog/PIR of the hardening pass itself.

### 10.2 Stop conditions
- Test fails unexpectedly → RCA first.
- Assumption proves false → re-evaluate increment.
- Finding deeper than expected → re-rank, don't sneak in.
- Prod metrics shift after deploy → investigate; rollback if unclear.

### 10.3 Definition of Done (per hardening task)
- [ ] Change merged.
- [ ] All linters/types/SAST/SCA green.
- [ ] Unit + integration + property tests added/updated.
- [ ] **FSV evidence captured** (before/trigger/after/assert log).
- [ ] Edge cases tested manually (≥3 from §6.3) w/ evidence.
- [ ] Docs/runbooks/ADRs updated.
- [ ] Observability updated (new metrics/logs/alerts).
- [ ] Deployed to prod; post-deploy verification done.
- [ ] Risk register updated; residual risk noted.

### 10.4 Continuous hardening (cadence)
- **Weekly**: deps auto-PR'd + merged; failed-login / anomalous-access review.
- **Monthly**: dead-code sweep; coverage + complexity trend review.
- **Quarterly**: backup restore drill; IR tabletop; threat model refresh; access review.
- **Annually**: full arch review; DR exercise; 3rd-party pentest.
- **Every PR**: review checklist (§11.3).

---

## 11. Master checklists (copy-paste)

### 11.1 Daily / per-PR security & quality gate
- [ ] No secrets in diff (gitleaks clean).
- [ ] No new high/critical CVEs (SCA clean).
- [ ] No new SAST findings above threshold.
- [ ] Tests added/updated.
- [ ] FSV evidence attached for feature changes.
- [ ] No new TODO/FIXME w/o issue link.
- [ ] No magic numbers/strings w/o named constant.
- [ ] No `console.log`/`print`/debug stmts left in.
- [ ] Errors handled (no bare `catch(e){}` / `except: pass`).
- [ ] Inputs validated; outputs encoded.
- [ ] DB changes reversible / forward-compatible.
- [ ] Observability signals added where state changed.
- [ ] Docs updated where behavior changed.

### 11.2 Architecture review (per service)
- [ ] Single clear responsibility documented.
- [ ] Public API spec'd; backward-compat policy explicit.
- [ ] Owner + on-call documented.
- [ ] Data ownership: every table/topic owned by exactly one service.
- [ ] No shared write access to another service's data.
- [ ] Failure modes per external dep: timeout/retry/CB/fallback/bulkhead/idempotency.
- [ ] Latency budget met at p95/p99.
- [ ] Horizontal scaling demonstrated.
- [ ] Capacity headroom ≥2× peak.
- [ ] Deploy: canary or B/G; rollback <5min.
- [ ] Observability: RED metrics + structured logs + traces + SLO-aligned alerts.
- [ ] Security: authn+authz every endpoint, secrets in vault, least priv.
- [ ] Backup+restore drill within 90d.
- [ ] Runbook for top 5 alerts.

### 11.3 Code review checklist
- [ ] Behavior matches spec / PR description.
- [ ] Names communicate intent.
- [ ] Functions ≤~30 lines; cyclomatic ≤10.
- [ ] No duplication of existing utilities.
- [ ] No dead code added.
- [ ] Errors propagated meaningfully; no silent swallowing.
- [ ] Edge cases covered by tests.
- [ ] No race conditions in shared state.
- [ ] No N+1 queries; pagination on lists.
- [ ] No PII/secrets in logs.
- [ ] No breaking API changes w/o versioning plan.
- [ ] No new deps w/o justification + license review.

### 11.4 Database hardening
- [ ] Private network only.
- [ ] TLS in transit; encryption at rest w/ managed keys.
- [ ] App user has min grants; separate users migration/reporting.
- [ ] Constraints PK/NOT NULL/CHECK/FK/UNIQUE per business invariant.
- [ ] Indexes match query patterns; no unused indexes.
- [ ] Slow query log on, reviewed.
- [ ] Backup encrypted, off-site, restore tested ≤90d ago.
- [ ] Audit log for DDL + privileged ops.
- [ ] Connection pooling; no app-side conn leaks.
- [ ] Migration: expand-contract, reversible, online for big tables.

### 11.5 Pre-production deploy
- [ ] All tests green.
- [ ] FSV evidence attached.
- [ ] Migration plan + rollback plan reviewed.
- [ ] Canary plan: percentage, duration, success metrics.
- [ ] Alerts in place for new behavior.
- [ ] Feature flag default correct.
- [ ] Comms sent to who needs to know.
- [ ] Rollback button verified (not assumed).

### 11.6 IR readiness
- [ ] On-call schedule current; pager tested.
- [ ] Runbooks current for top 10 alerts.
- [ ] Forensic snapshot capability tested.
- [ ] Customer comms templates pre-approved.
- [ ] Regulator comms templates pre-approved (if regulated).
- [ ] Tabletop in last 6mo.
- [ ] Last-incident action items closed.

---

## 12. References (canon — defer to these over blogs)

**Security:**
- OWASP Top 10:2025 — https://owasp.org/Top10/
- OWASP ASVS 5.0 — https://owasp.org/www-project-application-security-verification-standard/
- OWASP Cheat Sheets — https://cheatsheetseries.owasp.org/
- OWASP Developer Guide (System Hardening) — https://devguide.owasp.org/
- CIS Benchmarks — https://www.cisecurity.org/cis-benchmarks
- CIS Critical Security Controls v8 — https://www.cisecurity.org/controls
- NIST CSF 2.0 — https://www.nist.gov/cyberframework
- NIST SP 800-53/-171 — https://csrc.nist.gov/publications
- NIST SP 800-63B (Auth) — https://pages.nist.gov/800-63-3/sp800-63b.html
- DISA STIGs — https://public.cyber.mil/stigs/
- CISA Secure by Design — https://www.cisa.gov/securebydesign
- ASD Essential Eight — https://www.cyber.gov.au/
- Sigstore / SLSA — https://www.sigstore.dev/, https://slsa.dev/
- K8s Hardening Guide (NSA/CISA) — search "Kubernetes Hardening Guidance NSA CISA"

**Code quality & architecture:**
- Fowler — *Refactoring* (code smells + refactorings)
- Feathers — *Working Effectively with Legacy Code* (characterization tests, seams)
- Martin — *Clean Architecture*
- Ousterhout — *A Philosophy of Software Design*
- Ford/Parsons/Kua — *Building Evolutionary Architectures*
- SonarQube rules — https://rules.sonarsource.com/
- Microsoft Application Architecture Guide
- AWS Well-Architected — https://aws.amazon.com/architecture/well-architected/
- Google SRE Books — https://sre.google/books/

**Testing & verification:**
- Humble & Farley — *Continuous Delivery*
- ISTQB Foundation Syllabus (BVA, ECP)
- Meszaros — *xUnit Test Patterns*
- Property-based: Hypothesis docs, fast-check docs
- Fuzzing: OSS-Fuzz, ClusterFuzz, AFL++

**RCA / lean:**
- Lean Enterprise Institute 5 Whys — https://www.lean.org/lexicon-terms/5-whys/
- ASQ Five Whys/Hows — https://asq.org/quality-resources/five-whys
- Dekker — *The Field Guide to Understanding Human Error*; *Drift into Failure*
- Toyota Production System lit — Taiichi Ohno

---

## The one thing to remember

> **A function's return value is a claim. The Source of Truth is the verdict. Always read the verdict.**

Every practice in this doc is built around that discipline. Scanners lie. Tests pass on stale data. Logs go missing. The row in the database — or its absence — does not lie.

Harden the system = change the system so it is harder to break, easier to understand, faster to fix, and **provably correct at the SoT**, every time, forever.
