---
name: recipe-freedom-to-operate
description: "Freedom-to-operate clearance for a product or technology: search queries for each product feature, a recall search for blocking patents in the EPO and USPTO databases, checks of legal status, designated states and family for each candidate, and an all-elements check of each independent claim of each live candidate, with unread claims and missing-key offices kept as open gaps. Use it when the user asks whether a product or technology can launch without infringing, or asks for an FTO or clearance search."
metadata:
  requires:
    skills: ["recipe-prior-art-search"]
---

# Recipe: Freedom-to-Operate (FTO) Search

**Tools.** This method calls the FlowLeap patent tools by name. In the FlowLeap Patent AI app the office tools are built in; `patent_analytics`, `get_fulltext` and the `patstat_*` tools are FlowLeap Data tools, named `mcp__flowleap__<name>`, which exist only when the firm or the user has FlowLeap Data. With the FlowLeap connector in another client, call the connector tool of the same name. When `get_claims` answers that it has no US source, call the tool that its answer names.

**Missing keys.** When a tool answers that an office's key is not configured or that the office rejected the stored key, that office is a user-action stop, never an exhausted route: finish the work with the other offices, name the gap in the result as a missing-key gap (not as "no results"), never fill it with data from the web, and at the end tell the user which key is missing or rejected. On the desktop app the user adds it in Settings > Patent data connectors; on the firm OS the firm administrator sets the keys, so tell the attorney to ask the administrator; in another client, relay the page that the tool's answer names. Never ask for a key value in the chat.

Search for patents that can block a product or technology in the markets where it will be sold. Get the product features and the sales jurisdictions from the user before Step 1.

## Step 1: Write a query for each feature

Build each query with the search method of `recipe-prior-art-search` (`references/search-method.md`); `get_search_syntax` gives the field grammar of each provider.

For each key product feature, extract its specific noun phrases and build the query around the discriminating ones. FTO is a recall search: when the count is low, make the query broader (remove classifications, add synonyms with OR), not more precise.

- EPO: probe the count with `search_patents` (`provider: "epo_ops"`, `query: "<CQL for the feature>"`, `range: "1-1"`, `details: false`) and read `total`.
- USPTO: write a Lucene query for `search_patents` with `provider: "uspto"`. It searches US application titles and metadata, not claim text.

Done when each key feature has a probed EPO CQL query and a USPTO Lucene query.

## Step 2: Search for blocking patents

For each feature, call `search_patents` with `provider: "epo_ops"` (`range: "1-20"`) and with `provider: "uspto"` (`limit: 20`). Record each query, its `total` and the hits you keep.

**A missing key never makes the clearance scope smaller.** An office that answers with a missing key is not searched, and an office that is not searched is not cleared. Search the other office fully and record the missing office as a missing-key gap in the memo. When the user adds the key, search only that office again and merge the results.

Done when both offices are searched for each feature, or the office without a key is recorded as an open missing-key gap.

## Step 3: Keep only live candidates in the sales markets

For each candidate, call `get_patent_summary` (`patent_number`) for the bibliography, legal status, family and estimated term in one call. For the separate parts, call `get_legal_status` (`patent_number`) and `get_family` (`patent_number`). Keep each granted patent that is **in force** and each published application that is still **pending**, when it covers **a jurisdiction where the product is sold**.

A pending application is a live risk: it can grant with claims that read on the product, and a published application can give provisional protection (EPC Art. 67; 35 USC 154(d)). Keep it as "pending, claims may change", chart its current claims in Step 4, and name it for a watch. Drop only applications that are withdrawn, refused or deemed withdrawn.

Read the jurisdiction from the correct field. `get_family` names the **offices** where the invention was published; for an EP member that is one entry ("EP") and it tells nothing about countries. The countries are the `designatedStates` of the legal status (`extensionStates` for extension and validation states). For a European regional filing, these are its designated states. Then remove the states that lapsed according to the `PG25` and `PGFP` events, which each carry their own `state`. A patent that designates DE and still pays fees there blocks in Germany; a patent that lapsed in DE does not.

The designated states come from the `AK` event with the latest date. If that event has no list that can be read, an older event gives the list, and that list can be a wider one that names a state the applicant has since dropped. For clearance, a wider list is the dangerous direction: read the `AK` rows in the events before you flag or clear a market on the summary list alone.

Read the remaining term from `get_patent_term` (`patent_number`); it is an estimate of 20 years from filing, without term adjustment or SPC. For a US patent, `get_patent_term_adjustment` (`application_number`) gives the official adjustment.

Done when each candidate that remains is in force or pending and covers a sales jurisdiction. Drop the others and record why.

## Step 4: Check all elements of each independent claim

For each remaining candidate, read its claims with `get_claims` (`patent_number`) and compare the product features with each element of each independent claim. Quote each element from the tool result.

Office coverage: `get_claims` and `get_description` serve EP and WO; US claims come in English; US description is not served. For another office (CN, JP, KR, …) or a missing text, read a family member (`get_family`) in an office the tool serves, and label the text as the member's text.

**An unread claim is never a cleared claim.** A candidate at an office without a text source answers `FULLTEXT_UNAVAILABLE`. When a family member's claims can be read, chart from that member's claims and label the chart "member text, not the office's claims": it helps the reader, but it is not a decision about the office's own right. When no member's claims can be read, carry the candidate into the memo as "claims not readable here", an open gap of the same weight as a missing-key gap: never cleared, never dropped.

A claim clears only if at least one of its elements is absent from the product. Done when each independent claim of each candidate is cleared (one or more elements absent, each named), flagged as a live infringement risk, or recorded as "claims not readable here".

## Output

An FTO data package for each live blocking patent:

- Legal status (in force; pending, claims may change; expired; abandoned) and remaining term, each from the tool result that gave it.
- Geographic reach against the sales jurisdictions: family members for the offices, and `designatedStates` minus lapsed states for the countries of an EP filing.
- An all-elements result for each independent claim: cleared (with the absent element) or at risk. A chart built on a family member's claims carries the label "member text, not the office's claims". A right whose claims no source serves is "claims not readable here", an open gap.
- The queries, the counts, the offices searched, and each missing-key gap.

In the FlowLeap Patent AI app, save the report with `write_patent_results` (`template: "fto-memo"`, `filePath`: for example `fto/<product-slug>-fto-memo.md`, `subject`: the product, `jurisdictions`: the sales markets, `matter` when the user gave one, `content`: the analysis for each feature, which names each candidate blocking claim, its legal status and its jurisdiction). Elsewhere, write the report as a Markdown file.

To chart the at-risk claims element by element, continue with `recipe-infringement-charting`.
