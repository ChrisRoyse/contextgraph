# 02 Data Model And Sources

## Boundary: content versus learner

Content records and learner records must remain separate.

| Record type | Slots | Primary storage |
| --- | --- | --- |
| content memory | E1-E14 | `CF_FINGERPRINTS`, `CF_CONTENT`, `CF_SOURCE_METADATA` |
| learner observation | E15-E21 + scalar components | `CF_FINGERPRINTS_LEARNER`, `CF_LEARNER_STATE_HISTORY` |
| learning outcome | content + learner + outcome | `CF_LEARNING_EVENTS` |
| learner training dataset | row-major tensor export | `CF_LEARNER_TRAINING_DATASETS` |

Never mutate `TeleologicalFingerprint` to include E15-E21. Use `LearnerFingerprint` and `LearnerStateVector`.

## E15-E21 mapping

| Slot | Modality | Minimum viable source | Full source |
| --- | --- | --- | --- |
| E15 | affect speech | recorded learner explanation audio | Wav2Vec2/audeering affect model |
| E16 | affect face | webcam face frames | OpenFace/AU/gaze/emotion head |
| E17 | affect text | learner self-report or typed answer | text affect regressor |
| E18 | PPG/HRV | wearable pulse stream | PaPaGei/HRV feature pipeline |
| E19 | EDA/stress | EDA wrist/chest sensor | WESAD-style stress head |
| E20 | EEG plasticity | EEG recording | LaBraM-style EEG encoder |
| E21 | EEG robust | artifact-heavy EEG recording | EEGPT/artifact-robust encoder |

## Minimum real-data ladder

Do not wait for all seven learner channels. Implement and verify in this order:

1. E17 affect text and self-report.
2. E15 learner speech from microphone recordings.
3. E19 EDA or E18 PPG from a cheap wearable/sensor.
4. E16 face if webcam consent and local processing are ready.
5. E20/E21 EEG only after the lower-cost channels prove value.

Each step must be separately FSV'd.

## Observation payload

`record_session_observation` already accepts:

- `learnerId`
- `sessionTs`
- `modality`
- `rawText`
- `consentState`
- `preprocessingVersion`
- `embedderVersion`
- `thresholdVersion`
- `stateVector`
- `components`
- `embeddings`
- `signals`

For initial real operation, require:

```json
{
  "learnerId": "uuid",
  "sessionTs": 1234567890,
  "modality": "affect_text",
  "rawText": "I feel focused but slightly frustrated.",
  "consentState": "consented-local-first",
  "preprocessingVersion": "affect-text-v1",
  "embedderVersion": "E17-text-affect-v1",
  "thresholdVersion": "learner-baseline-v1",
  "components": {
    "plasticityWindow": 0.72,
    "hrvCoherence": 0.60,
    "valence": 0.15,
    "arousal": 0.35,
    "stressFloor": 0.25,
    "kSleep": 1.0
  },
  "embeddings": [
    {
      "modality": "affect_text",
      "vector": [0.1, 0.2, 0.3],
      "scalar": 0.72
    }
  ]
}
```

The vector can be a real model vector or a real extracted feature vector. It must not be arbitrary mock data in FSV.

## Calibration data

A learner must have a baseline before state-conditioned retrieval is trusted.

Baseline collection:

- at least 5 sessions for initial development
- at least 10 sessions for user-facing claims
- include self-report and one physiological channel when available
- store each observation independently
- compile a learner constellation after baseline rows exist

Baseline output:

- `CF_FINGERPRINTS_LEARNER`: one fingerprint per session
- `CF_LEARNER_STATE_HISTORY`: state vector per session
- `CF_LEARNER_CONSTELLATIONS`: regulated baseline
- `CF_LEARNER_AUDIT`: provenance rows

## Consent and deletion model

Required consent flags:

| State | Meaning |
| --- | --- |
| `consented-local-first` | local processing and storage allowed |
| `consented-training` | rows may be exported for model training |
| `revoked` | no new observations, retrieval must not use stored learner state |

Deletion rules:

- raw signal files should be deleted on revocation unless explicitly retained.
- derived learner fingerprints should be hard-deletable by learner ID.
- training datasets containing revoked learner rows must be invalidated.
- content-side Shakespeare records are not learner data and are unaffected.

## Raw-source hashes

Every observation must preserve:

- raw SHA-256
- preprocessing version
- embedder version
- threshold version
- parent observation IDs when derived
- created timestamp

FSV must verify the stored values by reading learner CFs, not by trusting MCP responses.

