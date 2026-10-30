# Office action analysis: the report layout

Write the report body in this order, with these headings and field labels. Keep the labels as they are: a reader (or a later work page) finds each objection and its fields by them. Use one `### Objection` block for each objection, also for a formal objection.

In the FlowLeap app, sections 0 to 3 are the fields of `officeAction`, and `content` holds only sections 4 and 5 and the closing sentence. Do not write the headings of sections 0 to 3 in `content`: the writer refuses that save.

- Section 0 is `summary`. Section 1 is `application`, `communication`, `issued` and `deadline` (each date as YYYY-MM-DD, or null), one `action` row (`field`, `value`, `source`) for each row of the table, and one `documents` entry (`label`, `text`) for each cited document.
- `communication` holds only the office and the action, for example "EPO, communication under Art. 94(3) EPC" or "EPO, European search report". The board shows it as the heading. Write no remark in it, also not in parentheses: put a remark, such as the SEARCH REPORT ONLY line or "no examination communication read", in the first line of `summary`, and an assumption in the Assumptions row of `action`.
- Each objection block is one entry of `officeAction.objections[]`: `id` ("1", or "1.2" for a part of objection 1; each objection has a different id), `title` (the line after "Objection 1:"), and one field for each label. Ground → `ground`, Legal basis → `legalBasis`, Claims hit → `claims` as whole numbers from 1 to 9999 (write a range as each number), Examiner's reasoning → `reasoning`, Source of the objection → `source`, Cited documents → `citations` (one `document`, `role`, `passage`, `says`, `anchor` entry for each row of the table), Assessment → `assessment`, Options → `options` (one `label`, `text` entry for each option), Reply must address → `replyMustAddress`, Confidence and gaps → `confidence`, Sources → `sources`.
- Section 3 is `comparison`, one entry for each claim table: `claim` (an independent claim that the `claims` of an objection lists), `heading`, `interpretation`, `rows` (`element`, `examiner`, `cited`, `assessment`) and `arrangement`. The lines on the dependent claims are `dependentClaims`.

```markdown
## 0. Summary

3 to 5 lines: the action type, final or not, the number of objections, the main problems, and the last date without extension. For an EP search report without its opinion, the first line is: "SEARCH REPORT ONLY: no examination communication or search opinion text was read. The 'objections' below are inferred from search categories, not raised by the EPO."

## 1. Action

| Field | Value | Source |
|---|---|---|
| Application | US 17/000,001 | get_us_application |
| Office and action | USPTO, non-final rejection (CTNF) | get_application_documents |
| Action type | Non-final; restriction withdrawn | OA page 1 |
| Action date | 2026-01-15 (mailing date as the OA states it) | OA page 1 |
| Response deadline (as stated) | 3 months SSP; extendable to 6 months under 1.136(a); attorney confirms | OA page 1 |
| Last date without extension / with maximum extension | 2026-04-15 / 2026-07-15 (computed by the agent; the attorney confirms) | computed |
| Later filings after this action | None found | get_application_documents |
| Claims examined | 1-20, listing of 2025-06-02 | CLM document of 2025-06-02 |
| Effective filing date per claim | 1-20: 2024-03-01 (priority document not read) | get_us_application |
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

- **Assessment:** one or two sentences, with the passage that decides it. For 102: the arrangement and any inherency. For 103 / Art. 56: the test block (problem-solution, or Graham and KSR).
- **Options:** Argue (…); Amend (…, basis ¶[00xx] "…"); Accept or cancel (…); Interview (…); Ask (…)
- **Reply must address:** each ground of this objection, the specific distinctions, and how an amendment avoids each reference (37 CFR 1.111(b), (c)).
- **Confidence and gaps:** what the assessment rests on, and what was not read.
- **Sources:** the anchors and URLs used in this block

### Objection 2: …

## 3. Claim-by-claim comparison

For each independent claim that an objection hits, first the interpretation of the disputed terms (term, reading, the definition quoted, why it is disputed). Then one table:

| Element (verbatim) | Examiner says (OA page) | Cited passage says (quote, passage, anchor) | Assessment |
|---|---|---|---|
| 1a "a housing …" | D1 ¶[0059] (OA page 4) | "…" US1111111A1 ¶[0059] [source anchor: …] | disclosed |

Assessment is one of `disclosed`, `not disclosed`, `disputable`, `not read`. Below the table, one line on the arrangement of the elements. After the tables, write one line for each dependent claim that the OA names: its added feature, and whether a cited passage discloses it.

## 4. Response plan

| Objection | Option | For | Against | Open question for the attorney |
|---|---|---|---|---|

Then a short list of the facts that the attorney or the applicant must supply, and the check items (law or periods that were not read in this run). No option is a decision: write "for the attorney's decision".

## 5. Sources and gaps

- Each tool call and page that the report cites, with what it gave.
- Family art: the documents and arguments of other family members that you found, or that you did not check.
- Each gap: what was not read (an OA text that no tool serves, a description outside the text coverage, a law page that did not load), what you tried, and what the attorney can supply.
```

The last line of the report is the closing sentence of the recipe (Step 7).
