# Reading the Examiner Baseline JSON

`flowleap --json patent examiner-baseline <publication>` prints one object.
In a chat client the `examiner_baseline` tool returns the same object.
Code computes every value from the offices' own records. Read it; do not
rebuild it from other calls. Fields are only added, never renamed.

## Top level

| Field | Meaning |
|---|---|
| `publication` | The publication you asked for. |
| `offices` | The offices of the family members walked. The requested office is first. These are the keys of each `cells` object. |
| `dedupe` | `"docdb-number"`: rows merge on country + number, kind dropped. Two family members of one cited document stay two rows. |
| `membersWalked[]` | The family members read, with the counts each office reported. |
| `documents[]` | One row per cited document. |
| `gaps[]` | What could not be read. A gap is "no record", never "nothing cited". |

## `membersWalked[]`

- `office`, `representativePublication`, `docdbApplication`.
- `publications[]`: one entry per publication of the member, with `status`
  (`read` or `no_citation_record`), `citedCount`, `examinerCount`,
  `applicantCount`. An EP B1 has no citation block; the EP citations come from
  the A2 and A3.
- `usptoEnriched` (US grants only): `applicationNumber`, `status`, `rows`,
  `total`, `unidentifiedRows`. An unidentified row is an office-action
  citation that names no document. Report the count; do not guess the document.

## `documents[]`

- `document`: DOCDB number without kind, for example `US5653436`.
- `kinds[]`: the kinds seen, for example `["A1"]`.
- `familyId`: always `null` today. Family dedupe is not applied.
- `cells.<office>.text`: the human cell text.
- `cells.<office>.citations[]`: each citation, with:
  - `source`: `ops_biblio` or `uspto_enriched`.
  - `citing`: the publication (OPS) or the US application number (USPTO) that
    carries the citation.
  - `citedBy`: `examiner` or `applicant`, copied from the source.
  - `category`: `X`, `Y`, `A`, or a combination such as `X,A`. Absent when the
    office gave none.
  - `relevantClaims`: for example `1-3,5,7,8`. For a combined category this
    is the first claim list of the search report, verbatim.
  - `categoryClaims[]` (OPS only): each category with its own claims, for
    example `[{"category": "X", "claims": "13"}, {"category": "A", "claims":
    "1,5,9"}]`. Present when the search report pairs them. The cell `text`
    then reads `X cl. 13, A cl. 1,5,9`.
  - `relevantPassages[]` (OPS), `phase` (OPS), `officeActionDate` and
    `officeActionType` (USPTO).

## Rules for Find Better

1. **X or Y means examiner-assessed.** Only an examiner assigns a category.
   OPS can mark a categorised citation `citedBy: "applicant"` (example:
   US7819306 on EP2743895A1, `A,D` claims 1-18). It is still examiner's art.
2. **Claim numbers belong to the citing claim set.** `relevantClaims` numbers
   the claims of `citing` as that office searched them: the application claims
   for an EP A3 search report, the pending claims for a US office action. Map
   each number to the granted independent claims by comparing claim text (a
   claim concordance). A searched claim with no granted counterpart reaches no
   granted claim; list it under "searched claims not granted".
3. **A combined category splits through `categoryClaims`.** `X,A` with
   `categoryClaims` X `1-5` and A `9,13` is X for claims 1-5 only. Use the
   pairs for ranking and quote them in the report. Without `categoryClaims`,
   `X,A` with claims `1-5` does not say which claims are X: treat the document
   as X for those claims and quote the category and claims verbatim.
4. **Gaps stay gaps.** Copy each `gaps[].message` into the report. A CN, JP or
   KR member gives citations and English abstracts only; say that element
   mapping against those documents is not possible.
5. **Stops are not gaps.** A key gate, 401, 402, 429 or 410 stops the verb with
   its usual exit code and hint. Follow `flowleap-keys` or `flowleap-shared`;
   do not report a partial Baseline as complete.

## Ranking the examiner's best art per independent claim

Any X or Y category counts as examiner-assessed, whatever `citedBy` says
(rule 1). The order of `documents[]` is not this ranking: it puts rows with an
`examiner` citation first, so a categorised row marked `citedBy: "applicant"`
can sit lower. Rank by the steps below.

1. Keep each citation whose `category` contains X or Y.
2. Map its X and Y claims to granted independent claims (rule 2): the
   `categoryClaims` pairs for X and Y when present, otherwise
   `relevantClaims` (rule 3).
3. Per granted independent claim, sort: X before Y; then more offices citing
   it with X or Y; then more independent claims reached.
4. When no X or Y citation exists (a US-origin family often has none), take
   the documents with `citedBy: "examiner"` and no category, then the
   documents a US office action rejected claims with under 102 or 103
   (`source: "uspto_enriched"`). Show that basis beside each document. Never
   take a document that only the applicant cited, with no category and no
   rejection.

## Worked example: EP2743895B1 (critical date 2012-12-17)

Granted independent claims 1 and 13 (claim 11 refers back to "any proceeding
claim"). Concordance: EP A1 claims 1 and 3 map to claim 1 (granted claim 1
adds the A1 claim 3 features); EP A1 claims 2 and 4-11 map to granted claims
2-10; A1 claims 12-13 map to claims 11-12; A1 claims 14-15 map to claim 13 and
its dependent 14. EP A1 claims 16-18 (the "aperture plate and shutter"
apparatus) were not granted.

| Granted claim | Examiner's best art | Evidence in the Baseline |
|---|---|---|
| 1 | US5653436 A | EP `X` A1 cl. 1,12-15 |
| 13 | US5653436 A | EP `X` A1 cl. 1,12-15 (cl. 14-15 map to claim 13) |

The EP evidence is the X pair of each `categoryClaims`: US5653436 reads
`X cl. 1,12-15, Y cl. 2-11,16-18` on the A1, so only A1 cl. 1,12-15 are X.

Not best art for an independent claim: WO2009103933 A1 (EP `Y` A1 cl.
2-11,16-18) reaches only dependent claims 2-10, not claim 1. Searched claims
not granted: US5653436 A and WO2009103933 A1 on A1 cl. 16-18, and
WO2010014035 A1 (EP `X` cl. 16, citing EP2743895B1; the grant has 14 claims).
Gaps: CN100594522 C is `X` cl. 16-18 of CN103871153A, but a CN member gives
citations only, so its claim numbers cannot be mapped to the granted claims.
No USPTO enriched-citation record for US9290983B2 (application 14101625): the
US citations carry no category, so none counts as examiner's best art.
