# 10 — `/contact` Page Specification

**Route:** `/contact`
**File:** `src/app/contact/page.tsx`
**Content:** `src/content/routes/contact.ts` (REPOSITION existing file)
**Audience:** Anyone wanting to talk about the work
**Single thing it must communicate:** "Email Chris. Here is exactly how. There is no other contact channel."

---

## 1. Page meta

| Field | Value |
|---|---|
| `title` | `Contact — Teleox.ai` |
| `description` | `Direct email contact for research conversations about Teleox: chrisroyseai@gmail.com.` |
| `og:title` | `Contact Teleox` |
| `og:description` | `Direct email is the only contact channel. No forms, no scheduling tools.` |
| `og:image` | `/og-image.png` |
| `canonical` | `https://teleox.ai/contact` |

---

## 2. Page structure

```
[ Header ]
[ Hero (eyebrow + h1 + subhead) ]
[ Section: How to reach me ]
[ Section: What gets a faster reply ]
[ Section: What does not get a reply ]
[ Footer ]
```

Container width: `reading` (660px max). No CTA pair at the bottom — the entire page is the CTA.

---

## 3. Content (verbatim copy)

### 3.1 Eyebrow

```
Contact
```

### 3.2 H1

```
Reach me directly
```

### 3.3 Subhead

```
The contact channel is a single email address. No contact forms, no Calendly, no Slack invites. If the work is interesting, write.
```

### 3.4 Section: How to reach me

**Heading:** `How to reach me`

**Body:**

```
chrisroyseai@gmail.com

Plain-text emails preferred. Subject lines that name the topic ("ME-JEPA preview
access" / "Teleox research" / "ClipCannon question") get triaged faster than
generic openings.
```

(The email address renders as a `mailto:` link with prominent treatment — see §5 below.)

### 3.5 Section: What gets a faster reply

**Heading:** `What gets a faster reply`

**Body (bulleted list — render as `<ul>`):**

```
- Specific questions about the technical content of the current paper
- Requests for private-preview access from researchers at frontier AI labs or academic groups
- Pointers to related work I should be reading
- Pointers to errors, omissions, or weaknesses in the current research
- Forwarding from someone whose work the paper engages with
```

### 3.6 Section: What does not get a reply

**Heading:** `What does not get a reply`

**Body (bulleted list):**

```
- Sales pitches, marketing solicitations, or "AI partnership opportunities"
- Generic "would love to chat" emails without a specific question
- Investment pitches (the operation is not raising)
- Speaker-bureau / conference-booking inquiries
- Requests to license the runtime for commercial use without a clear reason
- AI-generated cold outreach
```

---

## 4. The full `contact.ts` RouteCopy file

Replace `src/content/routes/contact.ts` with:

```ts
import type { RouteCopy } from "@/types/route-copy";

export const contactCopy: RouteCopy = {
  path: "/contact",
  purpose:
    "Provide a single direct-email contact channel for research conversations, and set expectations about what gets a faster reply.",
  audience: "primary",
  playerClasses: ["lab", "researcher", "public"],
  eyebrow: "Contact",

  primaryCta: {
    label: "Email chrisroyseai@gmail.com",
    href: "mailto:chrisroyseai@gmail.com?subject=Teleox",
  },
  secondaryCta: undefined,

  signals: ["S1", "S3"],
  scopeGuards: [],

  heroHeadline: "Reach me directly",
  heroHeadlineLitWord: "directly",
  heroSubhead:
    "The contact channel is a single email address. No contact forms, no Calendly, no Slack invites. If the work is interesting, write.",
  heroAlt: "Contact Teleox.ai — direct email only.",

  bodyBlocks: [
    {
      heading: "How to reach me",
      paragraphs: [
        "chrisroyseai@gmail.com",
        "Plain-text emails preferred. Subject lines that name the topic (\"ME-JEPA preview access\" / \"Teleox research\" / \"ClipCannon question\") get triaged faster than generic openings.",
      ],
    },
    {
      heading: "What gets a faster reply",
      paragraphs: [
        "Specific questions about the technical content of the current paper. Requests for private-preview access from researchers at frontier AI labs or academic groups. Pointers to related work I should be reading. Pointers to errors, omissions, or weaknesses in the current research. Forwarding from someone whose work the paper engages with.",
      ],
    },
    {
      heading: "What does not get a reply",
      paragraphs: [
        "Sales pitches, marketing solicitations, or \"AI partnership opportunities.\" Generic \"would love to chat\" emails without a specific question. Investment pitches (the operation is not raising). Speaker-bureau / conference-booking inquiries. Requests to license the runtime for commercial use without a clear reason. AI-generated cold outreach.",
      ],
    },
  ],

  mustNotSay: [
    "guarantees AGI",
    "solves the data wall",
    "model collapse is over",
    "Send data problem",
    "Talk to sales",
    "Schedule a call",
    "Book a demo",
  ],
};

export default contactCopy;
```

---

## 5. The full `page.tsx` file

Replace `src/app/contact/page.tsx` with:

```tsx
import type { Metadata } from "next";
import { contactCopy } from "@/content/routes/contact";
import { generateRouteMetadata } from "@/lib/metadata";
import { PageContainer } from "@/components/layout/PageContainer";
import { ResearchHero } from "@/components/hero/ResearchHero";
import { Section } from "@/components/content-blocks/Section";
import { Mail } from "lucide-react";

export const metadata: Metadata = generateRouteMetadata(contactCopy, {
  title: "Contact — Teleox.ai",
  description:
    "Direct email contact for research conversations about Teleox: chrisroyseai@gmail.com.",
});

export default function ContactPage() {
  return (
    <PageContainer width="reading">
      <ResearchHero
        eyebrow={contactCopy.eyebrow}
        headline={contactCopy.heroHeadline}
        subhead={contactCopy.heroSubhead}
      />

      <Section heading={contactCopy.bodyBlocks[0].heading}>
        <p className="text-[20px] md:text-[24px] font-body">
          <a
            href="mailto:chrisroyseai@gmail.com?subject=Teleox"
            className="
              inline-flex items-center
              text-cream
              underline decoration-amber decoration-2 underline-offset-4
              hover:text-honey hover:decoration-honey
              transition-colors
            "
          >
            <Mail size={18} className="mr-2" />
            chrisroyseai@gmail.com
          </a>
        </p>
        <p>
          Plain-text emails preferred. Subject lines that name the topic
          ("ME-JEPA preview access" / "Teleox research" / "ClipCannon
          question") get triaged faster than generic openings.
        </p>
      </Section>

      <Section heading={contactCopy.bodyBlocks[1].heading}>
        <ul className="list-disc list-outside pl-6 [&>li]:mb-2">
          <li>Specific questions about the technical content of the current paper</li>
          <li>Requests for private-preview access from researchers at frontier AI labs or academic groups</li>
          <li>Pointers to related work I should be reading</li>
          <li>Pointers to errors, omissions, or weaknesses in the current research</li>
          <li>Forwarding from someone whose work the paper engages with</li>
        </ul>
      </Section>

      <Section heading={contactCopy.bodyBlocks[2].heading}>
        <ul className="list-disc list-outside pl-6 [&>li]:mb-2 text-cream/72">
          <li>Sales pitches, marketing solicitations, or "AI partnership opportunities"</li>
          <li>Generic "would love to chat" emails without a specific question</li>
          <li>Investment pitches (the operation is not raising)</li>
          <li>Speaker-bureau / conference-booking inquiries</li>
          <li>Requests to license the runtime for commercial use without a clear reason</li>
          <li>AI-generated cold outreach</li>
        </ul>
      </Section>
    </PageContainer>
  );
}
```

Note: `paragraphs[0]` of the first body block is the email address itself. `page.tsx` overrides the rendering for this single paragraph to make it a prominent linkified mailto.

---

## 6. Why no contact form

A research operation with one principal does not need a contact form. Forms add:
- A spam vector (need anti-spam infra)
- A backend dependency (form submission handling)
- An interaction friction (people fill them out half-heartedly)
- A signal that "this is a corporate website" (against positioning)

A direct `mailto:` link signals: "this is a person; reach them directly."

---

## 7. Test plan

- [ ] `https://teleox.ai/contact` returns 200
- [ ] Email link renders as a prominent linkified mailto with amber underline
- [ ] Clicking the email link opens the system mail client with subject "Teleox"
- [ ] Three sections render with bullet lists styled correctly
- [ ] No console errors
- [ ] Lighthouse Performance ≥ 95
- [ ] `npm run scan:all` passes
- [ ] No "Send data problem" or "Schedule a call" anywhere on page
