# 04 MCP Tool Workflows

This file describes concrete MCP workflows. JSON snippets show intent, not exact request IDs.

## Workflow A: capability discovery

Call:

```json
{
  "name": "get_capability_matrix",
  "arguments": {
    "includeToolSchemas": true
  }
}
```

Expected proof:

- tool count includes `store_memories` and learner tools
- content count = 14
- learner-state count = 7
- total slots = 21
- source-of-truth CF list includes content and learner CFs

## Workflow B: register learner

Call:

```json
{
  "name": "register_learner",
  "arguments": {
    "handle": "shakespeare-pilot-learner",
    "consentState": "consented-local-first",
    "modalitiesEnabled": ["affect_text", "self_report"]
  }
}
```

Verify:

1. Read `count_learner_state`.
2. Read `get_learner_state` with returned learner ID.
3. Inspect `CF_LEARNER_PROFILE` count with `ldb`.
4. Confirm no learner fingerprint exists yet.

## Workflow C: record learner baseline observation

Call:

```json
{
  "name": "record_session_observation",
  "arguments": {
    "learnerId": "uuid",
    "sessionTs": 1000,
    "modality": "affect_text",
    "rawText": "Focused and calm.",
    "consentState": "consented-local-first",
    "preprocessingVersion": "affect-text-v1",
    "embedderVersion": "E17-affect-text-v1",
    "thresholdVersion": "baseline-v1",
    "components": {
      "plasticityWindow": 0.75,
      "hrvCoherence": 0.65,
      "valence": 0.25,
      "arousal": 0.20,
      "stressFloor": 0.15,
      "kSleep": 1.0
    },
    "embeddings": [
      {
        "modality": "affect_text",
        "vector": [0.75, 0.65, 0.25, 0.20, 0.15],
        "scalar": 0.75
      }
    ]
  }
}
```

Verify:

- `CF_FINGERPRINTS_LEARNER` +1
- `CF_LEARNER_STATE_HISTORY` +1
- `CF_LEARNER_AUDIT` +1
- `get_learner_state` returns exact session timestamp and components

## Workflow D: resolve retrieval policy

Call:

```json
{
  "name": "resolve_learner_retrieval_policy",
  "arguments": {
    "learnerId": "uuid",
    "sessionTs": 1000,
    "baseWeightProfile": "semantic_search",
    "includeWeights": true
  }
}
```

Expected behavior:

- optimal/calm state keeps or lightly adapts semantic profile
- high stress routes to `affect_repair`
- negative valence can route to `affect_priming`
- high stress with emotional load can route to `affect_neutral`

Verify:

- no content or learner CF mutation
- returned selected profile matches deterministic policy
- missing learner/session errors, no fallback

## Workflow E: state-conditioned Shakespeare search

Call:

```json
{
  "name": "search_graph",
  "arguments": {
    "query": "Explain ambition in Macbeth using the text.",
    "topK": 5,
    "includeContent": true,
    "includeProvenance": true,
    "stateConditionedProfile": true,
    "learnerId": "uuid",
    "learnerSessionTs": 1000,
    "baseWeightProfile": "semantic_search"
  }
}
```

Verify:

- response contains state-conditioned policy evidence
- returned memory IDs exist in `CF_FINGERPRINTS`
- returned content hashes match `CF_CONTENT`
- no learner CF mutation

## Workflow F: record outcome

After an answer, persist a learning event.

Call:

```json
{
  "name": "record_learning_event",
  "arguments": {
    "memoryIds": ["retrieved-memory-uuid"],
    "sessionId": "shakespeare-tutor-session-1",
    "responseId": "assistant-response-uuid-or-hash",
    "taskId": "macbeth-ambition",
    "query": "Explain ambition in Macbeth using the text.",
    "retrievedContext": "selected passage text",
    "assistantResponse": "answer shown to learner",
    "before": {
      "topicProfile": [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
      "crossCorrelations": [91 numbers]
    },
    "after": {
      "topicProfile": [14 numbers],
      "crossCorrelations": [91 numbers]
    },
    "outcome": {
      "label": "useful",
      "utilityDelta": 0.25,
      "correctionRequired": false,
      "reuseObserved": true
    }
  }
}
```

Verify:

- `CF_LEARNING_EVENTS` +1
- `get_learning_event` returns same event
- event memory IDs are real content IDs
- text fields are preserved when `includeText=true`

## Workflow F2: inspect learning signal embedders

Call:

```json
{
  "name": "list_learning_signal_embedders",
  "arguments": {}
}
```

Verify:

- exactly five embedders are listed
- IDs are `delta_e`, `surprise`, `coherence`, `consolidation`, `transfer`
- dimensions are 14 for `delta_e` and 3 for the other four signals

## Workflow F3: compute inline learning signals

Call:

```json
{
  "name": "compute_learning_signals",
  "arguments": {
    "before": {
      "topicProfile": [14 numbers],
      "crossCorrelations": [91 numbers]
    },
    "after": {
      "topicProfile": [14 numbers],
      "crossCorrelations": [91 numbers]
    },
    "outcome": {
      "label": "useful",
      "utilityDelta": 0.25
    }
  }
}
```

Verify:

- response includes five signal vectors
- response includes computed feature values when `includeFeatures=true`
- no RocksDB CF count changes
- invalid dimensions and unknown `signalIds` error without mutation

## Workflow F4: embed persisted learning event signals

Call:

```json
{
  "name": "embed_learning_event_signals",
  "arguments": {
    "eventId": "learning-event-uuid",
    "includePersisted": true
  }
}
```

Verify:

- source of truth is `CF_LEARNING_EVENTS`
- before/after `learning_events` count is unchanged
- `persisted_all_match=true`
- missing event IDs error without mutation

## Workflow G: compute and persist L

Call:

```json
{
  "name": "compute_L",
  "arguments": {
    "learnerId": "uuid",
    "sessionTs": 1001,
    "traceId": "uuid",
    "predictedText": "learner expected answer",
    "actualText": "correct answer or response",
    "recentScores": [0.4, 0.5, 0.7],
    "hrvCoherence": 0.65,
    "panelAgreement": 0.70,
    "contradiction": 0.10,
    "components": {
      "plasticityWindow": 0.75,
      "hrvCoherence": 0.65,
      "valence": 0.25,
      "arousal": 0.20,
      "stressFloor": 0.15,
      "kSleep": 1.0
    },
    "persist": true,
    "retrievalCorrect": true
  }
}
```

Verify:

- `CF_LEARNER_DELTA_LOG` +1
- `CF_LEARNER_M_PER_TRACE` created or updated
- `get_learner_M` returns the trace
- `next_review_for_trace` returns a timestamp or explicit null policy

## Workflow H: export learner training data

Call:

```json
{
  "name": "export_learner_training_dataset",
  "arguments": {
    "task": "reward_model",
    "maxRows": 10000,
    "clearExisting": false
  }
}
```

Verify:

- `CF_LEARNER_TRAINING_DATASETS` +1
- `get_learner_training_dataset` returns shape and preview rows
- row count equals real eligible event/state rows
- revoked or non-training-consented rows are excluded with explicit counts
