---
name: recipe-patent-landscape
description: "Map the patents of a technology area: a scoped search in the EPO and USPTO databases, the key players, filing trends, the jurisdiction split and white space found by CPC class against year, each figure with its counting basis and source. Uses the FlowLeap Data analytics and PATSTAT tools when they exist, and live search counts when they do not. Use it when the user asks to map a technology area, to find who patents in it, or to report filing trends and white space."
metadata:
  requires:
    skills: ["recipe-prior-art-search", "recipe-custom-dashboard"]
---

# Recipe: Patent Landscape Analysis

**Tools.** This method calls the FlowLeap patent tools by name. In the FlowLeap Patent AI app the office tools are built in; `patent_analytics`, `get_fulltext` and the `patstat_*` tools are FlowLeap Data tools, named `mcp__flowleap__<name>`, which exist only when the firm or the user has FlowLeap Data. With the FlowLeap connector in another client, call the connector tool of the same name. When `get_claims` answers that it has no US source, call the tool that its answer names.

**Missing keys.** When a tool answers that an office's key is not configured or that the office rejected the stored key, that office is a user-action stop, never an exhausted route: finish the work with the other offices, name the gap in the result as a missing-key gap (not as "no results"), never fill it with data from the web, and at the end tell the user which key is missing or rejected. On the desktop app the user adds it in Settings > Patent data connectors; on the firm OS the firm administrator sets the keys, so tell the attorney to ask the administrator; in another client, relay the page that the tool's answer names. Never ask for a key value in the chat.

Map the patents of a technology area: the key players, the trends and the gaps.

**Every figure carries its counting basis and its source.** The basis is families, applications, publications or live search hits. The source is the tool and its input, with the `data_slice` of `patent_analytics` or the PATSTAT edition of a `patstat_*` answer. Never add figures of different bases together.

**Without FlowLeap Data.** When `patent_analytics` and the `patstat_*` tools are not available, do Steps 3 and 6 with count probes of `search_patents` (see each step), label each figure "live search count, publications", and state in the report that the full-corpus analytics were not available.

## Step 1: Define the scope

Build each query with the search method of `recipe-prior-art-search` (`references/search-method.md`); `get_search_syntax` gives the field grammar of each provider.

Extract the specific noun phrases of the area and keep at least one discriminating term: a CPC class alone covers a whole technology field, not a landscape. Probe the count with `search_patents` (`provider: "epo_ops"`, `query: "<CQL>"`, `range: "1-1"`, `details: false`) and read `total`. A landscape query can be broader than a novelty query, but a count in the tens of thousands means the scope is the technology field, not the landscape: narrow it.

Verify each CPC code before you scope on it. With FlowLeap Data, call `patstat_cpc` (`q`: a technology keyword) and use only codes from its answer; the main-group titles name the specific technology, the subclass only the general field. Without it, take the codes from `get_bibliography` (`patent_number`) of the closest hits and mark the meaning of each code as not verified.

Done when you have a probed EPO CQL query and a USPTO Lucene query for the area.

## Step 2: Broad patent search

Call `search_patents` with `provider: "epo_ops"` (`range: "1-50"`) and with `provider: "uspto"` (`limit: 50`). The USPTO query searches US application titles and metadata, not claim text.

If one office answers with a missing key, map the other office fully and label the landscape as covering that office only because of a missing key, never as the real distribution of the field. The PATSTAT counts are twice-yearly snapshots; they do not replace a live search of the missing office.

Done when both offices returned their result sets, or the office without a key is named as an open missing-key gap.

## Step 3: Filing trends and jurisdiction split

With FlowLeap Data:

- `patent_analytics` gives filings per year, country and CPC breakdowns and top assignees over a worldwide corpus. Give at least one criterion: `keywords` or `phrases` (title and abstract substrings), `cpc` (prefixes), `countryCode`, `dateFrom` and `dateTo` (`YYYY-MM-DD`). For example `keywords: ["<technology term>"]`, `dateFrom: "2015-01-01"`, then `cpc: ["<prefix>"]`, `countryCode: "US"`, `dateFrom: "2020-01-01"`. Cite `data_slice` with each figure. The keyword match is a substring match on the English title and abstract: check a sample of the area before you trust the count.
- `patstat_technology` (`cpc`: a prefix from `patstat_cpc`) gives family and application counts, top applicants by family count, the filing trend, the grant rate by office and new entrants for one CPC prefix.

Without FlowLeap Data, run count probes of the Step 1 CQL: one probe for each year (add a publication-date range for that year, grammar from `get_search_syntax`) and one probe for each main office (`countries: ["<code>"]`). Each probe is `range: "1-1"`, `details: false`; read `total`.

Done when the report has a filing-trend table and a jurisdiction table, each with its basis and source.

## Step 4: Identify the key players

Limit the Step 1 query to each applicant with `pa=`. A wildcard catches name variants (`pa=GOOGLE*` catches "Google LLC" and "Google Inc"), and subsidiaries file under their own names (Google also files as Alphabet, DeepMind, Waymo): `search_patents` (`provider: "epo_ops"`, `query: "pa=<ASSIGNEE>* AND <discriminating terms from Step 1>"`, `range: "1-30"`).

With FlowLeap Data, `patstat_resolve` (`q`: the applicant name) gives the harmonized entities, and `patstat_applicant` (`psn_id` from that answer) gives the applicant's filings by year, top CPC areas, jurisdictions and co-applicants. The harmonized name grouping is inferred, not a legal fact: say so.

When you count players from search hits, count one entry for each patent **family** (`get_family`), so that filings in several offices are not counted more than once.

## Step 5: Check recent activity

Add a publication-date limit to the Step 1 CQL, for example `<discriminating terms> AND pd>=2024` (grammar from `get_search_syntax`), and call `search_patents` (`provider: "epo_ops"`, `range: "1-50"`). For the USPTO, call `search_patents` with `provider: "uspto"` and `sort: "applicationMetaData.filingDate desc"`.

## Step 6: Flag white space

Compare the CPC breakdown with the filing-year trend. Flag the subclasses or main groups that are sparse or declining while the adjacent ones grow. With FlowLeap Data, use the CPC breakdown of `patent_analytics` with a date range for each period, or the filing trend of `patstat_technology` for each prefix. Without it, run count probes of the Step 1 CQL for each candidate CPC prefix (`cpc=<prefix>` in the CQL) and each period; limit this to the few prefixes that Step 3 names.

Done when at least one sparse or declining CPC subclass or main group is identified, or its absence is confirmed with the counts that show it.

## Output

A dataset by office, applicant and filing date, with the filing-trend table, the top-filers table, the jurisdiction split and the white-space notes. Each figure has its counting basis and its source. Corpus figures from `patent_analytics` and `patstat_*` are reported as returned; counts from search hits are counted by family. Name each office not searched and each tool that was not available.

In the FlowLeap Patent AI app, save the report with `write_patent_results` (`template: "landscape-report"`, `subject`: the technology area, `searchStrategy`: the queries, offices and tools used, `content`: the trend, top-filer and jurisdiction tables and the white-space notes, each figure with its counting basis and source). Elsewhere, write the report as a Markdown file.

## Visual deliverable

To show the filing trends of Step 3 and the white space of Step 6 as a dashboard, finish this analysis, then use `recipe-custom-dashboard`: its **landscape white-space** template (CPC against year) is made for Step 6, and its **filing-trends** template shows the counts of Step 3. The analysis stays in this recipe; the dashboard recipe only presents it.
