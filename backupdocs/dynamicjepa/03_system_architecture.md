# System Architecture

DynamicJEPA has five planes:

1. Data plane
2. Instrument plane
3. Dynamics plane
4. Planning and skill plane
5. Verification and audit plane

## End-To-End Flow

```text
raw domain source
  -> domain adapter
  -> universal event/action/state rows
  -> consent/safety/legal gate if applicable
  -> frozen instrument panel
  -> latent panel record
  -> cross-instrument binding record
  -> trajectory/state graph
  -> DynamicJEPA predictor
  -> planner/read-out/skill proposal
  -> verifier guard
  -> audited output/action
```

## Data Plane

The data plane normalizes every domain into typed records:

- observation events,
- action events,
- outcome events,
- entity states,
- relations,
- goals,
- constraints,
- source metadata.

The data plane never silently repairs malformed data. It validates or errors.

## Instrument Plane

The instrument plane manages frozen encoders, classifiers, simulators, feature extractors, and domain-specific probes.

An instrument can be:

- text embedder,
- image/video encoder,
- audio encoder,
- graph embedder,
- code embedder,
- time-series encoder,
- tabular featurizer,
- physics/simulation probe,
- domain classifier,
- safety detector.

Every instrument has a version, dimension contract, allowed domains, preprocessing hash, and validation harness.

## Dynamics Plane

The dynamics plane trains predictors over latent state transitions:

```text
P_psi(z_context, action, goal, time_delta, domain_id) -> z_target_distribution
```

It supports:

- forward prediction,
- masked state prediction,
- action-conditioned prediction,
- counterfactual branch prediction,
- surprise/anomaly scoring,
- uncertainty estimates.

## Planning And Skill Plane

The planner uses predicted latent futures to choose actions, propose workflows, or select tools.

The skill library stores verified action programs:

- code procedures,
- tool workflows,
- robot policies,
- analysis playbooks,
- domain recipes,
- recovery procedures.

Skills are not trusted because they exist. They are trusted only after FSV and outcome logs prove they work.

## Verification And Audit Plane

Every prediction, plan, skill execution, model update, and state mutation writes audit evidence.

The verifier checks:

- source record exists,
- instrument versions match,
- state transition is valid,
- prediction is within accepted constellation,
- prediction avoids anti-poles,
- action respects constraints,
- output was physically persisted and read back.

If verification fails, the operation fails.

