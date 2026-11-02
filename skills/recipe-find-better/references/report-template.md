# Find Better report template

In the FlowLeap Patent AI app, `write_patent_results` with `template: "find-better-report"` builds the report and the working record from the structured fields (see Step 5 of SKILL.md); it uses the section order and column names below. Elsewhere, write the report as Markdown with this structure.

Write two files: `<publication>.find-better.md` (the report) and
`<publication>.find-better.working-record.md` (the working record).

## Report

```markdown
# Find Better: <publication>

Critical date: <earliest priority date, YYYY-MM-DD>. Family members walked: <n>. Offices: <list>.
Scope: post-grant discovery. This report states no invalidity conclusion.

## Result per independent claim

| Claim | Examiner's best art | Disclosed | Best art found | Disclosed | Result |
|---|---|---|---|---|---|
| <n> | <doc> (<office> <category>, ...) | <k> of <m> | <doc> (<publication date>) | <k> of <m> | better art found |
| <n> | <doc> (<office> <category>) | <k> of <m> | none | - | no better art found |

## Element rows: claim <n>

| Element | <examiner's best art> | <best art found> |
|---|---|---|
| <n>a <claim language> | "<quote>" (<anchor>) | not disclosed |

## Examiner Baseline

<the matrix: cited document by office, cell = category and cited claims, or
"applicant", or "-">

Searched claims not granted: <documents whose X/Y claims have no granted counterpart>

## Gaps

<each gaps[].message verbatim; CN/JP/KR members: "citations and abstracts only,
no element mapping">

## Tracks log

| Track | Seed | Tool and query | Count | Kept |
|---|---|---|---|---|
| 1 backward | <X/Y doc> hop 1 | get_bibliography <X/Y doc> | <count> | <kept> |
| 2 inventors | <name> | search_patents in="<name>" AND pd<<critical date> | <count> | <kept> |
| 3 classification | <cpc> | search_patents cpc=<cpc> AND ta=<term> AND pd<<critical date> | <count> | <kept> |
```

Result column: write "No better art found for claim N; the examiner's best art
remains <pub>" only when all three tracks ran a query and Track 1 has a hop 2.
Otherwise write "Search incomplete for claim N: <reason>". Name only the
examiner documents you scored; list the rest as "Examiner's best art named but
not scored". Show the basis of a document without X or Y, for example
"US3980041 (examiner-cited, no category)". Flag an examiner document with a P
category, or published after the critical date, as "published after the
critical date; not prior art for this claim unless the priority claim fails".
Exception: an earlier application published after the critical date may be
novelty art: at the EPO only a European application, or a Euro-PCT application
that entered the European phase (Rule 165 EPC), with an earlier effective date
(EPC Art. 54(3), 89, 153(5)); in the US a US patent, a published US application
or a WO application designating the US that names another inventor and has an
earlier effectively filed date (35 USC 102(a)(2), 102(d)), subject to the
102(b)(2) exceptions; for an application whose claims all have an effective
filing date before 2013-03-16 (AIA §3(n)(1)), pre-AIA 35 USC 102(e) applies
instead (a WO application counts only if it designates the US and was published
in English); a national earlier right is a check item (EPC Art. 139(2)). At the
EPO such art counts for novelty only; in the US it also counts for obviousness.
Flag such a document (often category E) as "earlier application published after
the critical date: check Art. 54(3) / 102(a)(2) or pre-AIA 102(e)".

## Rules

- "Disclosed n of m" counts only rows with a quoted, anchored passage from a tool result.
- Score the examiner's art and the found art on the same element rows.
- Every track is in the log, with every query, including the ones that
  returned 0.
- "No better art found" is a result, not a failure. Keep the full Baseline and
  the full log with it.
- No similarity score, no "strong" or "weak" rating, no invalidity statement.

## Working record

The working record holds what the report leaves out: the claim concordance
(searched claim numbers to granted claims), every candidate seen and why it was
dropped (date, already of record, no element match), the full text of each quote with
the tool and input it came from, and every tool call that ran.
