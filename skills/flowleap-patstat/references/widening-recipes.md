# PATSTAT widening recipes — chains, legal events, text discovery, INPADOC coverage

Four view families the typed commands do not reach. Each recipe below is the
**same SQL** as a verified query in the backend corpus, so this file and
`flowleap patstat docs --section examples` agree. Prefer the served copy when
you can run the command — it carries the live `status` and the measured
fingerprint. Use this file when you want the shape before you spend a call.

Replace the anchors (application ids, `LIKE 'BASF%'`, the CPC prefix, the seed
phrase, the year bounds) with the user's. Leave the **shape** alone: every
join direction, `DISTINCT`, code list, and bound in these queries is load
bearing, and the comment under each one says which fact it protects.

Run each through the normal guarded-SQL workflow in `SKILL.md` — schema-qualify
as `flowleap.<view>`, pass `--question` verbatim, state the interpretation and
the `data_edition` in the answer.

---

## 1. Chains — priorities and continuations (`flowleap.priorities`, `flowleap.continuations`)

A family is a **set** of applications that share priorities. A chain is a
**path** through the edges between them. They answer different questions:

- **Count over the DOCDB family.** "How many patents does X have" is
  `COUNT(DISTINCT family_id)` over `flowleap.family_members`.
- **Walk the chain for descent.** "What came out of this filing" is a walk over
  `flowleap.priorities` and `flowleap.continuations`, unioned — the two are
  halves of one chain, and a real portfolio uses both.

Both edge views point child → parent. Which column you filter on decides the
cost:

| You filter on | You walk | Cost |
|---|---|---|
| `application_id` | **up** — what this claims priority from, what it is a divisional of | Rides the primary key. Cheap. |
| `priority_application_id` / `parent_application_id` | **down** — what descended from this | One scan of a 54M-row table **per hop**. No index. |

**Bound the hops.** Descent has no index, so each hop is a scan. Two hops land
in the gate's WARN band and still run; three do not. Never write an unbounded
recursive walk. Materialize hop 1 as a CTE — inlining it re-scans and costs
more.

### `link_kind` — the decoded continuation relation

`flowleap.continuations.link_kind` decodes the raw DOCDB `link_type`. Match on
`link_kind`, not on the raw code:

| `link_kind` | Raw | Means |
|---|---|---|
| `continuation` | CON | US continuation of the parent |
| `divisional` | DIV | Divided out of the parent (EP and US practice) |
| `continuation-in-part` | CIP | Continuation that adds new matter |
| `internal-priority` | INN | Domestic priority, mostly DE |
| `addition` | ADD | Patent of addition |
| `reissue` | REI | Reissue of the parent |
| `cognate` | CGT | Cognate application |
| `substitute` | SBS | Substitute for the parent |
| `supplementary-disclosure` | SUP | Supplementary disclosure |
| `patent-to-utility-model` | P2U | Converted to a utility model |
| `utility-model-to-patent` | U2P | Converted to a patent |
| `unknown` | blank | The office published no code |

An undocumented future code passes through raw. `unknown` is common on
non-US offices and is not an error.

### Recipe: descendants of application X, two hops

Backend verified query: `chain_descent_two_hops`.

**Dedupe on `application_id`.** The answer is a set of applications, not a set
of edges. The hops overlap heavily and one filing is commonly reachable by a
priority edge **and** a continuation edge at once. On the proven anchor, 158
of 337 applications are reached twice, and counting raw edge rows inflates the
total by about 47%. Each application is reported once, at the first hop that
reaches it. `reached_via` is the set of edge kinds that reach it anywhere
within the bound — `continuation,priority` is one application with two edges,
not two applications.

```sql
WITH hop1 AS (
  SELECT pr.application_id, 'priority'::text AS link
  FROM flowleap.priorities pr
  WHERE pr.priority_application_id = 905384508
  UNION
  SELECT co.application_id, co.link_kind
  FROM flowleap.continuations co
  WHERE co.parent_application_id = 905384508
), hop2 AS (
  SELECT pr.application_id, 'priority'::text AS link
  FROM flowleap.priorities pr
  JOIN hop1 h ON h.application_id = pr.priority_application_id
  UNION
  SELECT co.application_id, co.link_kind
  FROM flowleap.continuations co
  JOIN hop1 h ON h.application_id = co.parent_application_id
), descent AS (
  SELECT e.application_id,
         MIN(e.hop) AS hop,
         string_agg(DISTINCT e.link, ',' ORDER BY e.link) AS reached_via
  FROM (SELECT application_id, link, 1 AS hop FROM hop1
        UNION ALL
        SELECT application_id, link, 2 AS hop FROM hop2) e
  GROUP BY e.application_id
)
SELECT d.hop, d.reached_via, a.office,
       COUNT(*) AS applications,
       COUNT(DISTINCT a.family_id) AS families
FROM descent d
JOIN flowleap.applications a ON a.application_id = d.application_id
GROUP BY 1, 2, 3
ORDER BY 1, 4 DESC
```

The cheap opposite direction — what does X claim priority from, what is X a
divisional of — is `chain_ancestry` in the served examples. Use it whenever the
question has a known starting document and looks backwards.

### Recipe: divisional share per applicant in a CPC prefix

Backend verified query: `divisional_share_by_applicant`.

Chains as an **analytic signal** instead of a walk. A high divisional share is
a continuation-strategy tell — a portfolio kept alive through pending
children. The `continuations` join is on `application_id`, the cheap indexed
direction, so the whole query stays far below the warn band.

Divisionals are counted per **application** here, not per family: the question
is about filing behaviour at an office, not about inventions. Say so in the
answer.

```sql
SELECT ap.name AS applicant, a.filing_year,
       COUNT(DISTINCT a.application_id) AS applications,
       COUNT(DISTINCT co.application_id) AS divisionals,
       ROUND(100.0 * COUNT(DISTINCT co.application_id)
                   / COUNT(DISTINCT a.application_id), 1) AS divisional_share_pct
FROM flowleap.classifications c
JOIN flowleap.applications a  ON a.application_id = c.application_id
JOIN flowleap.applicants ap   ON ap.application_id = a.application_id
LEFT JOIN flowleap.continuations co
       ON co.application_id = a.application_id AND co.link_kind = 'divisional'
WHERE c.cpc_code LIKE 'H01M10/056%'
  AND a.filing_year BETWEEN 2010 AND 2022
  AND a.ipr_type = 'PI'
GROUP BY 1, 2
HAVING COUNT(DISTINCT a.application_id) >= 20
ORDER BY divisional_share_pct DESC, applications DESC
```

Most applicant-years come back at 0.0%. Divisional practice is concentrated,
not uniform — that spread is the finding, so do not present only the head of
the ranking.

---

## 2. Legal events — decoded (`flowleap.legal_events`, `flowleap.legal_event_codes`)

Five rules, all of them about not producing a confidently wrong number.

**Opposition is not one category. Enumerate the codes first.** Event codes are
office-specific and a legal concept is a **list** of codes, never a category.
The 69 EP codes whose description mentions opposition sit in three categories:
L (IP right review request), Y (correction and deletion of event information)
and W (other). Filtering `event_category = 'L'` would both miss opposition
codes and sweep in every other kind of review request. Run the code-book query
below **before** any legal-event aggregate, for the office and the concept in
front of you.

**The shortlist is a starting point, not the answer.** Sibling codes carry the
opposite meaning — `NO OPPOSITION FILED`, `OPPOSITION DEEMED NOT TO HAVE BEEN
FILED`, `OPPOSITION FILED (DELETED)`. Read the descriptions and curate by
hand.

**Count DISTINCT applications, never event rows.** Offices publish the same act
in several streams. EP `26` and `PLBI` both mean OPPOSITION FILED and co-occur
on nearly every opposed EP grant, so counting rows inflates the answer by
215% on the proven anchor.

**Sentinels are already NULL.** PATSTAT's "no value" markers — a 9999-12-31
date, a blank char column, `fee_renewal_year` 0 or 9999 — are scrubbed to NULL
in this view. `IS NULL` is the right test everywhere. Do not re-implement the
scrub.

**Anchor on a bounded application set.** `flowleap.legal_events` is a 519M-row
table reached by an index on `application_id`. Always build the anchor set
first (an applicant's grants, a technology area) and join legal events onto
it; never filter legal events first. Runtime grows with the anchor: about 4k
applications return in ~2 s, while 25k applications measured 110 s and would
blow the role's 20 s statement timeout. Bound by office, `filing_year` or
technology before widening the question.

**As of edition.** These are snapshot events. Any answer that touches them must
point the user at the live document layer — the `get_legal_status` tool — for
current status. Never present a PATSTAT legal event as today's state.

### Step 0: which codes mean this concept?

Backend verified query: `legal_event_codes_for_concept`. The code book is 4,332
rows. Scan it freely.

```sql
SELECT c.code, c.description, c.category, c.category_title
FROM flowleap.legal_event_codes c
WHERE c.office = 'EP'
  AND c.description ILIKE '%opposition%'
ORDER BY c.category NULLS LAST, c.code
```

### Recipe: EP oppositions against applicant X's grants, by year

Backend verified query: `ep_oppositions_by_year`.

The curated EP code list, derived from step 0 and then hand-checked:

| Code | Description | In? | Why |
|---|---|---|---|
| `26` | OPPOSITION FILED | **yes** | The EP Bulletin code. |
| `PLBI` | OPPOSITION FILED | **yes** | The INPADOC-stream twin of `26`. Same act, published twice. |
| `R26` | OPPOSITION FILED (CORRECTED) | **yes** | A correction to a filing event. The opposition still happened. |
| `D26` | OPPOSITION FILED (DELETED) | no | The event was retracted. |
| `26N`, `PLBE` | NO OPPOSITION FILED | no | The opposite outcome, and far more common than the filings. |
| `26D`, `PLBG`, `PLBH` | Deemed not filed | no | The opposite outcome. |
| `26U`, `PLBJ`, `PLBK` | Found inadmissible | no | The opposite outcome. |
| `NLR1` | NL: OPPOSITION FILED WITH THE EPO | no | A national mirror of the same EPO opposition. Including it double-counts. |
| `PLAX`, `PLBB`, `27C`, `27O`, `PLBN` … | Later stages | no | Later stages of the **same** opposition. |

One grant is counted once, in the year of its first opposition-filed event.
`event_rows` is carried alongside so the inflation ratio stays visible. The
baseline is **2 rows per opposed grant**, not 1, because `26` and `PLBI` both
fire on one opposition. A grant with more than 2 was opposed by several
opponents, or had its filing event corrected.

```sql
WITH grants AS (
  SELECT DISTINCT a.application_id
  FROM flowleap.applications a
  JOIN flowleap.applicants ap ON ap.application_id = a.application_id
  WHERE UPPER(ap.name) LIKE 'BASF%' AND a.office = 'EP' AND a.granted
), opposed AS (
  SELECT le.application_id,
         MIN(le.event_date) AS first_opposition_date,
         COUNT(*)           AS opposition_event_rows
  FROM grants g
  JOIN flowleap.legal_events le ON le.application_id = g.application_id
  WHERE le.event_office = 'EP'
    AND le.event_code IN ('26', 'PLBI', 'R26')
    AND le.event_date IS NOT NULL
  GROUP BY le.application_id
)
SELECT EXTRACT(YEAR FROM o.first_opposition_date)::int AS opposition_year,
       COUNT(*)                       AS grants_opposed,
       SUM(o.opposition_event_rows)   AS event_rows
FROM opposed o
GROUP BY 1
ORDER BY 1
```

### Recipe: lapses per year for applicant X in EP

Backend verified query: `ep_lapses_by_country_and_year`.

**This counts `PG25` and `VS25` only** — the two EP codes that carry a real
`lapse_date`:

- `PG25` — LAPSED IN A CONTRACTING STATE, announced post-grant from the
  national office to the EPO.
- `VS25` — LAPSED IN A VALIDATION STATE, same announcement route.

**Category H is not the filter, and this is the trap.** H is "IP right
cessation", and at EP it also holds 17 codes that are not lapses: `27W`,
`RDAA`, `RDAF`, `RDAG`, `RDAH`, `GBPR` (revoked), `BE20` (expired at full
term), `NLV5`, `MEDN`, `LTIE` (annulment, invalidation), `NLV6` (surrendered),
`NLV7` (maximum lifetime reached), `GBV` (treated as void). Reading an H
aggregate as "lapses" silently merges a patent the owner stopped paying for
with one an opponent destroyed — opposite facts about a portfolio. On the
proven anchor the same grant set carries 350 EP revocation events and 576 LT
invalidations under H, none of them a lapse.

**One office's stream only.** These are the EP-published post-grant
announcements. The same national lapse is often also published by the national
office under its own codes (`GBPC`, `BERE`, `NLV4`, `EUG` …), which carry no
`lapse_date` and are excluded. Present this as "what EP published", not as
"every lapse everywhere", and do not union the national codes in without
re-deduping.

**`event_office` is not `lapse_country`.** An EP-published cessation names the
national state that lost the right: `event_office` is `EP`, `lapse_country` is
the state. That pair is the point — a portfolio shrinks state by state, not
all at once.

The filing-year bound is **cost**, not semantics.

```sql
WITH grants AS (
  SELECT DISTINCT a.application_id
  FROM flowleap.applications a
  JOIN flowleap.applicants ap ON ap.application_id = a.application_id
  WHERE UPPER(ap.name) LIKE 'BAYER%'
    AND a.office = 'EP' AND a.granted
    AND a.filing_year BETWEEN 2000 AND 2015
)
SELECT le.lapse_country,
       EXTRACT(YEAR FROM le.lapse_date)::int      AS lapse_year,
       COUNT(DISTINCT le.application_id)          AS patents_lapsed
FROM grants g
JOIN flowleap.legal_events le ON le.application_id = g.application_id
WHERE le.event_office = 'EP'
  AND le.event_code IN ('PG25', 'VS25')
  AND le.lapse_date IS NOT NULL
GROUP BY 1, 2
ORDER BY 2 DESC, 3 DESC
```

---

## 3. Text discovery — concept to identifiers (`flowleap.application_texts`)

The entry point that lets an agent go from a **concept** to CPC codes without a
hardcoded table.

**Discovery returns identifiers, never document text as the answer.** A text
match finds a candidate set. Turn it into an aggregate (a code distribution, a
trend, a ranking) or a shortlist of identifiers. Every document read — claims,
description, live status — goes to the live document layer: `flowleap-ops`
(EPO OPS) or `flowleap-uspto` (USPTO ODP). Never hand back an abstract from
this view as the deliverable.

**The match idiom is exact.** The GIN indexes are partial expression indexes,
and Postgres uses them only when the query matches both halves:

```sql
to_tsvector('english', tx.title) @@ plainto_tsquery('english', $q)
  AND tx.title_lang = 'en'
```

Any other spelling — `ILIKE`, a different regconfig, no language predicate —
seq-scans 122M title rows, plans at 7.1M–17M against the gate's 5M ceiling, and
is **rejected**, not merely slow.

**Abstract variant: same idiom, different columns.** Swap `tx.title` for
`tx.abstract` and `tx.title_lang` for `tx.abstract_lang`. The abstract index
exists, so the shape is legal. It is the **more expensive path** and the
higher-recall one: prefer the title for a tight concept, the abstract when the
title match comes back thin.

**EXPLAIN does not bound a GIN seed.** Postgres estimates about 1 row per GIN
match however broad the seed, so a query touching thousands of applications
still plans in the hundreds. The cost gate does not bound a text search at
all. What bounds it is the 20 s statement timeout and the 5,000-row cap — and
you. Narrow it yourself with `ipr_type = 'PI'`, a year floor, and/or an office,
applied **before** joining out to `classifications` or `applicants`.

**English is a slice, not the corpus.** Titles are English on 86% of rows,
abstracts on 92%. An English-title match under-counts JP and CN filers badly.
Say so whenever it matters to the question.

### Three traps, all measured on real answers

1. **Circularity.** The seed terms decide the corpus, so a weak seed returns
   the wrong art and then a confidently wrong code. Cross-check with a second
   phrasing before you believe a ranking.
2. **Generic co-occurring codes.** Patents carry many CPC codes, so raw
   frequency surfaces the broad code, not the discriminating one. On the proven
   `solid state battery electrolyte` seed, rank 1 is `Y02E60/10` "Energy
   storage using batteries" — a Y-scheme tag on 86.7% of hits. The real answer
   is rank 2. Never take the mode: join `cpc_scheme` for the titles, read the
   top few, and sanity-check a candidate against its own corpus size
   (`COUNT(DISTINCT family_id)` over `classifications` for that code, ~200 ms).
   A code whose corpus dwarfs the hit set is a tag, not the area.
3. **Reclassification mix.** PATSTAT is a snapshot and offices reclassify in
   bulk, so legacy and current codes coexist — `H01L` and `H10F` for
   photovoltaics on this edition. The mix is informative. Taking the mode
   blindly is not. Verify any code you derive against `flowleap.cpc_scheme`
   before landscaping with it.

### Recipe: concept → CPC distribution

Backend verified query: `concept_to_cpc_codes`.

Companion to `cpc_candidate_codes`, which it does **not** replace.
`cpc_candidate_codes` asks the official scheme "which codes are *named* this";
this one asks the corpus "which codes are *used* for this". Run both and
compare — a code that is named for the concept but barely used, or used but
never named, tells you the seed phrasing is off.

`cpc_subclass` is display only. It is `LEFT(cpc_code, 4)`, functionally
dependent on the code beside it, so it groups nothing and its counts are
per-code. The family counts do **not** sum down that column. For a real
subclass rollup, group by the prefix alone in a separate query.

```sql
WITH hits AS (
  SELECT tx.application_id
  FROM flowleap.application_texts tx
  WHERE to_tsvector('english', tx.title) @@ plainto_tsquery('english', 'solid state battery electrolyte')
    AND tx.title_lang = 'en'
), scoped AS (
  SELECT h.application_id, a.family_id
  FROM hits h
  JOIN flowleap.applications a ON a.application_id = h.application_id
  WHERE a.ipr_type = 'PI' AND a.earliest_filing_year >= 2015
), codes AS (
  SELECT LEFT(c.cpc_code, 4) AS cpc_subclass,
         c.cpc_code,
         COUNT(DISTINCT s.family_id)      AS families,
         COUNT(DISTINCT s.application_id) AS applications
  FROM scoped s
  JOIN flowleap.classifications c ON c.application_id = s.application_id
  GROUP BY 1, 2
)
SELECT k.cpc_subclass, k.cpc_code, k.families, k.applications,
       ROUND(100.0 * k.families / (SELECT COUNT(DISTINCT family_id) FROM scoped), 1) AS share_of_hits_pct,
       sub.title AS subclass_title, grp.title AS code_title
FROM codes k
LEFT JOIN flowleap.cpc_scheme sub ON sub.symbol = k.cpc_subclass
LEFT JOIN flowleap.cpc_scheme grp ON grp.symbol = k.cpc_code
ORDER BY k.families DESC, k.cpc_code
LIMIT 30
```

**This is a shortlist, not a census.** An English-title match recalls a
language-skewed slice — measured at 2,353 families against 11,181 for the same
window under `H01M10/0562`, about 21%, with JP at 95 families against CN's
1,610. Use it to **find** the codes, then landscape over
`flowleap.classifications`, which is the census.

### Recipe: top applicants for a concept

Backend verified query: `concept_top_applicants`.

The same discovery shape answering a landscape question directly, when no CPC
code is known yet. Counted in families; `offices` shows how widely each
applicant files the same inventions.

**Read the ranking as text-derived, and say so in the answer.** It is not the
same ranking as a CPC landscape over the same area, and the difference is the
method, not the data: over `H01M10/0562` the ranking is Toyota > Panasonic >
Samsung SDI, while this one returns FUJIFILM > LG Energy Solution > LG Chem. A
four-term English-title match finds whoever writes English titles containing
all four stems. Once the codes are known, re-run the ranking over
`classifications`. Use this one to discover, or when no code fits the concept.

Applicant-name harmonization still applies: `LG ENERGY SOLUTION`, `LG NEW
ENERGY LTD.` and a CJK-script LG row are three rows for one group. Group
variants by hand before presenting a ranking.

```sql
WITH hits AS (
  SELECT tx.application_id
  FROM flowleap.application_texts tx
  WHERE to_tsvector('english', tx.title) @@ plainto_tsquery('english', 'solid state battery electrolyte')
    AND tx.title_lang = 'en'
), scoped AS (
  SELECT h.application_id, a.family_id, a.office
  FROM hits h
  JOIN flowleap.applications a ON a.application_id = h.application_id
  WHERE a.ipr_type = 'PI' AND a.earliest_filing_year >= 2015
)
SELECT ap.name AS applicant,
       COUNT(DISTINCT s.family_id)      AS families,
       COUNT(DISTINCT s.application_id) AS applications,
       COUNT(DISTINCT s.office)         AS offices
FROM scoped s
JOIN flowleap.applicants ap ON ap.application_id = s.application_id
GROUP BY 1
HAVING COUNT(DISTINCT s.family_id) >= 5
ORDER BY families DESC, applicant
LIMIT 25
```

### Where the hardcoded CPC tables now sit

The CPC tables inside the VS Code app skills
(`patent-search/references/cql-reference.md`,
`prior-art/references/cpc-classification.md`) are a **last-resort fallback**,
behind the PATSTAT lookup. Reach for them only when PATSTAT is unreachable
(`patstat_unavailable`) or the question cannot be phrased as a concept. A
hand-typed table drifts every quarter; `concept_to_cpc_codes` plus
`cpc_candidate_codes` read the same answer off the corpus and the official
scheme at the current edition. Those two files live in the `flowleap-agent-v2`
repository, not here — this skill is their upstream doctrine, not their host.

---

## 4. INPADOC coverage — the extended family (`flowleap.inpadoc_family_members`)

Two family notions. Picking the wrong one changes the number.

- **COUNT over DOCDB** (`flowleap.family_members.family_id`). A DOCDB simple
  family is the applications that share **exactly** the same priorities — the
  best available proxy for one invention. This stays the default counting unit
  everywhere.
- **COVER over INPADOC** (`flowleap.inpadoc_family_members`). The extended
  family is applications that share a priority **directly or indirectly**. The
  PATSTAT catalog says it "covers one or more DOCDB families and covers a set
  of *related* inventions". It adds everything linked through a third
  application, a continuation, or a technical relation an examiner drew
  between similar content.

**Counting inventions over INPADOC inflates.** On the proven anchor,
EP05820528 sits in a DOCDB family of 28 applications over 27 offices and an
INPADOC family of 67 over 40 offices, merging **6** DOCDB families — a 6×
overstatement if you count inventions over the extended family. State which
family you used in every answer.

**Never persist or cross-edition-compare an `inpadoc_family_id`.** PATSTAT
recomputes it every edition (it is the smallest `application_id` among the
members). A DOCDB `family_id` comes from DOCDB and is stable.

One row per application, and every application belongs to exactly one extended
family, so the view cannot fan out and needs no `DISTINCT`.

### Recipe: where is the extended family of application X in force

Backend verified query: `inpadoc_family_coverage`.

Anchoring on `application_id` rides the primary key and is cheap.

`granted` is as of edition: a granted member is a **live** right only if it has
not since lapsed. Point the user at `flowleap.legal_events` for the lapse
history, and at the `get_legal_status` tool for today's state. WO members never
grant, so a 0 in the WO row is definitional, not a refusal.

```sql
WITH anchor AS (
  SELECT inpadoc_family_id
  FROM flowleap.inpadoc_family_members
  WHERE application_id = 16277555
)
SELECT m.office,
       COUNT(*)                          AS members,
       COUNT(*) FILTER (WHERE m.granted) AS granted_members,
       MIN(m.filing_date)                AS earliest_filing
FROM anchor a
JOIN flowleap.inpadoc_family_members m ON m.inpadoc_family_id = a.inpadoc_family_id
GROUP BY m.office
ORDER BY members DESC, m.office
```

`MIN(filing_date)` over an extended family is **not** the priority date of one
invention — the family spans several.

### The count-versus-cover check, as a query

Backend verified query: `inpadoc_family_scope`. Run it before you present any
number derived from the extended family. `docdb_families_inside` is the
inflation factor.

```sql
WITH anchor AS (
  SELECT inpadoc_family_id
  FROM flowleap.inpadoc_family_members
  WHERE application_id = 16277555
)
SELECT COUNT(*)                     AS extended_members,
       COUNT(DISTINCT fm.family_id) AS docdb_families_inside,
       COUNT(DISTINCT m.office)     AS offices
FROM anchor a
JOIN flowleap.inpadoc_family_members m ON m.inpadoc_family_id = a.inpadoc_family_id
JOIN flowleap.family_members fm        ON fm.application_id = m.application_id
```

---

## If a recipe is not yet served

`flowleap patstat docs --section examples` is the live corpus. A recipe here
that the served list does not carry yet is **pending on the deployed edition**,
not wrong: the text-discovery and INPADOC entries land as `logical-pending`
until the backend re-runs its view layer and re-measures them. If a query comes
back with a missing-relation error, say that the view is not on the deployed
edition yet and stop — do not rewrite it against raw `tls*` tables, which the
gate rejects by design.
