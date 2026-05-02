# BlackBoxFix Planning Index

BlackBoxFix is the working codename for a Verified AgentOps platform:
a black box recorder and verifier for AI agents. The platform records every
agent action, verifies the claimed effect against the physical source of truth,
scores the action, supports replay and incident response, and exports verified
episodes for model training and evals.

The product should not become generic observability with AI labels. The core
asset is verifiable operational evidence: what the agent intended, what it did,
what persisted, what the verifier read back, and what should happen next.

## Planning Documents

- [01_product_vision.md](01_product_vision.md): product thesis, users, principles, non-goals.
- [02_market_and_buyer_research.md](02_market_and_buyer_research.md): buyers, urgency, wedge, competitors, pricing.
- [03_reference_architecture.md](03_reference_architecture.md): system components and deployment patterns.
- [04_event_ledger_schema.md](04_event_ledger_schema.md): immutable run/action/tool/verifier record contracts.
- [05_source_of_truth_connectors.md](05_source_of_truth_connectors.md): connector model and first connector priorities.
- [06_fsv_verification_engine.md](06_fsv_verification_engine.md): full-state verification protocol and verifier registry.
- [07_replay_forensics_and_incidents.md](07_replay_forensics_and_incidents.md): replay, audit packets, and incident workflow.
- [08_training_eval_corpus.md](08_training_eval_corpus.md): verified episode export for training and eval data.
- [09_security_governance_compliance.md](09_security_governance_compliance.md): security, compliance, NIST, OWASP, MITRE mapping.
- [10_api_cli_mcp_surface.md](10_api_cli_mcp_surface.md): CLI, API, MCP, and SDK surface.
- [11_storage_retention_privacy.md](11_storage_retention_privacy.md): storage, retention, encryption, privacy, tenancy.
- [12_scoring_and_risk_model.md](12_scoring_and_risk_model.md): verifier status, risk scoring, trust levels.
- [13_mvp_implementation_phases.md](13_mvp_implementation_phases.md): sequenced build plan.
- [14_tests_acceptance_and_fsv.md](14_tests_acceptance_and_fsv.md): acceptance tests and manual full-state verification.
- [15_gtm_pricing_and_packaging.md](15_gtm_pricing_and_packaging.md): go-to-market, packaging, sales motion.
- [16_risks_open_questions.md](16_risks_open_questions.md): product, technical, and business risks.
- [17_research_basis.md](17_research_basis.md): web research references and design implications.
- [18_first_wedge_demo.md](18_first_wedge_demo.md): first proof-of-value workflow.

## Build Doctrine

Every mutating action must produce durable evidence:

1. Define the source of truth before execution.
2. Capture the pre-state independently.
3. Execute the agent or tool action.
4. Read the source of truth through a separate read path.
5. Compare expected persisted delta to actual readback.
6. Emit a typed verifier result with error code and remediation.
7. Persist the evidence packet.
8. Export accepted and rejected episodes into eval/training data.

The system must fail closed. Missing connectors, missing readback permissions,
schema drift, stale evidence, or unverified claims are product-visible failures,
not soft warnings.

## First Product Wedge

The first commercially useful wedge is DevOps/SRE/SecOps agent governance:
an AI agent proposes or performs code, CI, dependency, infrastructure, or ticket
actions; BlackBoxFix records the run and verifies physical outcomes in Git,
GitHub, CI, scanner output, Terraform/Kubernetes state, and ticketing systems.

This wedge is attractive because the sources of truth are inspectable, the
workflow has budget, and the cost of unverified agent behavior is concrete.

