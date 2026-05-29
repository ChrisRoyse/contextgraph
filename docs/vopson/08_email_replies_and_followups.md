# 08 — Draft Emails & Follow-up Cadence

**Copy-paste-ready drafts. Adjust to your voice, but keep the rigor and the honesty.** Tone: warm, concise, numbers-first, no hype. He writes short, friendly emails — match that.

The thread so far ended with **his** ask (Title/Abstract/Bio) and your "I'll get right on it." So **Email 1 is the one he's waiting for.**

---

## EMAIL 1 — The talk package (send now; this is overdue in his mind)

**Subject:** Re: A binary reality predictor for AI-generated code — building on infodynamics

> Hi Melvin,
>
> Here's the IPI Talk package. I've leaned it toward the science and your second law of infodynamics, since that's where I think the real collaboration is.
>
> **Title:** The Second Law of Infodynamics in a Learning Machine: Watching Information Entropy Fall as an AI Verifier Trains Itself Against Reality
>
> **Abstract:** [paste the abstract from `01`]
>
> **Bio:** [paste the bio from `01`]
>
> **Picture:** attached.
>
> A few logistics whenever you have them: which weekend works, Teams or Zoom, how long you'd like the talk vs. the member Q&A, and whether you want the slides in advance. Happy for it to be a collaboration post on both channels as you suggested.
>
> On your P.S. — you're right that the earlier write-up had no data in it. I'll send a short data note separately so the talk isn't the first time you see real numbers. Short version: the verifier is at 0.48 correlation against a 0.95 gate (honest gap, not a flattering transform), but the trainer already logs the per-step reduction in the description length of the system's state — information entropy falling over training, in bits. That's the thread I think connects to your work, and I'd like to measure it properly with you.
>
> Thanks again for the invitation — genuinely looking forward to it.
>
> Best,
> Chris

*Notes:* attach the headshot. Don't attach the whole data package yet — tease it, send it as Email 2 so it lands as its own thing. Leading the P.S. response with "you're right" disarms and signals you take his feedback seriously.

---

## EMAIL 2 — The data note (send 1–2 days later, or same day if he replies fast)

**Subject:** The data, as promised — ME-JEPA numbers + an infodynamics experiment

> Hi Melvin,
>
> As promised, the real numbers and the experiment I think we should run.
>
> **What's measured today**
> - Verifier–oracle correlation (held-out): 0.48 Pearson; gate is 0.95 stable over 4 windows. Calibration is already good (ECE 0.024). I'm less than halfway and won't pretend otherwise.
> - The trainer logs, per step, the conditional description length (in bits) of the system's frozen target state, and the per-step reduction in it — i.e. information entropy decreasing over training, in the system's own units. The instrument runs today; the full real-training trajectory is what I'd like to measure with you.
> - A separate system of mine (voice cloning, same frozen-multichannel method) scores 0.778 on the standard SeedTTS benchmark — near the 0.79 SOTA and above the 0.73 human reference — with the honest caveat that an *independent* speaker encoder still finds the clone distinguishable (EER 0.25). I report both, because encoder-matched numbers flatter and cross-encoder numbers tell the truth.
>
> **The experiment (your law, in a learning machine)**
> As the verifier trains, log four curves: per-channel information entropy, the compression-progress bits, a channel-symmetry score, and oracle agreement. The clean confirmation of your second law of infodynamics is: entropy falls, symmetry rises, and both track accuracy. I've also written down the falsification condition in advance — if accuracy rises while entropy stays flat, your law doesn't govern this kind of learner, which is itself worth reporting.
>
> I've also put together a longer technical brief that explains the system without the software jargon — happy to send if useful, since you mentioned wanting to understand the engineering first-hand.
>
> Best,
> Chris

*Notes:* this email is the whole game. It answers "no data," shows you understand encoder-matched vs cross-encoder evaluation, and pre-registers a falsification condition — three things that make a metrologist trust you.

---

## EMAIL 3 — The joint-article proposal (send AFTER the talk is scheduled or delivered)

**Subject:** A one-pager on the joint article you suggested

> Hi Melvin,
>
> You floated a joint article if the infodynamics behaviour showed up in the data. Here's a one-page proposal with pre-registered hypotheses and a falsification condition [attach/paste `04`]. If the framing works for you, I'll run the experiment and send the raw logs and certificates for independent re-analysis. IPI Letters seems the natural first home, but I'll defer to you on venue and scope — happy to shape it to whatever you'd want to co-author.
>
> Best,
> Chris

---

## EMAIL 4 — Gentle nudge (only if no reply after ~5–7 days)

> Hi Melvin — just bumping this in case it slipped. No rush at all; whenever the weekend timing suits you works for me. Happy to send slides ahead if that helps you plan the session.

---

## Follow-up cadence & rules
- **Reply speed:** answer his emails within a day. He's busy and runs an institute; responsiveness reads as seriousness.
- **One ask per email.** Email 1 = talk package. Email 2 = data. Email 3 = paper. Don't stack.
- **Always attach a concrete artifact**, not just prose — the abstract, the data table, the one-pager. He's a scientist; artifacts beat enthusiasm.
- **Mirror his register:** short, friendly, numbered, signed "Melvin." You sign "Chris."
- **If he goes quiet after the talk:** the durable asset is the recorded talk + the data note. Don't chase the paper hard; offer to run the cheap probes (#734 MI, #723 oracle self-consistency) and send him the result as a no-strings data point. Data re-opens doors.
- **If he says yes to the paper:** your immediate next move is to actually run the per-channel entropy measurement (issue #511 / #734 territory) — which also moves your own ship-gate work. Don't let the paper stall on un-run measurements; that's the one thing that would burn the goodwill.

## Things to confirm/track
- [ ] Headshot ready and attached to Email 1.
- [ ] Weekend date, platform, talk length, slides-in-advance — confirmed.
- [ ] Cross-post agreement for the recording — confirmed.
- [ ] Decide your honest answer to "can I participate in Leapable.ai?" before any call (see `07 Q12`).
- [ ] Before the talk: rehearse `07`; build the deck from `02`; never show the 0.79 number.
