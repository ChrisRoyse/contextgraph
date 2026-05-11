# The Complete System Hardening Guide

> **Audience:** an AI agent, software developer, or engineering team being told "harden the system."
> **Purpose:** define what "harden" means in its widest, most useful sense — security hardening, code hardening, architecture hardening, defect hunting, and most importantly the **verification discipline** (Full State Verification / FSV) that proves the work actually worked.
> **Promise:** this document is exhaustive on purpose. Treat it as a checklist + playbook + mental model. Skim the table of contents, then go deep where it matters.

---

## Table of Contents

1. [What "Harden the System" Really Means](#1-what-harden-the-system-really-means)
2. [The Mental Models You Must Internalize First](#2-the-mental-models-you-must-internalize-first)
3. [Security Hardening — The Exhaustive List](#3-security-hardening--the-exhaustive-list)
4. [Code Quality Hardening — Bugs, Dead Code, Redundancy, Tech Debt](#4-code-quality-hardening--bugs-dead-code-redundancy-tech-debt)
5. [Architecture Hardening — Structural Failures and Anti-Patterns](#5-architecture-hardening--structural-failures-and-anti-patterns)
6. [Full State Verification (FSV) — The Non-Negotiable Protocol](#6-full-state-verification-fsv--the-non-negotiable-protocol)
7. [Manual Testing Methodology — Happy Path + Edge Cases + Synthetic Data](#7-manual-testing-methodology--happy-path--edge-cases--synthetic-data)
8. [Root Cause Analysis — First Principles, 5 Whys, Fishbone](#8-root-cause-analysis--first-principles-5-whys-fishbone)
9. [Capability Optimization — Extracting Maximum Value From Each Component](#9-capability-optimization--extracting-maximum-value-from-each-component)
10. [Operational Workflow — Putting It All Together](#10-operational-workflow--putting-it-all-together)
11. [Master Checklists (Copy-Paste Ready)](#11-master-checklists-copy-paste-ready)
12. [References & Authoritative Sources](#12-references--authoritative-sources)

---

## 1. What "Harden the System" Really Means

In ordinary speech, "hardening" is short for "system hardening" — the security discipline of reducing attack surface. But when an experienced engineer says "harden this system," they usually mean something much wider. The complete definition has **five orthogonal axes**:

| Axis | What it covers | Failure mode if neglected |
|------|---------------|---------------------------|
| **Security hardening** | Configuration, attack surface, IAM, secrets, supply chain | Breach, data loss, ransomware |
| **Code-quality hardening** | Bugs, dead code, redundancy, complexity, tech debt | Slow velocity, regressions, brittle features |
| **Architectural hardening** | Coupling, cohesion, single points of failure, scalability | "Death by a thousand cuts," costly rewrites |
| **Operational hardening** | Observability, runbooks, deploy/rollback, capacity | Long outages, on-call burnout |
| **Verification hardening** | Provable correctness via FSV, manual + synthetic tests | "Worked on my machine," ghost regressions |

You **must** address all five. A system that is secure but full of dead code rots. A system with clean code but no observability fails silently. A system tested only with green unit tests but never **verified at the source of truth** is a liability waiting to surface.

> **Operating principle:** *Hardening is not a sprint and it is not a checklist you finish. It is a discipline you apply on every change, forever.*

---

## 2. The Mental Models You Must Internalize First

Before any tooling, install these mental models. Every later section assumes them.

### 2.1 The Trigger → Process → Outcome model

Every meaningful action in a system follows this shape:

```
[Trigger Event]  ──►  [Process X]  ──►  [Outcome Y]
       │                   │                  │
   observable          measurable        verifiable
```

Three implications:

- **The trigger can be observed.** A user submits a form, a cron fires, a webhook arrives, a queue message lands. You can log it, count it, time it.
- **The process can be measured.** Latency, error rate, branch taken, exception thrown.
- **The outcome can be verified.** A row is written, a file appears, an email is sent, a UI state changes, a counter increments. **There is always a place where the outcome physically lives** — the *source of truth*.

If you cannot point at all three, you do not understand the feature well enough to harden it. Start by drawing the triangle for whatever you are working on.

### 2.2 First-principles thinking

Decompose every problem to the smallest physical/logical units it stands on, then rebuild your reasoning from there. Do not pattern-match on what looks similar. Symptoms cluster; causes do not.

A useful prompt sequence:
1. What is *literally* happening at the byte/SQL/HTTP/syscall level?
2. What invariant is being violated? (e.g., "this counter must equal the row count")
3. What single fact, if changed, would make the symptom impossible?
4. Why is that fact currently false?
5. What is the smallest change that makes it permanently true?

### 2.3 Source of Truth (SoT) is non-negotiable

For every piece of state, there is exactly one authoritative location:
- A **database row / column** (RDBMS, NoSQL, KV store).
- A **file** on disk (config, log, artifact, blob).
- A **message / event** in a queue or topic.
- A **cache** *only when* the cache is itself the source of truth (rare).
- A **UI state** rendered from the above (almost never the source).
- A **metric / counter** in a TSDB.
- An **external system's** record (Stripe charge ID, Auth0 user, etc.).

A function returning `True` or `200 OK` is **not evidence** the work happened. The SoT must be inspected directly.

### 2.4 Symptom vs cause vs root cause

- **Symptom**: what the user/operator notices ("the page is slow").
- **Cause**: the immediate trigger ("the query takes 4s").
- **Root cause**: the structural reason it could happen ("no index on `created_at` *and* no review process catches missing indexes before merge").

Fixing only the symptom is a patch. Fixing the cause is a fix. Fixing the root cause is hardening.

### 2.5 Fail-closed vs fail-open

When something goes wrong, what is the default? A system that "fails open" into an undefined state is a security and correctness bomb. Hardening means designing failure modes so the safe path is the default.

### 2.6 Defense in depth

Never trust a single control. Input validation **and** parameterized queries **and** least-privileged DB user **and** WAF **and** logging. Layers are how systems survive partial compromises.

---

## 3. Security Hardening — The Exhaustive List

This section is organized by **layer**: from the silicon up to the user. Apply every layer that exists in your system. Cross-reference with the OWASP Top 10:2025 and CIS Benchmarks (links in §12).

### 3.1 Asset inventory & threat modeling (do this first or you will miss things)

- [ ] **Build a full asset inventory.** Every host, container, function, repo, queue, bucket, secret, third-party service, domain, certificate, and human/service identity. You cannot harden what you do not know exists.
- [ ] **Generate / maintain a Software Bill of Materials (SBOM)** in CycloneDX or SPDX format. Attach it to every artifact.
- [ ] **Threat-model** with STRIDE (Spoofing, Tampering, Repudiation, Information Disclosure, Denial of Service, Elevation of Privilege) or PASTA per data flow.
- [ ] **Classify data** by sensitivity (public / internal / confidential / regulated). Hardening priority follows classification.
- [ ] **Define trust boundaries** explicitly. Every line in a system diagram that crosses a boundary needs auth + validation + logging.

### 3.2 OWASP Top 10:2025 — the application-layer canon

The 2025 edition (released at Global AppSec EU Barcelona, May 2025) reshuffled categories meaningfully. Address each:

| # | Category | Core controls |
|---|----------|---------------|
| **A01** | Broken Access Control | Deny by default, server-side authorization on every request, ABAC/RBAC, IDOR tests, no client-side auth checks |
| **A02** | Security Misconfiguration | Repeatable hardening process, minimal platform, identical dev/QA/prod configs, CSP/HSTS/X-Frame-Options, no default creds |
| **A03** | Software Supply Chain Failures (NEW — replaces "Vulnerable Components") | SBOM, signed artifacts (Sigstore/Cosign), pinned dependencies with hashes, SCA in CI, provenance attestations (SLSA) |
| **A04** | Cryptographic Failures | TLS 1.2+ everywhere, AES-GCM or ChaCha20-Poly1305, Argon2/scrypt for passwords, no homemade crypto, key rotation |
| **A05** | Injection (SQL, NoSQL, LDAP, OS, ORM, template, log, prompt) | Parameterized queries, allow-listed inputs, output encoding, ORM-safe APIs |
| **A06** | Insecure Design | Threat modeling, abuse cases, secure design patterns, security requirements alongside functional ones |
| **A07** | Identification & Authentication Failures | MFA, password rotation policy, session management, no credential stuffing exposure |
| **A08** | Software & Data Integrity Failures | Signed updates, integrity checks, no insecure deserialization, CI/CD pipeline hardening |
| **A09** | Security Logging & Monitoring Failures | Centralized logs, alerting on auth/authz events, retention policy, tamper-evident |
| **A10** | Mishandling of Exceptional Conditions (NEW) | No "fail open," timeouts everywhere, malformed-input fuzzing, race-condition tests, structured error handling |

> **Notable 2025 changes:** Security Misconfiguration jumped from #5 to #2; Supply Chain Failures jumped to #3 (was #6 in 2021); two new categories — A10 (exceptional conditions) and A03 (supply chain expansion).

### 3.3 Operating system hardening

Apply the relevant **CIS Benchmark** for your OS — Level 1 (basic) for general workloads, Level 2 (defense-in-depth) for sensitive workloads, STIG profile for government/military.

- [ ] Disable / remove unused services, daemons, and packages.
- [ ] Enforce minimum filesystem mount options (`nodev`, `nosuid`, `noexec` on `/tmp`, `/var/tmp`, `/dev/shm`, `/home`).
- [ ] Kernel hardening: ASLR on, `kernel.randomize_va_space=2`, `kernel.kptr_restrict=2`, `net.ipv4.conf.all.rp_filter=1`, disable IP forwarding unless required.
- [ ] Apply MAC controls: SELinux (enforcing) or AppArmor (enforce profile).
- [ ] Patch policy: critical CVEs ≤ 7 days, high ≤ 30 days; unattended-upgrades for security only.
- [ ] Auditd / EventLog enabled, forwarded to SIEM, retained ≥ 90 days.
- [ ] Disable USB / autorun / auto-mount on servers.
- [ ] Restrict cron / at to allow-listed users.
- [ ] Time sync via authenticated NTP (chrony with NTS).
- [ ] Bootloader password set; secure boot enabled where supported.
- [ ] Full-disk encryption (LUKS, BitLocker, FileVault) on anything mobile or non-datacenter.

### 3.4 User accounts, identity, and access management

- [ ] Remove all default accounts and default passwords.
- [ ] Enforce MFA on every interactive account (admin, developer, service-with-human-fallback). Prefer phishing-resistant MFA (FIDO2/WebAuthn) over TOTP, TOTP over SMS.
- [ ] Password policy aligned to NIST SP 800-63B: ≥ 12 chars, no composition rules, no forced periodic rotation, breach-corpus check (HIBP API).
- [ ] **Principle of least privilege**: every identity has the minimum permissions to do its job; review quarterly.
- [ ] **Role-based or attribute-based access control** centralized; no scattered `if user.is_admin` checks.
- [ ] Just-in-time elevation (sudo with logging, AWS IAM Identity Center sessions, PAM tools).
- [ ] Service accounts: no human login, rotated credentials, scoped roles, no shared service accounts across systems.
- [ ] Off-boarding playbook: identity disabled within 1 hour of departure, audit run weekly for orphaned accounts.
- [ ] Privileged Access Management (PAM) for break-glass accounts; vault sealed; access logged.

### 3.5 Network hardening

- [ ] **Default deny** at every layer: egress, ingress, east-west.
- [ ] Segmentation: prod vs non-prod, app vs data, DMZ vs internal. VPC subnets, security groups, network policies.
- [ ] No `0.0.0.0/0` rules except where explicitly justified and documented.
- [ ] WAF in front of every public HTTP endpoint; tuned rules, not just defaults.
- [ ] DDoS protection (CloudFront, Cloudflare, AWS Shield, equivalent).
- [ ] DNS: DNSSEC where possible, CAA records pinned, registrar locked, registrar MFA enforced.
- [ ] All admin planes (SSH, RDP, K8s API, DB) behind VPN or zero-trust proxy (Tailscale, Cloudflare Access, Teleport). **Never** publicly exposed.
- [ ] mTLS for service-to-service traffic in zero-trust networks.
- [ ] Egress filtering: only allow outbound to known-good destinations; this stops most exfiltration and C2.

### 3.6 Cloud hardening (AWS / Azure / GCP)

Apply the relevant CIS Cloud Foundations Benchmark.

- [ ] Root account: hardware MFA, no API keys, alarms on use.
- [ ] CloudTrail / Activity Log / Audit Log: all regions, all services, log validation enabled, sent to immutable bucket in separate account.
- [ ] Public storage: zero public S3/Blob/GCS buckets unless explicitly designated; Block Public Access at account level.
- [ ] Encrypt every storage service at rest with customer-managed keys where required by data class.
- [ ] Network: default VPC unused, flow logs on, private endpoints/VPC endpoints for service traffic.
- [ ] IAM: no inline policies, no `*` action in production, permission boundaries, SCPs at org level.
- [ ] Tag every resource with owner, env, data class, cost center. Untagged = quarantined.
- [ ] CSPM (cloud security posture management) tool continuously scanning: Wiz, Prisma Cloud, AWS Security Hub, Defender for Cloud.
- [ ] Region restriction via SCP — block regions you do not use.

### 3.7 Container & Kubernetes hardening

- [ ] Base images: distroless or minimal (Alpine, distroless, Chainguard); scanned in CI; signed.
- [ ] No root user inside containers; `USER` directive set; capabilities dropped.
- [ ] Read-only root filesystem; writable paths explicit.
- [ ] Resource limits (CPU, memory) and requests on every workload — prevents noisy neighbor & DoS.
- [ ] Pod Security Admission set to `restricted` for non-system namespaces.
- [ ] NetworkPolicies default-deny; explicit allow per service.
- [ ] Secrets via external secret manager (Vault, AWS Secrets Manager, Sealed Secrets) — never raw `Secret` objects in Git.
- [ ] Image admission control: block unsigned, block known CVEs above threshold (Kyverno, OPA Gatekeeper, Connaisseur).
- [ ] kube-apiserver: no anonymous auth, RBAC enforced, audit log on.
- [ ] etcd encrypted at rest; access restricted to control plane.
- [ ] Service mesh (Istio, Linkerd) for mTLS if running multi-tenant.
- [ ] Runtime threat detection (Falco, Tetragon).

### 3.8 Application hardening

- [ ] Input validation on **every** boundary using allow-lists, not deny-lists.
- [ ] Output encoding contextual to sink (HTML, attribute, JS, URL, SQL).
- [ ] Parameterized queries / prepared statements for **every** DB interaction.
- [ ] CSRF tokens on every state-changing endpoint that uses cookies.
- [ ] CORS: explicit origins, no `*` with credentials.
- [ ] Security headers: `Content-Security-Policy`, `Strict-Transport-Security` (preload), `X-Content-Type-Options: nosniff`, `Referrer-Policy: strict-origin-when-cross-origin`, `Permissions-Policy`, `X-Frame-Options: DENY`.
- [ ] Session cookies: `HttpOnly`, `Secure`, `SameSite=Lax` or `Strict`.
- [ ] Rate limiting per endpoint (especially auth, password reset, search).
- [ ] Account lockout / progressive delay on auth failures.
- [ ] No verbose errors to client (no stack traces); correlation IDs for support.
- [ ] File uploads: MIME sniffing, magic-byte check, size cap, antivirus scan, store outside web root, never serve from same origin.
- [ ] SSRF defenses: block link-local (`169.254.169.254`), private CIDRs, by default in any URL fetcher.
- [ ] Server-side request forgery test for every URL-taking endpoint.
- [ ] No insecure deserialization (no `pickle`/`unserialize` on untrusted input).
- [ ] Crypto: use vetted libraries (`libsodium`, `cryptography`), never `MD5`, `SHA1`, `DES`, `RC4`, ECB mode, custom protocols.

### 3.9 API hardening

- [ ] OpenAPI / spec is the contract; reject anything outside spec.
- [ ] Per-endpoint authentication and authorization — never "the gateway will catch it."
- [ ] Resource-level authorization checks (BOLA / IDOR).
- [ ] Input bound for arrays, strings, depth, batch size.
- [ ] Pagination required on list endpoints; no `?limit=10000` allowed.
- [ ] Versioning strategy (URL or header) with sunset policy.
- [ ] Token lifetime minimal; rotation supported.
- [ ] Replay protection on sensitive endpoints (nonce + timestamp).
- [ ] Webhook signing (HMAC-SHA256) for outbound webhooks; verify on inbound.

### 3.10 Database hardening

- [ ] Network-restricted: DB never on public internet. Private subnet + security group.
- [ ] App connects with **least-privilege** user, scoped to schemas/tables it actually needs. No `GRANT ALL`.
- [ ] Separate users for migrations, app, reporting.
- [ ] Encryption at rest (TDE, KMS-managed keys).
- [ ] TLS in transit, certificate pinning at app layer where feasible.
- [ ] Backups encrypted, stored in different account/region, **restore tested quarterly**.
- [ ] Audit log: DDL, privileged queries, failed logins.
- [ ] Schema constraints: `NOT NULL`, `CHECK`, `UNIQUE`, `FOREIGN KEY` — push integrity to the DB, never trust app code alone.
- [ ] Slow query log on; reviewed weekly.
- [ ] Row-level security (RLS) for multi-tenant data.

### 3.11 Secrets management

- [ ] Centralized secret store (Vault, AWS/GCP/Azure Secret Manager, 1Password Connect, Doppler).
- [ ] No secrets in code, config files, env vars in Dockerfiles, CI logs, or chat.
- [ ] Pre-commit hooks (`gitleaks`, `trufflehog`) blocking commits with secrets.
- [ ] Rotation: short-lived dynamic credentials wherever possible; static secrets rotated on schedule.
- [ ] Each secret has owner, purpose, rotation cadence, last-rotated date.
- [ ] Audit log on every fetch.

### 3.12 Supply chain hardening (OWASP A03:2025)

- [ ] SBOM generated on every build; stored alongside artifact.
- [ ] Dependencies pinned by version **and** hash (`pip` `--require-hashes`, `npm` lockfile w/ integrity, `go.sum`).
- [ ] Dependency proxy / private registry — no direct pulls from public registries to prod builds.
- [ ] SCA in CI; block PRs with critical / high CVEs.
- [ ] Build provenance: SLSA Level 3+ where feasible; signed attestations (Sigstore Cosign, in-toto).
- [ ] Reproducible builds where possible.
- [ ] Code signing for distributed binaries.
- [ ] Verify upstream maintainers / packages — typo-squatting attacks (`reqeusts`, `colourama`) are common.
- [ ] Internal package names registered on public registries to defeat dependency confusion.

### 3.13 Logging, monitoring, observability

- [ ] Structured logs (JSON) with consistent schema across services.
- [ ] Required fields: timestamp (UTC), service, version, env, request ID, user/tenant ID, severity.
- [ ] **Never log secrets, full PII, full tokens, full card numbers.** Redact at the logger layer.
- [ ] Centralized log aggregation (ELK, Loki, Splunk, Datadog) with retention by data class.
- [ ] Metrics (RED: rate, errors, duration; USE: utilization, saturation, errors).
- [ ] Distributed tracing (OpenTelemetry) — every cross-service call propagates trace context.
- [ ] Alerts tied to SLOs, not raw metrics; alert on symptom not cause.
- [ ] Tamper-evident logs for audit (write-once, hash-chain, separate account).
- [ ] SIEM with correlation rules: brute force, lateral movement, data exfiltration patterns.

### 3.14 Backup, disaster recovery, business continuity

- [ ] **3-2-1 rule**: 3 copies, 2 media, 1 off-site / different account / different cloud.
- [ ] Backups encrypted, tested by **actual restore drill** at least quarterly. An untested backup is not a backup.
- [ ] RPO and RTO documented per system; gap to current state measured.
- [ ] Immutable backups (S3 Object Lock, Azure immutable blobs) for ransomware defense.
- [ ] Runbooks for every disaster scenario; rehearsed annually.

### 3.15 Incident response readiness

- [ ] Documented IR plan covering detection, triage, containment, eradication, recovery, lessons-learned.
- [ ] On-call rotation defined; pager works; escalation policy clear.
- [ ] Forensic capability: can you snapshot a host, preserve memory, pull logs, freeze a credential — *fast*?
- [ ] Comms plan (internal, customer, regulator) pre-written with templates.
- [ ] Post-incident reviews (blameless) within 5 business days; action items tracked.

### 3.16 Compliance mapping

If your system is in scope for any of the following, map your hardening checklist to its controls so audits become routine, not panic:
- **NIST CSF 2.0** (Identify, Protect, Detect, Respond, Recover, Govern)
- **NIST SP 800-53** / **800-171**
- **ISO/IEC 27001:2022**
- **PCI DSS 4.0** (if you touch cardholder data)
- **HIPAA** Security Rule (if you touch PHI)
- **SOC 2** Type II
- **GDPR** / **CCPA** / regional data laws

CIS Benchmarks map cleanly to most of these — pick CIS as your operational baseline and the compliance mapping mostly falls out.

---

## 4. Code Quality Hardening — Bugs, Dead Code, Redundancy, Tech Debt

Security alone does not harden a system. A secure system full of bugs and tech debt will rot in 18 months. This section is the engineering-quality counterpart.

### 4.1 The taxonomy of technical debt

Tech debt is not a single thing. Identify the type before treating it:

1. **Intentional / strategic debt** — knowingly taken to ship a deadline, with a documented payback plan.
2. **Unintentional debt** — created by inexperience, missing context, or oversight. Most debt.
3. **Outdated design debt** — yesterday's clever design is today's bottleneck because requirements changed.
4. **Environmental debt** — frameworks, libraries, runtimes, OSes have aged out.
5. **Architectural debt** — structural / foundational; the most expensive kind, distinct from code-level debt.

### 4.2 Bug hunting — the systematic sweep

Run all of these. They overlap; that is the point.

- [ ] **Static analysis (SAST):** language-appropriate (`semgrep`, `ruff`, `mypy --strict`, `eslint`, `tsc --noEmit`, `clippy`, `go vet`, `staticcheck`, `SonarQube`, `CodeQL`). Configure for your codebase; turn off rules that don't apply, but don't disable what's just inconvenient.
- [ ] **Type system tightened.** TypeScript strict, Python `mypy --strict`, Go without `interface{}` leakage. A type error caught at build is a runtime bug avoided.
- [ ] **Dynamic analysis (DAST):** OWASP ZAP / Burp / Nuclei against running app; per release.
- [ ] **Fuzzing:** `libFuzzer`, `AFL++`, `go-fuzz`, `Atheris`, `cargo-fuzz`, `Jazzer`. Fuzz every parser, every untrusted-input boundary, every deserializer. The 2025 OWASP A10 entry is here for a reason.
- [ ] **Property-based testing:** Hypothesis, fast-check, Quickcheck, proptest. Replaces "can you think of a test case?" with "can the system find one that breaks the invariant?"
- [ ] **Mutation testing:** Stryker, Pitest, mutmut. If your tests still pass after mutating the code under test, your tests are weak.
- [ ] **Race / concurrency testing:** `-race` flag (Go), ThreadSanitizer (C/C++), Loom (Rust), `pytest-xdist` repeated runs, controlled chaos.
- [ ] **Memory safety (where applicable):** ASan, MSan, Valgrind, leak detectors.
- [ ] **Code review checklist** applied to **every PR**: see §11.3.

### 4.3 Dead code detection — find it and delete it

Dead code is liability with no benefit: bigger attack surface, slower builds, confused readers, false test coverage.

- [ ] Run unused-export / unused-symbol tooling: `ts-prune`, `deadcode` (Go), `vulture` (Python), `cargo-udeps` (Rust), `knip` (JS/TS), IDE inspections.
- [ ] **Coverage at runtime, not just from tests.** Tools like Coverity, JaCoCo with production agent, Python `coverage` in canary. Endpoints that show 0 hits in a month of prod traffic are candidates.
- [ ] Search for feature flags that are 100% on or 100% off for > 90 days — both branches need a decision (delete the flag and one branch).
- [ ] Find commented-out code blocks; delete them. Git remembers; comments lie.
- [ ] Search for `TODO`, `FIXME`, `XXX`, `HACK` markers older than 12 months — convert to issues or delete.
- [ ] Identify orphaned files: not imported anywhere, not entry points, not tests.
- [ ] Identify orphaned database columns, tables, indexes — query `pg_stat_user_tables` / equivalent for tables with zero reads/writes over a long window.
- [ ] **Critical caveat:** dead code can have indirect consumers (reflection, dependency injection, dynamic dispatch, generated SDKs, dashboards, scripts). Verify before deleting; delete in a feature branch and run full integration + manual smoke; deploy behind a flag if uncertain.

### 4.4 Redundant / duplicated code

- [ ] Use duplication detectors: `jscpd`, `simian`, SonarQube duplication, `dupl` (Go).
- [ ] Distinguish coincidental duplication (looks the same now, will diverge) from semantic duplication (same intent in two places — extract).
- [ ] Apply DRY *judiciously*. Premature abstraction is also debt. Three or more occurrences with proven shared intent → extract.
- [ ] Watch for redundant logic across services in microservice systems — the same email-validation regex lives in 12 places.

### 4.5 Code smells — the canonical list

Map every smell to a refactoring (Fowler's catalog is the reference):

| Smell | Refactoring |
|-------|-------------|
| Long method (> ~30 lines or > 1 reason to change) | Extract Method |
| Long parameter list (> 4) | Introduce Parameter Object / Builder |
| Large class / God Object | Extract Class / split responsibilities |
| Feature envy (method uses another class's data more than its own) | Move Method |
| Data clumps (same 3-4 fields traveling together) | Extract Class |
| Primitive obsession (raw strings/ints for domain concepts) | Replace with Value Object |
| Switch/if cascades on type | Replace Conditional with Polymorphism |
| Shotgun surgery (one change touches many files) | Move Method/Field, consolidate |
| Divergent change (one file, many reasons to change) | Extract Class |
| Parallel inheritance hierarchies | Move members, merge hierarchies |
| Lazy class (does too little to justify existing) | Inline Class |
| Speculative generality (built for needs that never materialized) | Inline / collapse |
| Temporary field (set in some flows, null in others) | Replace with Method, Introduce Null Object |
| Message chains (`a.getB().getC().getD()`) | Hide Delegate |
| Middle man (class only delegates) | Remove Middle Man |
| Inappropriate intimacy (two classes share too many internals) | Move Method, extract module |
| Comments explaining what code does | Rewrite code so comment is unnecessary |
| Magic numbers / strings | Replace with Named Constant |
| Mutable shared state | Make immutable, encapsulate, use channels/locks |

### 4.6 Complexity metrics — measure to manage

- **Cyclomatic complexity** per function: target ≤ 10, alarm at > 15.
- **Cognitive complexity** (SonarQube): different from cyclomatic, captures human readability.
- **Function length:** target ≤ 30 lines.
- **File length:** target ≤ 500 lines for most languages.
- **Class size:** target ≤ 200 lines / ≤ 7 public methods.
- **Halstead metrics** for math-heavy modules.
- **Maintainability Index** as a rolled-up signal.
- **Coupling (afferent/efferent)** and **instability** (`I = Ce / (Ca + Ce)`) at module level.
- **Test coverage** target depends on context; line coverage is weak, branch coverage stronger, mutation score strongest.

### 4.7 Refactoring patterns (Fowler / Kerievsky)

The standard catalog every developer should know:

- Extract Method, Inline Method, Extract Variable, Inline Variable
- Rename (Variable / Method / Class) — boring but the highest-leverage refactoring
- Move Method, Move Field, Move Class
- Extract Class, Inline Class
- Replace Conditional with Polymorphism
- Replace Temp with Query
- Introduce Parameter Object
- Replace Magic Number with Symbolic Constant
- Encapsulate Field, Encapsulate Collection
- Replace Inheritance with Delegation
- Replace Type Code with State/Strategy
- Decompose Conditional, Consolidate Conditional Expression
- Replace Error Code with Exception, Replace Exception with Test
- Strangler Fig (for incrementally replacing a legacy module/service)

### 4.8 The "Boy Scout" rule, codified

Leave every file you touch cleaner than you found it. In practice:
- Rename one bad name.
- Delete one stale comment / commented-out block.
- Extract one too-long method.
- Tighten one type or assertion.
- Replace one magic number.

Compounded across a team, this **is** debt repayment.

### 4.9 Tooling stack (minimum viable)

- **Linter + formatter** enforced in pre-commit and CI (no merging without green).
- **Type checker** in CI on strict mode.
- **SAST** (Semgrep / CodeQL / SonarQube).
- **SCA** (Snyk / Dependabot / OSV-Scanner / Trivy).
- **Secret scan** (gitleaks, trufflehog) in pre-commit *and* CI *and* historical.
- **Test runner** with coverage, branch coverage, and mutation testing periodically.
- **Code quality dashboard** (SonarQube, CodeClimate, Codacy) with debt trend over time.

---

## 5. Architecture Hardening — Structural Failures and Anti-Patterns

Code-level fixes will not save a broken architecture. This section is for the architecture review that should accompany any "harden the system" work.

### 5.1 The six dimensions of architectural review

Score the system honestly on each, 1–5. Anything ≤ 2 is a hardening priority.

1. **Scalability** — can it handle 10× traffic, 100× data? Where is the next bottleneck?
2. **Security** — are trust boundaries explicit? Are controls layered?
3. **Maintainability** — can a new engineer ship a feature in week 1?
4. **Performance** — is it within the latency budget at p95 and p99 under realistic load?
5. **Deployability** — can you roll out, roll back, and run multiple versions safely?
6. **Observability** — when something breaks at 3 AM, can the on-call see why in five minutes?

### 5.2 Architectural anti-patterns — the watchlist

If you see any of these, you have found hardening work.

- **Big Ball of Mud** — no recognizable structure; everything talks to everything.
- **Distributed monolith** — microservices that must all be deployed together; combines distribution complexity with monolith rigidity. *Often the worst architecture in industry.*
- **Shared database across services** — multiple services reading and writing the same tables, creating hidden coupling that breaks independent deployment.
- **God service / God class** — one component owns 60%+ of business logic and traffic; single point of failure and contention.
- **Stovepipe** — point-to-point integrations between systems with no shared contract or strategy.
- **Missing circuit breakers** — one slow downstream takes down the whole graph.
- **Synchronous chains 4+ deep** — p99 latency multiplies; failure probability multiplies.
- **No bulkheads** — one customer / tenant can starve all others (no per-tenant quotas, shared thread pools).
- **The Blob / God Object** — class with hundreds of methods or thousands of lines.
- **Spaghetti code** — control flow that can't be followed without a debugger.
- **Lava flow** — old code nobody understands, "do not touch" comments, deepening sediment.
- **Golden hammer** — one tool/pattern used for every problem regardless of fit.
- **Vendor lock-in by accident** — proprietary APIs woven through business logic with no abstraction.
- **Overengineering / Inner-Platform Effect** — building a configurable system that ends up reinventing the platform it runs on.
- **Reinvented wheel** — handwritten crypto, retry logic, queue, ORM, etc., when a battle-tested library exists.
- **Cargo cult architecture** — adopted because "Netflix does it," not because the constraints match.
- **Premature optimization** — micro-optimizations before profiling.
- **Premature abstraction** — interfaces with one implementation, factories with no need.
- **Boat anchor** — keeping a deprecated component "just in case."
- **Magic Pushbutton** — UI tightly coupled to business logic; impossible to script or test.
- **Anemic Domain Model** — domain objects are just data bags; logic scattered in "service" classes.

### 5.3 Coupling and cohesion

- High **cohesion** (a module does one thing well) — desirable.
- Low **coupling** (modules depend on each other minimally and through stable interfaces) — desirable.
- Acyclic dependency graph at the module/package/service level — required.
- The Stable Dependencies Principle: depend in the direction of stability.
- The Stable Abstractions Principle: stable modules should be abstract; concrete modules should be unstable.

### 5.4 Failure-mode analysis (FMEA, lite)

For every external dependency (database, queue, third-party API, DNS, time, network):
1. What happens if it is **slow**?
2. What happens if it is **unavailable**?
3. What happens if it returns **wrong** data?
4. What happens if it is **partially** available (50% errors)?
5. What happens if it is **flaky** (1% errors)?

Each answer must be either a control (timeout, circuit breaker, retry with backoff and jitter, bulkhead, fallback) or a documented accepted risk.

### 5.5 Single points of failure

- [ ] Single DB primary with no replica? Single AZ? Single region?
- [ ] Single load balancer with no failover?
- [ ] Single CI server holding deploy credentials?
- [ ] Single human who is the only one who knows X (the bus-factor problem)?
- [ ] Single shared cache that takes down everything when cold?
- [ ] Single auth provider with no fallback for outages?

Document the SPOFs you accept and have a runbook for each.

### 5.6 Capacity & scaling review

- [ ] Load testing at 2× current peak; identify the next bottleneck.
- [ ] Latency budget allocated per hop; sums to user SLA.
- [ ] Horizontal scaling story for each tier; vertical scaling has a known ceiling.
- [ ] Database growth projected 12 months out; storage, IOPS, connection limits.
- [ ] Cache hit rates monitored; cache stampede protection (singleflight, jittered TTL).
- [ ] Background job backlogs alerted; workers scale independently.

### 5.7 Data architecture review

- [ ] Data ownership is clear: one service owns each table.
- [ ] Schema evolution plan: backward-compatible migrations, expand-contract pattern.
- [ ] Consistency model documented: strong vs eventual, transaction boundaries.
- [ ] Idempotency keys on every mutating endpoint that can be retried.
- [ ] Soft-delete vs hard-delete decided per table; retention aligned to legal/compliance.
- [ ] PII map: where does PII live, who has access, how is it deleted on request?

### 5.8 Deployability hardening

- [ ] Build is reproducible.
- [ ] Deploys are automated; manual steps are bugs.
- [ ] Blue-green or canary or rolling; never big-bang on prod.
- [ ] Rollback is tested and < 5 minutes.
- [ ] Database migrations are forward and backward compatible across at least one version.
- [ ] Feature flags used for risky changes; flag retirement tracked.

---

## 6. Full State Verification (FSV) — The Non-Negotiable Protocol

This is the discipline that distinguishes "I ran the test and it passed" from "I have **proof** the system did what it was supposed to do."

> **Core principle:** *Return values lie. Logs lie. The Source of Truth does not lie. Always inspect the SoT.*

### 6.1 The four steps of FSV

**Step 1 — Define the Source of Truth.**
Before executing any test, write down:
- *What* state should change?
- *Where* does that state live (DB table + column, file path, queue name, S3 key, metric, external system ID)?
- *How* will I read it (SQL, file read, API call, dashboard query)?
- *What is the expected value* (exact value, range, schema, count delta)?

If you cannot answer these four, you do not understand the feature well enough to test it. Stop and learn it.

**Step 2 — Capture the BEFORE state.**
Read the SoT *first*. Log the value. This is your baseline.

```sql
-- BEFORE
SELECT id, status, updated_at FROM orders WHERE id = 'test-order-1';
```

**Step 3 — Execute the trigger.**
Run the action: call the API, send the message, click the button, run the script. Capture the response. The response is **evidence of attempt**, not evidence of success.

**Step 4 — Capture the AFTER state and assert.**
Read the SoT again. Compare against expected. Record the delta. This is your evidence of success or failure.

```sql
-- AFTER
SELECT id, status, updated_at FROM orders WHERE id = 'test-order-1';
-- Expected: status = 'paid', updated_at > BEFORE.updated_at
```

If the response said "200 OK" but the row is unchanged, the test **failed** — even if the code "ran without errors." This is the most common silent bug class in modern systems.

### 6.2 The verification chain

Most real features write to multiple SoTs. **Verify every one.**

Example: "user submits an order."
- DB: `orders` row inserted with correct fields.
- DB: `order_items` rows inserted, count matches cart.
- DB: `inventory.available` decremented by the right amount.
- Queue: `order.created` event published with correct payload.
- External: Stripe charge created with correct amount.
- Email: confirmation email queued (check `outbox` table) and eventually sent.
- Metric: `orders_created_total` counter incremented.
- Log: structured log entry with order ID and user ID.

Each of those is a separate read against a separate SoT. Skipping any one is how production bugs ship.

### 6.3 Boundary & edge case audit (mandatory)

For every code path, simulate **at minimum** three edge cases. Print state before and after for each.

Required categories:

1. **Empty input** — `""`, `[]`, `{}`, `null`, missing field.
2. **Single-item input** — sometimes the off-by-one only shows here.
3. **Maximum allowed input** — the documented upper bound.
4. **One past the maximum** — should be rejected cleanly.
5. **Minimum allowed** — `0`, `1`, the documented lower bound.
6. **One below the minimum** — `-1`, `0`.
7. **Wrong type** — string where int expected, etc.
8. **Malformed format** — invalid JSON, invalid UTF-8, invalid email.
9. **Unicode edge cases** — emoji, RTL text, combining characters, NULL bytes, very long strings.
10. **Duplicate / replay** — same idempotency key twice; re-submission of the same form.
11. **Out-of-order** — event B arrives before event A.
12. **Concurrent** — two actors do the same thing at the same instant.
13. **Authorization variants** — owner, non-owner, admin, anonymous.
14. **Tenant / scope** — tenant A cannot see tenant B's data.
15. **Time edges** — DST transition, leap second, timezone with negative offset, end-of-month.

For each, the log must show:
- The synthetic input used.
- The SoT state **before**.
- The action taken.
- The SoT state **after**.
- The assertion result (PASS/FAIL with expected vs actual).

### 6.4 Evidence of success — the deliverable

A passing FSV run produces a log artifact. Minimum contents:

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

Without that artifact, hardening work is not complete.

### 6.5 What FSV is *not*

- It is not a unit test that mocks the database.
- It is not a code review.
- It is not "the CI passed."
- It is not "I tested it on my machine and it looked right."

It is a **read against the real SoT** showing the real change.

---

## 7. Manual Testing Methodology — Happy Path + Edge Cases + Synthetic Data

Automation is great. It is also incomplete. Manual testing — done well — finds the bugs the test pyramid misses.

### 7.1 Why manual matters

- Automated tests verify what you thought to test. Manual exploration finds what you didn't.
- Edge cases at the *intersection* of features rarely have automated coverage.
- "Worked on my machine" is broken by manual testing in a real environment.
- FSV (§6) is most rigorous when done by hand the first time, then partially automated.

### 7.2 Synthetic test data — the controlled experiment

Synthetic data is fabricated input where you **know in advance** what the output should be. This is the only way to write a deterministic verification.

Properties of good synthetic data:
- **Deterministic seed** — same input → same expected output, every run.
- **Distinguishable** — values like `synthetic_test_user_2026_05_08` so you can find them in the DB and clean them up.
- **Representative** — same shape and distribution as real data.
- **Boundary-rich** — includes the edge cases from §6.3.
- **Privacy-safe** — generated, never copied from production.
- **Cleanup-tagged** — can be deleted at the end of the run by a single query.

Example principle: *if `2 + 2 = 4`, then sending `{a: 2, b: 2}` to `/add` must produce `4` AND a row in `calculations` with `result = 4`.* The synthetic value gives you the expected output; FSV verifies the SoT.

### 7.3 Happy path testing

The happy path is the documented intended flow. For each user story / endpoint / job:
1. Set up the typical preconditions.
2. Perform the canonical action.
3. Verify the canonical outcome via FSV.

Cover every primary user story this way before moving on. If happy paths are broken, edge cases are noise.

### 7.4 Edge case testing — the systematic sweep

Apply **boundary value analysis (BVA)**: errors cluster at boundaries. For every numeric or sized input range `[min, max]`, test:
- `min - 1` (invalid)
- `min` (valid)
- `min + 1` (valid)
- a typical mid value
- `max - 1` (valid)
- `max` (valid)
- `max + 1` (invalid)

Combine with **equivalence class partitioning (ECP)**: divide the input domain into classes where members are treated equivalently; pick one representative per class plus the BVA values.

Also cover:
- All combinations from §6.3.
- Negative testing — does the system reject what it should?
- Authorization testing — does each role see exactly what it should and nothing more?
- Concurrency — same action by two actors at the same instant.
- Network failures — kill the DB / queue / external API mid-flow; resume; verify consistency.
- Partial failures — retries, idempotency, dedup keys.
- Time travel — clock skew, large jumps, DST.
- Resource exhaustion — full disk, OOM, connection pool exhausted, rate-limit hit.

### 7.5 The "trigger → process → outcome" verification pattern

For every test:

1. **Identify the trigger.** What event starts the process? (HTTP request, cron, queue message, file watch, timer.)
2. **Identify what makes it observable.** Logs, metrics, traces.
3. **Identify the process.** What code path runs? What does it touch?
4. **Identify the outcome.** What state changes? Where? (The SoT.)
5. **Identify how the outcome is verifiable.** Query, file check, dashboard, API.
6. **Run the trigger with synthetic data.**
7. **Read the SoT.**
8. **Assert the outcome matches expectation.**

If at any step you cannot answer the question, you have found a hardening item — usually a missing log, missing metric, or unclear data ownership.

### 7.6 What to do when a test fails

Stop. Do not "rerun and hope." Do not patch over. Apply §8 — root cause analysis — *before* writing any fix.

When the fix is in:
- Update the failing test so it would catch the bug now.
- Add a regression test pinned to the bug ID.
- Re-run **all** related FSV scenarios — fixes commonly break adjacent paths.
- Verify the fix in the SoT, not just the green test.

### 7.7 Handling disposable test data

Synthetic data must not pollute real environments. Either:
- Run in an isolated test DB / namespace, dropped at end of run.
- Tag every synthetic record with a known marker; delete by query at end.
- Use transactions that always roll back (only when the system supports it cleanly).

Document the cleanup; verify cleanup ran (FSV applies to cleanup too).

---

## 8. Root Cause Analysis — First Principles, 5 Whys, Fishbone

When something goes wrong, the goal is **never** "make the symptom stop." The goal is *the smallest structural change that makes this class of failure impossible.* That requires RCA.

### 8.1 The 5 Whys

Originated at Toyota by Sakichi Toyoda; popularized by Taiichi Ohno's example:

> 1. Why did the machine stop? — A fuse blew from overload.
> 2. Why was there an overload? — The bearing was not lubricated enough.
> 3. Why was it not lubricated? — The lubrication pump wasn't pumping enough.
> 4. Why was it not pumping enough? — The pump shaft was worn and rattling.
> 5. Why was the shaft worn? — There was no strainer; metal scraps got in.

Without the fifth Why, you replace the fuse and the failure recurs. With it, you install a strainer and the entire failure mode is eliminated.

**Rules for good 5 Whys:**
- Use evidence, not opinion. "I think" doesn't count; logs, timestamps, code do.
- The number 5 is approximate. Sometimes 3, sometimes 7.
- When the answer is "someone forgot," go deeper: *why did the system allow human memory to be the only safeguard?* That's the systemic question.
- Multiple branches are okay (use a fishbone if so).
- Validate by asking: would fixing the alleged root cause have prevented the symptom? If not, dig more.

### 8.2 First-principles thinking applied to debugging

The 5 Whys is a discovery method. First principles is a **rebuilding** method.

1. State the symptom precisely, with evidence.
2. List every assumption that, if true, would make the symptom impossible. ("I assumed the request reached the server." "I assumed the user ID was correct." "I assumed the DB write committed.")
3. Verify each assumption — by inspection, not reasoning. Look at logs, traces, the SoT.
4. The first one that fails is your suspect; recurse.
5. Once found, ask: *what fact, if structurally true, would make this assumption self-evidently safe?* (Example: "the request must reach the server" → enforce by health check + circuit breaker + dead-letter on the client side.)

### 8.3 Fishbone (Ishikawa) diagrams

When multiple causes interact, the linear 5 Whys is too narrow. Use a fishbone:

- Spine: the problem statement.
- Major bones (the categories): typically **Methods, Machines, Materials, Measurement, Environment, People** (the "6 Ms"); for software, adapt to **Code, Data, Config, Infrastructure, Process, People**.
- Sub-bones: contributing factors under each category.
- For each plausible factor, run mini-5-Whys.

### 8.4 Fault tree analysis

For high-stakes failures (security, safety, reliability):
- Top event: the failure.
- AND/OR gates decomposing what combinations of conditions produce it.
- Leaves: basic events with measurable probabilities.
- Useful for quantitative risk and for finding non-obvious AND-paths that low individual probabilities can mask.

### 8.5 Anti-patterns in RCA

- **Stopping at "human error."** Always ask: why did the system permit it?
- **Stopping at the first plausible cause.** Multiple causes often coexist.
- **Confusing correlation with causation.** Verify the causal mechanism, don't just match timing.
- **No follow-up.** Schedule a check 30–60 days later: did the fix hold? If the symptom recurred, the RCA was wrong.
- **Blaming.** Blameless culture is not optional — it changes whether information surfaces.

### 8.6 Output of an RCA

Every meaningful RCA produces:
- Timeline (with timestamps and evidence sources).
- Symptoms observed.
- Causal chain (5 Whys / fishbone / fault tree).
- Root cause stated as a *system property* ("the system allowed X because Y").
- Action items: a fix for the immediate cause, a fix for the root cause, a detection improvement, a prevention improvement.
- Owner and due date for each.
- Re-verification plan.

---

## 9. Capability Optimization — Extracting Maximum Value From Each Component

Hardening is not just defensive. The best hardening sessions also **upgrade** components by asking: *what is this part of the system **for**, and is it delivering the maximum it could?*

### 9.1 The four strategic questions

For every module, service, library, or feature being hardened, answer:

1. **What does this contribute to the system?** Stated in user-visible terms — not "it parses JSON" but "it lets users import data without the type errors that caused 40% of support tickets."
2. **What does the system need from this?** SLAs, throughput, accuracy, security posture, extensibility.
3. **What capability was it intended to enable?** Look at the original ticket / RFC / commit message. Often the implementation drifted from intent.
4. **What is the maximum it could deliver if optimized?** Imagine the best-in-class version. The gap is your roadmap.

### 9.2 The capability-extraction checklist

For the component under review:

- [ ] Are all originally-intended capabilities actually exposed and used?
- [ ] Are there capabilities the implementation accidentally has but doesn't expose? (Often performance-related: cache that could serve another query, index that could support another feature.)
- [ ] Are there capabilities the system needs that this could provide but currently doesn't?
- [ ] Is the public API at the right level of abstraction — is it harder to use than necessary, easier to misuse than necessary?
- [ ] Is this the cleanest seam to extend? If a future feature is anticipated, would a small refactor make it cheap?
- [ ] Are the latency / throughput / cost characteristics aligned to what the system actually needs, or accidental?
- [ ] Does the observability emit the signals the rest of the system would benefit from?
- [ ] Are there capabilities currently provided that **shouldn't** be — over-broad APIs invite misuse and lock in bad patterns?

### 9.3 Alignment with project intent

A subtle failure mode: the team optimizes a component for what's easy or interesting, not for what the project needs. Re-anchor every hardening pass to the project goals. If the project's core promise is "low-latency search," a hardening pass on the analytics pipeline shouldn't add 200 ms to search.

When in doubt, write down the project's top 3 promises (the user-visible ones) and check each hardening change against them: does this make those promises easier or harder to keep?

---

## 10. Operational Workflow — Putting It All Together

A repeatable sequence to run "harden the system" as a real engagement.

### 10.1 The hardening session, end to end

**Phase 1 — Discover (read-only).**
1. Build / verify asset inventory.
2. Read all docs, ADRs, READMEs, runbooks. Note gaps.
3. Generate dependency graph (`madge`, `pydeps`, language equivalent).
4. Generate metrics dashboard for the past 90 days (errors, latency, traffic, costs).
5. Read recent incidents & post-mortems.
6. Run the security scanners and the code-quality scanners (§3, §4) and capture baselines.
7. Walk the codebase visually; note the smells, the dead code, the anti-patterns, the SoTs.

**Phase 2 — Threat-model & risk-rank.**
1. STRIDE per data flow.
2. Architecture review across the six dimensions (§5.1).
3. List every finding from Phase 1 + threat model; rank by *likelihood × impact × ease-of-fix*.
4. Group findings into **must fix**, **should fix**, **nice to fix**, **document & accept**.

**Phase 3 — Plan & sequence.**
1. Order findings to maximize safety. Do not refactor without tests; do not add tests without understanding behavior.
2. Establish a **safety net first**: characterization tests around current behavior so refactoring is safe.
3. Slice work into PR-sized increments; each one shippable and reversible.
4. Define FSV evidence for each increment up front — what SoT, what synthetic data, what expected outcome.

**Phase 4 — Execute (the loop).**
For each increment:
1. Snapshot state (SoT before, metrics before).
2. Make the change.
3. Run all relevant tests.
4. Run **manual FSV** with synthetic data covering happy path + edge cases.
5. Inspect every SoT; capture evidence.
6. Deploy to staging; repeat FSV in staging.
7. Deploy to production behind a flag; canary; monitor; full rollout.
8. Verify post-deploy in production SoTs (read-only, never modifying prod data).
9. Update docs and runbooks.

**Phase 5 — Re-verify & report.**
1. Re-run baselines from Phase 1; quantify improvement.
2. Document remaining accepted risks; assign owners; schedule re-review.
3. Hand-off / changelog / post-mortem of the hardening pass itself.

### 10.2 Stop conditions — when to halt and re-plan

- A test fails unexpectedly — RCA before continuing.
- An assumption proves false — re-evaluate the increment.
- A finding turns out to be deeper than expected — re-rank, don't sneak it in.
- Production metrics shift after a deploy — investigate; rollback if unclear.

### 10.3 Definition of Done for a hardening task

- [ ] Code change merged.
- [ ] All linters / type checks / SAST / SCA green.
- [ ] Unit + integration + property tests added or updated.
- [ ] **FSV evidence captured** (before / trigger / after / assertion log).
- [ ] Edge cases tested manually (≥ 3 from §6.3 categories) with evidence.
- [ ] Docs / runbooks / ADRs updated.
- [ ] Observability updated — new metrics / logs / alerts as needed.
- [ ] Deployed to prod; post-deploy verification done.
- [ ] Risk register updated; any residual risk noted.

### 10.4 Continuous hardening — making it permanent

A one-time pass decays. Make it continuous:
- Weekly: dependency updates auto-PR'd and merged.
- Weekly: failed-login / anomalous-access review.
- Monthly: dead-code sweep, coverage and complexity trend review.
- Quarterly: backup restore drill, IR tabletop, threat model refresh, access review.
- Annually: full architecture review, DR exercise, third-party pentest.
- On every PR: review checklist (§11.3) applied.

---

## 11. Master Checklists (Copy-Paste Ready)

### 11.1 Daily / per-PR security & quality gate

- [ ] No secrets in diff (gitleaks clean).
- [ ] No new high/critical CVEs introduced (SCA clean).
- [ ] No new SAST findings above threshold.
- [ ] Tests added / updated.
- [ ] FSV evidence attached for any feature change.
- [ ] No new TODO / FIXME without an issue link.
- [ ] No magic numbers / strings without a named constant.
- [ ] No `console.log` / `print` / debug statements left in.
- [ ] Errors handled (no bare `catch(e){}` or `except: pass`).
- [ ] Inputs validated; outputs encoded.
- [ ] DB changes are reversible / forward-compatible.
- [ ] Observability signals added where state changed.
- [ ] Docs updated where behavior changed.

### 11.2 Architecture review checklist (per service)

- [ ] Single, clear responsibility documented.
- [ ] Public API spec'd; backward-compatibility policy explicit.
- [ ] Owner and on-call documented.
- [ ] Data ownership: every table/topic owned by exactly one service.
- [ ] No shared write access to another service's data.
- [ ] Failure modes for each external dependency: timeout, retry, circuit breaker, fallback, bulkhead, idempotency.
- [ ] Latency budget allocated and met at p95 / p99.
- [ ] Horizontal scaling demonstrated.
- [ ] Capacity headroom ≥ 2× peak.
- [ ] Deploy: canary or blue-green; rollback < 5 min.
- [ ] Observability: RED metrics, structured logs, traces, SLO-aligned alerts.
- [ ] Security: authn + authz on every endpoint, secrets in vault, least privilege.
- [ ] Backup + restore drill within 90 days.
- [ ] Runbook for the top 5 alerts.

### 11.3 Code review checklist

- [ ] Behavior matches spec / PR description.
- [ ] Names communicate intent.
- [ ] Functions ≤ ~30 lines; cyclomatic ≤ 10.
- [ ] No duplication of existing utilities.
- [ ] No dead code added.
- [ ] Errors propagated meaningfully; no silent swallowing.
- [ ] Edge cases covered by tests.
- [ ] No race conditions in shared state.
- [ ] No N+1 queries; pagination on lists.
- [ ] No PII / secrets in logs.
- [ ] No breaking API changes without a versioning plan.
- [ ] No new dependencies without justification + license review.

### 11.4 Database hardening checklist

- [ ] Private network only.
- [ ] TLS in transit, encryption at rest with managed keys.
- [ ] App user has minimum grants; separate users for migrations / reporting.
- [ ] Constraints: PK, NOT NULL, CHECK, FK, UNIQUE per business invariant.
- [ ] Indexes match query patterns; no unused indexes.
- [ ] Slow query log on; reviewed.
- [ ] Backup: encrypted, off-site, restore tested ≤ 90 days ago.
- [ ] Audit log for DDL and privileged ops.
- [ ] Connection pooling; no app-side connection leaks.
- [ ] Migration strategy: expand-contract, reversible, online for big tables.

### 11.5 Pre-production deploy checklist

- [ ] All tests green.
- [ ] FSV evidence attached.
- [ ] Migration plan reviewed; rollback plan reviewed.
- [ ] Canary plan: percentage, duration, success metrics.
- [ ] Alerts in place for the new behavior.
- [ ] Feature flag default set correctly.
- [ ] Comms: who needs to know? sent.
- [ ] Rollback button works (verified, not assumed).

### 11.6 Incident response readiness checklist

- [ ] On-call schedule current; pager tested.
- [ ] Runbooks current for top 10 alert types.
- [ ] Forensic snapshot capability tested.
- [ ] Customer comms templates pre-approved.
- [ ] Regulator comms templates pre-approved (if regulated).
- [ ] Tabletop exercise within last 6 months.
- [ ] Action items from last incident closed.

---

## 12. References & Authoritative Sources

Treat these as the canon. When in doubt, defer to them over any blog post.

**Security:**
- OWASP Top 10:2025 — `https://owasp.org/Top10/`
- OWASP ASVS 5.0 (Application Security Verification Standard) — `https://owasp.org/www-project-application-security-verification-standard/`
- OWASP Cheat Sheet Series — `https://cheatsheetseries.owasp.org/`
- OWASP Developer Guide, System Hardening section — `https://devguide.owasp.org/`
- CIS Benchmarks — `https://www.cisecurity.org/cis-benchmarks`
- CIS Critical Security Controls v8 — `https://www.cisecurity.org/controls`
- NIST Cybersecurity Framework 2.0 — `https://www.nist.gov/cyberframework`
- NIST SP 800-53 / 800-171 — `https://csrc.nist.gov/publications`
- NIST SP 800-63B (Digital Identity / Authentication) — `https://pages.nist.gov/800-63-3/sp800-63b.html`
- DISA STIGs — `https://public.cyber.mil/stigs/`
- CISA Secure by Design — `https://www.cisa.gov/securebydesign`
- ASD Essential Eight — `https://www.cyber.gov.au/`
- Sigstore / SLSA (supply chain) — `https://www.sigstore.dev/`, `https://slsa.dev/`
- Kubernetes Hardening Guide (NSA/CISA) — search "Kubernetes Hardening Guidance NSA CISA"

**Code quality & architecture:**
- *Refactoring* — Martin Fowler (the catalog of code smells and refactorings)
- *Working Effectively with Legacy Code* — Michael Feathers (characterization tests, seams)
- *Clean Architecture* — Robert C. Martin
- *A Philosophy of Software Design* — John Ousterhout
- *Building Evolutionary Architectures* — Ford, Parsons, Kua
- SonarQube / SonarCloud rules — `https://rules.sonarsource.com/`
- Microsoft Application Architecture Guide
- AWS Well-Architected Framework — `https://aws.amazon.com/architecture/well-architected/`
- Google SRE Book and Workbook — `https://sre.google/books/`

**Testing & verification:**
- *Continuous Delivery* — Humble & Farley
- ISTQB Foundation Level Syllabus (BVA, ECP, test design)
- *xUnit Test Patterns* — Gerard Meszaros
- Property-based testing: Hypothesis docs, fast-check docs
- Fuzzing: OSS-Fuzz, ClusterFuzz, AFL++

**Root cause analysis & lean:**
- Lean Enterprise Institute — 5 Whys lexicon — `https://www.lean.org/lexicon-terms/5-whys/`
- ASQ — Five Whys / Five Hows — `https://asq.org/quality-resources/five-whys`
- *The Field Guide to Understanding Human Error* — Sidney Dekker
- *Drift into Failure* — Sidney Dekker
- Toyota Production System literature — Taiichi Ohno

---

## Final Note: The One Thing to Remember

If you remember nothing else from this guide, remember this:

> **A function's return value is a claim. The Source of Truth is the verdict. Always read the verdict.**

Every other practice in this document is built around that single discipline. Security scanners can lie. Tests can pass on stale data. Logs can be missing. The row in the database — or the absence of it — does not lie.

Harden the system means: change the system so that it is harder to break, easier to understand, faster to fix, and provably correct *at the source of truth*, every time, forever.