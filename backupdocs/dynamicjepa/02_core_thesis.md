# Core Thesis

DynamicJEPA is a universal world-model protocol, not a claim that one static model will perfectly model all domains.

## Definition

DynamicJEPA is:

```text
An auditable, domain-portable, multi-instrument latent dynamics system that
predicts future state representations, action effects, and verifier distances
over typed event streams.
```

## What "Universal" Means

Universal means:

- any domain can register a domain pack,
- any modality can register an adapter,
- any frozen model can register as an instrument if it passes validation,
- any event stream can become trajectories if it satisfies the universal schema,
- any predictor/read-out head can be traced to physical source records.

Universal does not mean:

- one model has perfect knowledge,
- every domain uses the same embedders,
- legal/privacy constraints disappear,
- failed instruments are ignored,
- predictions are accepted without verification.

## What "Dynamic" Means

Dynamic means:

- new domains can be added without rewriting the core runtime,
- new instruments can be registered and versioned,
- new observations update memory and trajectory state,
- new skills can be created and verified,
- surprise triggers investigation and retraining,
- online adaptation is logged and reversible.

Dynamic does not mean uncontrolled live weight mutation. Parameter updates require dataset manifests, artifact hashes, evaluation, and approval.

## Why This Is Different From ME-JEPA

ME-JEPA:

```text
career/recruiting corpus -> N=19 panel -> trajectory predictor -> recruiting heads
```

DynamicJEPA:

```text
domain pack -> instrument panel -> typed world state -> dynamics predictor -> planner/heads
```

ME-JEPA can become the first domain pack. The generalized runtime should also support code, robotics/video, finance, health, science, documents, physical sensors, simulations, and enterprise process data.

## Core Abstraction

Everything becomes a transition:

```text
(state_t, action_t, context_t, goal_t) -> state_t+1
```

Where each state is a multi-instrument latent panel plus source-provenance and verifier metadata.

The system learns the transition in latent space and verifies outputs against constellations, anti-poles, invariants, and physical readback.

