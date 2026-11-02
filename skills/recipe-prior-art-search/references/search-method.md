# Search method

How to write, probe and read a patent or literature search with the FlowLeap tools. The searching methods (`recipe-prior-art-search`, `recipe-find-better`, `recipe-invalidity-analysis` and the others) build every query with this method. `get_search_syntax` gives the full field grammar of each provider (`provider: "epo_ops"`, `"uspto"` or `"epo_register"`).

You write every query yourself. No tool turns a description into a query.

## 1. Extract the terms

1. List every specific noun phrase of the invention or claim: materials, mechanisms, subject matter ("sulfide glass ceramic", "foreign object detection"). For each phrase you leave out of the query, state why. When you are not sure of a word form ("glass ceramic" or "glass-ceramic"), OR both forms. Never drop the phrase that is the invention.
2. Write one synonym row for each claim element, not one row for the invention: the claim's own word, the structural equivalents another drafter uses, and the functional words for what the element does. Example (hydraulic actuator, EP4758353A1): pump: bidirectional, reversible, tiltable, over-centre, four-quadrant; chamber: cap side, rod side, piston side, annulus; pressure reservoir: accumulator, tank, pressure compensator.
3. Pick the **Discriminating Term**: the term that separates this invention from its technology area. A CPC or IPC code is never discriminating. A broad category word ("artificial intelligence", "sensor") is never discriminating. For a combination invention ("drone that inspects turbine blades"), the pair is the discrimination: `ta=drone AND ta="turbine blade"`.
4. When the subject is a published document, read its full `cpc[]` with `get_bibliography`, including the 2000-series indexing codes (for example `F15B2211/7053`). Indexing codes describe features independent of wording.

## 2. Write the CQL (`search_patents`, `provider: "epo_ops"`)

Every field in the query must be in this table.

| Field | Meaning | Wildcards |
|---|---|---|
| `ta` | title and abstract together (preferred: one term instead of two) | yes |
| `ti` / `ab` | title only / abstract only | yes |
| `txt` | title, abstract, applicant and inventor | yes |
| `pa` | applicant (`pa=GOOGLE*`, `pa="TESLA INC"`) | yes |
| `in` | inventor (`in="SMITH JOHN"`) | yes |
| `ic` / `cpc` / `cl` | IPC / CPC / all classifications | no |
| `pn` / `ap` / `pr` | publication / application / priority number | no |
| `ct` | documents that cite this one (`ct=EP1234567`) | no |
| `pd` | publication date (`pd<20230810`, `pd>=2020`, `pd within "2020 2023"`) | no |

These rules give an error answer, not a bad result:

- At most about 10 terms in one query.
- No wildcards on `ic`, `cpc`, `cl`, `pn`.
- Each term has 3 characters or more (write `ti=ultraviolet`, not `ti=uv`).
- `AND`, `OR`, `NOT` in capitals; phrases in double quotes.
- Parentheses group complete `field=value` clauses. Repeat the field: `(ta="glass ceramic" OR ta="glass-ceramic")` is correct; `ta=(turbine OR blade)` is refused.

Put the synonyms of one row in one OR group, AND the rows together, and add the date limit. `ta=` matches the English title and abstract, so English synonyms come first; add German or French terms when DE or FR documents may carry only original-language text. Two or three discriminating terms usually bring a count to a workable size.

**Recall or precision.** For novelty, invalidity and FTO, start broad (no classification, synonyms OR-ed, parent CPC class, wide dates) and narrow from the count. For the few closest documents, add a second `ta` term, narrow to a subgroup and tighten the dates. You cannot see what a too-narrow query did not return. The default balanced query has one discriminating `ta` term, one classification and one date bound.

**Classification-only track.** A main group AND 2-3 indexing codes that map to essential elements, with the date limit and no terms, gives a set small enough to read in full. It finds documents whose wording no keyword query matches. Example: `cpc=F15B7/006 AND cpc=F15B2211/7053 AND cpc=F15B2211/785 AND pd<20230810` (45 hits, probed 2026-10-11). One code alone is a neighbourhood, not a set.

**Verify each CPC code.** Never guess a code: CPC reclassifies (photovoltaics moved from `H01L31` to `H10F` in 2023). With FlowLeap Data, `patstat_cpc` (`q`: a keyword) gives classes and main groups, and `patstat_query` (`sql`: one SELECT on `flowleap.cpc_scheme`; `question`: the user's question, verbatim) gives subgroups and indexing codes (for example `SELECT symbol, title FROM flowleap.cpc_scheme WHERE title ILIKE '%photovoltaic%' ORDER BY symbol LIMIT 15`). Without FlowLeap Data, take codes only from the `cpc[]` of documents you read with `get_bibliography`, and mark each other code as unverified. A 4-character class carries only a headline (`H10F`: inorganic semiconductor devices sensitive to radiation); match your keywords against the titles of its groups (`H10F10/00`, `H10F71/00`), then search with the class (`ic=H10F`) or the exact group (`cpc=H10F10/00`).

## 3. Probe the count

Probe every query before you trust it. Call `search_patents` with `range: "1-1"` and `details: false`, and read `total`. The `countries` filter is part of the CQL that runs, so `total` counts the filtered set; `effectiveQuery` shows the CQL that ran.

- Over about 1,000 hits: add the next discriminating term.
- Under 10: broaden. Remove the classification, OR in synonyms, use the parent class, widen the dates.

Log every query with its tool, its input and its count, 0 included.

## 4. Read the whole set

- Results come back newest first, not by relevance. Older art is at the end.
- One page holds at most 100 results. Page with `range: "101-200"`, `"201-300"`, and so on.
- When a set is too large to read (over about 300), narrow it (the next synonym row, another indexing code, a date window) until you can read it, and log each step.
- Never judge a set, or record a document as absent, from its first page.
- OPS returns one publication per family: a hit can be the CA, WO or JP member of the US document you know (US2013333894A1 comes back as CA2828987A1). Compare documents by family with `get_family`, not by number.

## 5. The USPTO leg (`search_patents`, `provider: "uspto"`)

The USPTO Open Data Portal is a different collection from the EPO data. For US art, run this leg; `countries: ["US"]` on `epo_ops` gives the EPO copy of US documents, not USPTO records.

- The query is Lucene over application metadata. The only free-text field is `applicationMetaData.inventionTitle`: no abstract and no claims. Read the other field names from `get_search_syntax` (`provider: "uspto"`).
- Use the same terms and the same count rule, but the Discriminating Term must be a word that can be in a title. Never AND a feature that is only in the abstract (for example "UV-C sterilization" on a case titled "CHARGING CASE FOR EARBUDS").
- Probe with `limit: 1` and read `count`. Page with `limit` (at most 100) and `offset`.
- Limit dates with `range_filters`, for example `"applicationMetaData.filingDate 1900-01-01 2008-04-16"` for filings before a priority date of 2008-04-17.
- When the leg returns 0 and the query has a `cpcClassificationBag` filter, run it again once without that filter. When it is still 0, run a title recall pass on the core device noun with singular and plural forms, for example `applicationMetaData.inventionTitle:earbuds AND applicationMetaData.inventionTitle:"charging case"`, and triage the hits with `get_abstract`.
- 0 results is not a missing key. Only an answer that says the key is missing or not configured is a key stop.

## 6. Non-patent literature

1. `search_npl` (OpenAlex) first. Input: `query`, `limit` (at most 50), `page`, and `filter` with `from_year`, `to_year`, `open_access`, `type` (`journal-article`, `proceedings-article`, `book-chapter`, `preprint`, `review`, `book`, `dissertation`, `report`, `standard`, `dataset`). Write the discriminating terms in paper wording and run one variant for each essential feature.
2. `search_academic` (Semantic Scholar and arXiv) as the second pass, for preprints and computing or physics work. Input: `query`, `sources` (`semantic-scholar`, `arxiv`), `max_results`, `filter` with `from_year`, `to_year`. Remove duplicates of the first pass by DOI or title. A source that `sources[]` names as `rate_limited` or `error` is a gap, not "no literature".
3. A year filter is coarse: check each publication date against the exact critical date.
4. Papers an office already cited are resolved, not searched: `get_bibliography` with `resolve_npl: true` resolves every `npl` entry of `citedReferences[]` (for an EP family, on the A3 search-report publication); `get_npl_work` (`reference`: a DOI, an OpenAlex id or the citation string as printed) resolves one. Only `status: "matched"` is the cited paper. For `candidates`, say that none is certain and do not pick one. For `not_found`, cite the office's string as printed.
5. Read past the abstract (the open-access link or DOI) before you rely on a paper. Cite it by authors, title, venue, date and DOI.
6. The literature tools need no office key. Literature is different data, never a substitute for an office that has a missing key.
7. Product manuals, standards bodies' pages and other web disclosures: the FlowLeap Patent AI app has no web tool. In the app, name them as not searched. In a client with a web tool, read the page and cite its URL and date.

## 7. Citations

- Backward: `citedReferences[]` of `get_bibliography` (`docId`, `kind`, `npl`). For an EP family, the search-report citations are on the A3 publication; an EP B1 has none.
- Forward, all offices: `get_citations` (`patent_number`, `type`: `all`, `examination`, `opposition`, `applicant` or `other`, `range`). Its rows do not say who made each citation.
- Forward, US office actions: `search_enriched_citations` (`cited_document`, `category`, `examiner_cited_only`). Write `cited_document` as the office prints it, country, number and kind code separated by spaces (`US 20150284881 A1`); the compact form (`US20150284881A1`) returns 0. The art cited against one US application: `search_office_action_citations` (`application_number`, `category: "X"`, `examiner_cited_only: true` for the novelty-destroying references); counts only: `get_citation_stats`.
- Every document the examiners cited across a family, by office: `examiner_baseline` (`publication`).
- With FlowLeap Data, `patstat_patent` gives the origin of each citation (examiner search or applicant), and `patstat_neighborhood` (`node`, `depth: 2`, `edge_types: ["cites"]`) gives a two-hop citation set. A truncation notice means the list is capped: narrow it, never read it as complete.

## 8. The Search Statement block

Show this block each time you turn a claim or an invention into a query the user can run. Probe the statement first and log it as `Search Statement v<N>` (v1 first, plus 1 for each change) with its count. When the probe cannot run, still show the block, write `not run: <reason>` in place of the count, and say the statement is not in the query log.

````markdown
**Search Statement v1**: <what the statement looks for>, <total> hits

| Concept | Synonyms | CPC |
|---|---|---|
| <concept> | <term>, <term>, <term> | <code or none> |

```cql
<the statement exactly as it ran>
```

[Open in Espacenet](https://worldwide.espacenet.com/patent/search?q=<encoded statement>)

Discriminating Term: <term>

Ask me to change a concept and I regenerate the statement; the query log keeps every version that ran.
````

- Fence tag: `cql` for an EPO statement, `lucene` for a USPTO statement. A USPTO statement gets no Espacenet link.
- Espacenet runs the CQL unchanged for `ti`, `ab`, `ta`, `txt`, `pa`, `in`, `cpc`, `ic`, `pd`. Encode the statement with `encodeURIComponent`, then replace `(` with `%28` and `)` with `%29`, so the link does not end early.
- With no Discriminating Term, replace the `Discriminating Term:` line with `⚠ No Discriminating Term — this statement returns the field, not the invention` and say which concept the user can add.
- When the user changes a concept, rebuild the statement, probe it as the next version and show the whole block again. Never edit an earlier block.

Example: `(ta="wireless charging" OR ta="inductive charging" OR ta="wireless power") AND (ta="foreign object" OR ta="metal object") AND (cpc=H02J50/60 OR ic=H02J50) AND pd<2016`, Discriminating Term: foreign object.
