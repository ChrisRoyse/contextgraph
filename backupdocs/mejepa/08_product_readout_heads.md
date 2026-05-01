# Product Readout Heads

The ME-JEPA predictor is the shared foundation. Product artifacts should be heads on top of the predictor, not independent foundations.

## First Heads

| Head | Output | Why first |
| --- | --- | --- |
| `H_comp` | compensation band distribution | fastest revenue, aggregate-friendly |
| `H_match` | candidate-role-company success score | core recruiting value |
| `H_anti` | anti-pole/bias proximity scores | compliance moat |
| `H_traj` | next-state trajectory prediction | validates world-model thesis |

## Second Heads

| Head | Output |
| --- | --- |
| `H_hiring_manager` | probability of interest/offer from a hiring manager |
| `H_reference` | attributed reference summary claims |
| `H_market` | aggregate role/industry/region forecast |
| `H_coach` | next-turn coaching suggestion |
| `H_vendor_rerank` | reranking score for external recruiting systems |

## Head Contract

Every head must declare:

- predictor artifact ID,
- training dataset ID,
- allowed input event types,
- allowed consent envelopes,
- output type,
- required guard constellation IDs,
- required anti-pole IDs,
- calibration report ID,
- evaluation report ID.

If a head lacks any declaration, it cannot be served.

## Output Policy

Named-person outputs are disabled unless explicit consent supports that product.

Default allowed product surface:

- aggregate compensation intelligence,
- anonymized pattern analysis,
- compliance/bias audit,
- model-internal reranking with no raw identity exposure,
- customer-owned data analysis under a DPA.

High-risk or delayed product surface:

- named candidate recommendations,
- named hiring-manager predictions,
- voice clones,
- frontier-lab corpus licensing,
- any output used to make employment decisions without required audit and notices.

## Guard Requirement

Every served head output must write and verify:

- input source event IDs,
- model artifact IDs,
- constellation IDs and versions,
- anti-pole IDs and versions,
- threshold values,
- cosine traces or relevant scores,
- accept/reject/fallback-is-not-allowed decision,
- operator/customer/request ID.

If guard logging fails, no product output is returned.

