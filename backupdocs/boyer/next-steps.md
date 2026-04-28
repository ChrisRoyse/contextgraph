---
title: Next steps — review + proprietary prompt scope
subject: Unblock the email thread with Alan
---

# Next steps

## Immediate (today / this week)

1. **Read the book end-to-end** with `reading-notes-template.md` open. 90 min
   at most — it's a trade book, not a monograph.
2. **Post the Amazon review** using `review-draft.md` once the notes are
   filled in. Include the relationship disclosure since Chris is quoted.
3. **Reply to Alan's email thread** with one message that does two things:
   - Confirms the review is up and links it
   - Resolves the prompt ambiguity: *how is the prompt getting used?*
     API call from which surface? Who pays? What does the output look like
     that the teacher/school actually receives? Until that's nailed,
     the prompt itself can't be finalized.

## Prompt work (the real deliverable Alan is waiting on)

Alan's email says the learning theory he sent is "the secret sauce" that the
AI will use to develop optimized courses. Before writing the prompt, get
clarity on:

- **Surface**: Web app? Claude API called from a backend? Standalone CLI? Chat
  UI embedded in a school LMS? This decides whether the prompt is a system
  prompt, a developer prompt, or a user-turn template.
- **Inputs**: What does the teacher supply? (Topic? Learning outcomes? Student
  level? Duration? Existing syllabus?)
- **Outputs**: A full course shell? A single module? Assessments? Slides?
  Discussion prompts? Rubrics? The output schema defines prompt structure.
- **Proprietary layer**: Which parts of Alan's theory stay server-side
  (system prompt, never shown to the user) and which parts are reflected in
  the output as Boyer-branded scaffolding?
- **Pricing primitive**: Per course? Per module? Per seat? Because that
  shapes how much generation happens per API call and whether caching /
  structured output / batch is appropriate.
- **Model**: Claude Opus 4.7 for quality vs. Claude Sonnet 4.6 for cost.
  Opus for course architecture, Sonnet for expansion/drafting is a common
  split.

Once those are answered, the prompt writes itself — it's a system-prompt-plus-
user-template against a structured output schema, with prompt caching on the
Boyer theory block (it's stable across calls, so it should be cached).

## Longer arc

- If the product ships, Boyer's book becomes the marketing surface for it.
  A strong, specific Amazon review from Chris now seeds that funnel.
- Consider a short companion post (LinkedIn) from Chris referencing the
  book and the collaboration — a second, owned surface Alan can amplify.
