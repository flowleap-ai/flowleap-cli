# Office action analysis: the report layout

Write the report body in this order, with these headings and field labels. Keep the labels as they are: a reader (or a later work page) finds each objection and its fields by them. Use one `### Objection` block for each objection, also for a formal objection.

```markdown
## 1. Action

| Field | Value | Source |
|---|---|---|
| Application | US 17/000,001 | get_us_application |
| Office and action | USPTO, non-final rejection (CTNF) | get_application_documents |
| Action date | 2026-01-15 | OA page 1 |
| Response deadline (as stated) | 3 months, extendable | OA page 1; the attorney confirms |
| Claims examined | 1-20, listing of 2025-06-02 | CLM document of 2025-06-02 |
| Earliest priority date | 2024-03-01 | get_us_application |
| Cited documents | D1 US1111111A1; D2 EP2222222A1 | OA page 3; form 892 |
| Assumptions | Started from a routine: the newest CTNF on or after 2026-01-15 | routine instruction |

## 2. Objections

### Objection 1: Claims 1-7 lack novelty over D1

- **Ground:** 35 U.S.C. 102(a)(1)
- **Legal basis:** "…" (MPEP 2131, [R-01.2024], https://www.uspto.gov/web/offices/pac/mpep/s2131.html)
- **Claims hit:** 1, 2, 3, 4, 5, 6, 7
- **Examiner's reasoning:** "…" (OA page 4)
- **Source of the objection:** OA text | search report categories (the search opinion text was not read)
- **Cited documents:**

| Document | Role | Passage the examiner relied on | What the passage says | Source anchor |
|---|---|---|---|---|
| D1 US1111111A1 | applied alone | ¶[0059], Fig. 6 | "…" | [source anchor: …] |

- **Assessment:** one or two sentences, with the passage that decides it.
- **Options:** Argue (…); Amend (…, support ¶[00xx] "…"); Accept or cancel (…); Ask (…)
- **Sources:** the anchors and URLs used in this block

### Objection 2: …

## 3. Claim-by-claim comparison

One table for each independent claim that an objection hits:

| Element (verbatim) | Examiner says (OA page) | Cited passage says (quote, passage, anchor) | Assessment |
|---|---|---|---|
| 1a "a housing …" | D1 ¶[0059] (OA page 4) | "…" US1111111A1 ¶[0059] [source anchor: …] | disclosed |

Assessment is one of `disclosed`, `not disclosed`, `disputable`, `not read`. After the tables, write one line for each dependent claim that the OA names: its added feature, and whether a cited passage discloses it.

## 4. Response plan

| Objection | Option | For | Against | Open question for the attorney |
|---|---|---|---|---|

Then a short list of the facts that the attorney or the applicant must supply. No option is a decision: write "for the attorney's decision".

## 5. Sources and gaps

- Each tool call and page that the report cites, with what it gave.
- Each gap: what was not read (an OA text that no tool serves, a description outside the text coverage, a law page that did not load), what you tried, and what the attorney can supply.
```

The last line of the report is the closing sentence of the recipe (Step 7).
