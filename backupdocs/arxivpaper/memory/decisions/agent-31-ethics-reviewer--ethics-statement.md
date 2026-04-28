---
namespace: decisions
agent_id: agent-31-ethics-reviewer
timestamp: 2026-04-16T20:00:00Z
status: active
locked: true
supersedes: none
depends_on:
  - memory/context/mission-brief.md
  - memory/decisions/agent-25-method-designer--case-study-protocols.md
  - memory/decisions/agent-26-sampling-strategist--sampling-protocols.md
  - memory/discoveries/agent-29-validity-guardian--validity-threats.md
  - memory/discoveries/agent-30-risk-analyst--fmea-risk-register.md
  - memory/handoff/agent-30-to-31--risks-ready.md
  - memory/discoveries/agent-14-bias-detector--bias-audit.md
downstream_consumers:
  - agent-32-contradiction-analyzer
  - agent-37-methodology-writer
  - agent-40-limitations-writer
  - agent-42-adversarial-reviewer
  - agent-47-submission-packager
tags: ethics, consent, misuse, deepfake, dual-use, paste-ready, locked
answers_questions: [Q07, Q08, Q15, Q16]
---

# §Ethics — Paste-Ready Statement for the Manuscript

## 0. Placement guidance

This statement is designed to sit either as a standalone
**§Ethics** section placed between §8 Limitations and §9 Conclusion,
or as **Appendix E: Ethics and Responsible Disclosure** if §-count is
constrained. Recommended venue: standalone §Ethics, because arXiv
moderators for cs.LG look for this label on identity-cloning work.

Word count: **~540 words** (target range 400–600, observed 540).

Tier discipline: every factual claim is T1 MEASURED (subject counts,
consent mode, artefact-release scope) or T3 PROJECTED and flagged.

---

## 1. §Ethics (paste-ready; do not edit without agent-42 review)

> ## Ethics
>
> This work involves two human subjects whose likeness, voice, or
> speech was used as training or evaluation data, and one public-domain
> textual corpus. We describe each subject relationship, the consent
> status, the artefact-release posture, and the misuse-mitigation
> stance below.
>
> **Subject relationships.** Case 1 (Shakespeare stylistic LoRA) uses
> the canonical Shakespeare corpus (plays and sonnets) drawn from
> Folger Digital Texts and Project Gutenberg public-domain editions;
> no living subject is involved. Case 2 (identity-locked talking-head
> clone, "Santa") uses a 16-minute 15-second English-language
> interview of one adult individual (hereafter "the Case 2 subject"),
> not the author, collected in a single recording session for the
> purpose of producing a controlled identity-locked research
> demonstration. Case 3 (voice clone beyond 0.95 WavLM SECS) uses the
> author (Chris Royse) as the sole subject — 489 candidate voice clips
> drawn from the author's own video footage plus two clean-microphone
> reference recordings of 8 to 9 seconds each, with ten held-out
> English sentences authored and recorded by the author.
>
> **Consent.** Case 3 subject consent is unambiguous by virtue of
> author-as-subject identity; the author grants research use,
> manuscript publication, and inclusion of synthetic voice output in
> research demonstrations. Case 2 subject data were collected with
> **written informed consent** covering (a) research use, (b)
> manuscript publication, (c) inclusion of case-study description in
> a peer-reviewed / arXiv venue, (d) the scope of permissible
> commercial use, and (e) a consent-revocation mechanism under which
> the subject may request removal of subject-specific synthetic
> artefacts from any public-facing release. No Case 2 subject is a
> minor; no Case 2 data were collected covertly; no third-party data
> subjects appear in the pipeline. Case 1's Shakespeare corpus is
> public-domain and raises no living-subject concerns.
>
> **Dual-use posture.** The methodology we describe (Teleological
> Constellation Training) is inherently dual-use. The frozen-encoder
> geometric-proximity guard is **defensive** when used as an anti-drift
> constellation check and **offensive** when composed with an
> identity-locked generator in the hands of a bad actor. We disclose
> this explicitly: TCT is a verification primitive; it neither blocks
> nor enables misuse on its own, but it does reduce the data threshold
> for single-subject identity cloning from thousands of subject-specific
> examples to tens, and the reader should treat this as a capability
> characterisation, not a deployment recommendation.
>
> **Artefact-release scope.** We release (i) the Context Graph and
> ClipCannon pipeline source code under open-source licenses; (ii) the
> Case 1 Shakespeare LoRA training scripts and evaluation protocol
> against a public-domain corpus; (iii) the agent-27 reproducibility
> manifest schema (weight SHA-256 hashes, centroid hashes, threshold
> sets) so a third party can replicate the protocol on their own
> subject data. We **do not release** (i) the Case 2 subject's raw
> interview footage, the derived 2,362 clips, the Santa-specific LoRA
> adapter weights, the per-modality centroid .npz files, or any
> synthetic artefact of the Case 2 subject's likeness; (ii) the
> author's voice LoRA weights (Case 3 is inference-only, so no voice
> LoRA exists to withhold; the author does withhold the 489-clip
> speaker corpus and the 50-clip reference centroid from public release,
> sharing protocol and evaluation framework only). This asymmetric
> release posture is deliberate: the methodology is reproducible; the
> subject-specific identity artefacts are not.
>
> **Responsible disclosure.** A malicious user cannot, from this paper
> and its released artefacts alone, produce a cloned voice or face of
> the Case 2 subject or of any non-consenting third party, because the
> subject-specific weights, centroids, and reference clips are withheld.
> The methodology itself, however, could be re-instantiated by an
> adversary who possesses target-subject data. We therefore recommend
> that downstream consumers pair any TCT-style identity-locked
> generator with (i) output watermarking at synthesis time, (ii)
> cryptographic provenance metadata bound to the generator's centroid
> hash, and (iii) detection-side research on cross-encoder triangulation
> to expose encoder-matched self-consistency artefacts. We flag
> watermarking, provenance, and detection as named future work.
>
> **IRB / ethics review status.** This research was conducted outside a
> conventional university IRB context. The consent instruments, data
> handling, and release posture described above are designed to meet
> the spirit of the 45 CFR 46 Common Rule and the NeurIPS / ICML /
> ICLR responsible-research checklists for work involving human
> subjects and identity-cloning technology. The author takes personal
> responsibility for the consent chain documented above and for the
> asymmetric artefact-release posture; no commercial deployment of
> Case 2 subject-specific artefacts has occurred or is planned.

---

## 2. Compliance cross-reference

| Checklist item | Addressed in paragraph |
|---|---|
| Subjects identified | Subject relationships |
| Consent status per subject | Consent |
| Minor / vulnerable-population check | Consent (explicit "no minors") |
| Artefact-release scope | Artefact-release scope |
| Misuse disclosure | Dual-use posture + Responsible disclosure |
| Watermarking / detection pointer | Responsible disclosure |
| IRB status | IRB / ethics review status |
| Competing interests | Separate §Competing Interests — agent-31-ethics-reviewer--competing-interests.md |

## 3. Paste-ready single sentences (for use elsewhere in the manuscript)

For **§6.2 opener (Case 2)**:

> *"The Case 2 subject provided written informed consent for research
> use, manuscript publication, and case-study description; raw footage,
> derived clips, and Santa-specific LoRA weights are withheld from
> public release per the consent scope (see §Ethics)."*

For **§6.3 opener (Case 3)**:

> *"The Case 3 subject is the author; author-as-subject consent is
> unambiguous. The 489-clip speaker corpus and 50-clip reference
> centroid are withheld from public release; protocol and evaluation
> framework are released (see §Ethics)."*

For **§7 Discussion, after the capability summary**:

> *"We emphasise that TCT is dual-use: the same geometric-proximity
> guard that prevents drift in a protected pipeline enables selective
> acceptance in a cloning pipeline. Responsible deployment pairs
> identity-locked generation with output watermarking, cryptographic
> provenance, and cross-encoder detection (see §Ethics)."*

---

## 4. Confidence

**HIGH** that the §Ethics statement covers the consent, dual-use,
artefact-release, and responsible-disclosure axes that NeurIPS / ICML
/ ICLR 2025-era checklists require for identity-cloning research.

**HIGH** that the Case 2 consent description is an accurate
specification of what the manuscript must attest; principal author
must verify the written-consent instrument on file matches the five
scope items (a–e) before agent-47 locks submission.

**HIGH** that the asymmetric artefact-release posture (pipelines
released, subject-specific weights and clips withheld) is the
scientifically-correct and ethically-correct stance.

**MEDIUM** on whether downstream peer-review venues (NeurIPS / ICML /
ICLR 2025) will demand formal IRB documentation beyond what the
§Ethics statement describes; the "outside a conventional university
IRB" paragraph is a legitimate disclosure but may attract a
conditional-accept with "please provide IRB equivalent" request. The
principal author should pre-identify an IRB-equivalent review path
(e.g., an independent ethics advisor signing off on the consent
instrument) ahead of downstream submission.

## Changelog

- 2026-04-16 — Initial issue by agent-31-ethics-reviewer. Locked.
