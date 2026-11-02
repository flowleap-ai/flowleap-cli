# Fallback Coverage: the ladder, the report template, the Round Report

A proprietor under attack retreats to a fallback: a dependent claim, or an
amendment drawn from the description. A chart that covers the independent
claims but not the fallbacks stops where the litigator's work starts. This
file is the detail for Steps 2, 7 and 8 of `recipe-invalidity-analysis`.

In the FlowLeap Patent AI app, `write_patent_results` with `template:
"invalidity-claim-chart"` builds the ladder from the recorded `get_claims`
call and checks the coverage counts, the quotes and the round log. In another
client no tool checks them: you write them by hand, the reader checks them,
and the Limitations line of the report says so.

## Build the Fallback Ladder (Step 2)

1. Call `get_claims` (`patent_number`: the target) and keep the result.
2. Number the claims from the text. A claim with no reference to another
   claim is independent.
3. For each dependent claim, read the reference wording (table below) and
   record its parent or parents.
4. For each independent claim, walk down: every claim that refers to it, then
   every claim that refers to one of those, until no claim is left. Keep
   claim order.
5. For each rung, write the limitation the claim adds, in the claim's own
   words. Do not restate the parent claim.
6. A claim that refers to several parents ("claim 1 or 2", "any one of claims
   1 to 3") is a rung under each parent it names.
7. A claim that refers to an independent claim of another category ("Use of
   the holder according to claim 1") is a rung of that claim. Chart it, or
   name it as not charted with the reason.

If the claims text has no claim numbers, write "claim numbers could not be
established; no Fallback Ladder" in the report and continue with the
independent claims only.

### Dependency wording

| Language | Single parent | Several parents (a rung under each) |
|----------|---------------|-------------------------------------|
| EN | "according to claim 1", "as claimed in claim 1", "of claim 2" | "according to any one of claims 1 to 3", "as claimed in claim 1 or 2" |
| DE | "nach Anspruch 1", "gemäß Anspruch 1" | "nach einem der Ansprüche 1 bis 3", "nach Anspruch 1 oder 2" |
| FR | "selon la revendication 1" | "selon l'une des revendications 1 à 3", "selon la revendication 1 ou 2" |

### Ladder table

One table per independent claim. Copy the added limitation from the claims
text.

```markdown
| Rung | Depends on | Added limitation | Status | Reference, pin cite |
|---|---|---|---|---|
| 2 | 1 | titanium wire | covered | XX0000002A1, claim 4 |
| 3 | 1 or 2 | side opening for lateral insertion | not covered | - |
| 4 | 3 | resilient tab at the side opening | not covered | - |
| 5 | 1 | use on a carbon-fibre frame tube | not charted: use claim outside the attack | - |
```

Status is one of: covered (a quoted passage discloses the whole added
limitation), partly covered (a quoted passage discloses part of it), not
covered (no quoted passage), not charted (with the reason).

**Description fallback** (optional): an amendment the proprietor can draw from
the description. Add a row "d1" with the passage of the target's description
quoted verbatim as its basis. Chart it like a rung. It is unvalidated like the
rest.

## Count the Fallback Coverage (Step 7)

Read the counts off the ladder table. Count only rows with a quoted, pinned
passage. One line per independent claim:

"Claim 1: 4 of 4 elements disclosed; fallbacks: 1 of 4 covered (0 partly);
not covered: claims 3, 4; not charted: claim 5."

No "fallback strength". No model-rated score. No anticipation or obviousness
statement in the line: that judgment stays in your X/Y/A prose.

## Report template (Step 7)

Write `<target-patent>.invalidity.md`:

```markdown
# Invalidity Analysis: <target-patent>

Critical date: <YYYY-MM-DD> (<basis: earliest priority, or filing date when the priority claim fails>).
Legal status: <in force / lapsed, from get_patent_summary>. Venue and clock: <from Step 6>.
Offices searched: <EPO, USPTO>. Offices not searched: <office, reason>.

## Claim <n>

### Invalidity chart

| Element | <ref A> (<date>, X) | <ref B> (<date>, Y) |
|---|---|---|
| <n>a <claim language> | "<quote>" (<pin cite>) | not disclosed |

Combination rationale for every Y-pair: <which reference gives which element, the motivation, the legal citation>.

### Fallback Coverage

Claim <n>: <k> of <m> elements disclosed; fallbacks: <k> of <j> covered (<p> partly); not covered: claims <list>; not charted: claims <list>.

<the ladder table>

## Reference ranking

<the 3-5 strongest references; mark each NEW or of record>

## Search rounds

| Round | Answered | Query | Count | Found |
|---|---|---|---|---|
| 0 | first search | search_patents epo_ops: <CQL> | <n> | <ref or none> |
| 1 | rung-3, rung-4 | search_npl: <terms> (to_year <year>) | <n> | <ref or none> |

## Open points after the last round

<each rung still not covered, the queries run for it, the points the expert chose not to run, and what a deeper search would target>

## Limitations

<outside the app:> The ladder, the coverage counts and the round log are unvalidated: written by the agent, not checked by a tool.
<missing-key gaps, offices not searched, claims with no text source>

AI-assisted analysis for review by a registered patent attorney — not legal advice.
```

Round 0 is the first search of Step 3. Every later round carries the ids the
expert chose in "Answered". Copy each query exactly as you ran it, with its
tool.

## Open points and their ids (Step 8)

Read the open points off the report. Keep the ids stable across rounds.

| Id | Open point |
|----|------------|
| `rung-3`, `rung-d1` | a rung partly covered or not covered (`d1` = the first description fallback) |
| `element-1.2` | the second element of claim 1 is not disclosed in the cited art |
| `scope-XX0000004A` | a cited document from an office outside the searched scope |
| `date-XX0000005A` | a cited document published after the critical date, or with no date found (an application filed before the critical date and published after it stays prior art for novelty: EPC Art. 54(3), 35 USC 102(a)(2)) |
| `search-1` | a search that failed (error, timeout, key gate) and did not run again |

List them in this order: rungs, elements, scope, date, search. Each question
names the id and gives one recommended next step: the terms, office or source
you would search, or "accept as not covered".

## Round Report template (Step 8)

```markdown
Report written: <path>. Claim 1: fallbacks 1 of 4 covered; not covered: claims 3, 4.

1. [rung-3] No cited document shows a side opening. Recommended: search "side entry" and "lateral insertion" cages in both offices, then cycling NPL.
2. [rung-4] No cited document shows a retaining tab. Recommended: search "retaining tab" with the side-opening terms.
3. [date-XX0000006A] XX0000006A was published 2009-07-01, after the critical date. Recommended: drop it unless it is an earlier application (filed before the critical date: novelty art under EPC Art. 54(3) and 35 USC 102(a)(2)), or check whether the priority claim of claim 1 holds.
4. [search-1] The USPTO search failed with a key gate. Recommended: add the free USPTO ODP key and run it again.

Reply with the ids to run (for example rung-3, element-1.3), 'all', or your own steer.
```

Then wait. Do not search and do not rewrite the report before the expert
answers.

### Read the answer

- Ids, "all", or a steer in words: run one Search Round on those points. For
  a steer, name the ids the steer is about.
- Bare numbers ("1 and 2"): ask "question numbers or claim numbers?" and run
  nothing.
- "Stop", or no point chosen: end. The report as written is the result.

### Run the Search Round

1. Echo one line before any search: "Running: [rung-3] side opening for
   lateral insertion; [rung-4] resilient tab at the side opening."
2. New Discriminating Terms for the added limitation of each chosen point. Do
   not reuse the terms of the independent claim.
3. Search in this order: reformulate in `search_patents` (`provider:
   "epo_ops"`), the other office with `search_patents` (`provider: "uspto"`),
   then `search_npl` and `search_academic`, then a web search where the client
   has a web tool (the app has none: name the web as not searched).
   Search only for the chosen points. Keep the critical date on every query.
   In the app, each round needs at least one `search_patents` query: the
   writer refuses a round of only `search_npl` or `search_academic` queries.
4. Update the ladder table and the chart rows the round changed.
5. Add the round to "Search rounds" with its ids and every query as run.
6. Rewrite the Round Report from the updated report, and wait again.

One round per answer. "Not covered" after a round is a complete result.

## Worked example (SYNTHETIC)

The patent, the claims and the prior art below are invented to show the
shape. `XX` is not an office code. Never cite these numbers as real documents.

Target `XX0000001B1`, a bicycle bottle holder. `get_claims` returned:

1. A bottle holder for a bicycle frame, comprising a base plate with two
   mounting holes and a cage formed of a single bent wire attached to the base
   plate.
2. The holder according to claim 1, wherein the wire is of titanium.
3. The holder according to claim 1 or 2, wherein the cage has a side opening
   for lateral insertion of the bottle.
4. The holder according to claim 3, wherein a resilient tab at the side
   opening retains the bottle.
5. Use of the holder according to claim 1 on a frame tube of carbon fibre.

Critical date: 2009-04-17 (earliest priority, from `get_bibliography`).

Ladder of claim 1: claim 2; claim 3 (refers to 1 or 2, one rung under claim
1); claim 4 (through 3); claim 5. The first search finds `XX0000002A1`
(titanium wire, claim 4) and nothing for the side opening or the tab. Claim 5
is a use claim outside the attack: not charted, with that reason.

Fallback Coverage after round 0: "Claim 1: 4 of 4 elements disclosed;
fallbacks: 1 of 4 covered (0 partly); not covered: claims 3, 4; not charted:
claim 5."

Round Report: questions `[rung-3]` and `[rung-4]` as in the template above.
The expert answers "rung-3, rung-4".

Echo: "Running: [rung-3] side opening for lateral insertion; [rung-4]
resilient tab at the side opening." New Discriminating Terms: "side entry",
"lateral insertion", "retaining tab", "side-loading cage". Round 1 queries:

- `search_patents` (`provider: "epo_ops"`): `(ta="side entry" OR ta="lateral insertion") AND ta=cage AND ta=bottle AND pd<20090417`, `range: "1-30"`
- `search_patents` (`provider: "uspto"`): `applicationMetaData.inventionTitle:"side loading" AND applicationMetaData.inventionTitle:bottle`, `range_filters: "applicationMetaData.filingDate 1900-01-01 2009-04-16"`, `limit: 30`
- `search_npl`: `side-loading bottle cage retaining tab`, `filter.to_year: 2009`, `limit: 20`

Claim 3 is found in a dated product review (`XX-NPL-1`). Claim 4 is still not
found. Ladder row 3 becomes "covered, XX-NPL-1, p. 2". "Search rounds" gains
round 1 with "Answered: rung-3, rung-4" and the three queries. The new Round
Report lists `[rung-4]` only. The expert says stop. "Open points after the
last round" says: "Claim 4 (resilient retaining tab): not covered after one
Search Round. Queries run: the three in round 1. A deeper search would target
side-loading cages in product catalogues before the critical date."
