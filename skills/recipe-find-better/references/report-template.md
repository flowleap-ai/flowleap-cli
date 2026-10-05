# Find Better report template

In the CLI and MCP surface, the agent writes the report as markdown. The app's
typed writer (the `find-better-report` template) takes the same structure, so
keep the section order and column names.

Write two files: `<publication>.find-better.md` (the report) and
`<publication>.find-better.working-record.md` (the working record).

## Report

```markdown
# Find Better: <publication>

Critical date: <earliest priority date>. Family members walked: <n>. Offices: <list>.
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

| Track | Seed | Query | Count | Kept |
|---|---|---|---|---|
| 1 backward | <X/Y doc> hop 1 | ops biblio <X/Y doc> | <count> | <kept> |
| 2 inventors | <name> | in="<name>" AND pd<<critical date> | <count> | <kept> |
| 3 classification | <cpc> | cpc=<cpc> AND ta=<term> AND pd<<critical date> | <count> | <kept> |
```

## Rules

- "Disclosed n of m" counts only rows with a quoted, anchored passage.
- Score the examiner's art and the found art on the same element rows.
- Every track appears in the log, with every query, including the ones that
  returned 0.
- "No better art found" is a result, not a failure. Keep the full Baseline and
  the full log with it.
- No similarity score, no "strong" or "weak" rating, no invalidity statement.

## Working record

The working record holds what the report leaves out: the claim concordance
(searched claim numbers to granted claims), every candidate seen and why it was
dropped (date, not of record, no element match), the full text of each quote,
and the commands that ran.
