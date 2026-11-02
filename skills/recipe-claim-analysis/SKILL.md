---
name: recipe-claim-analysis
description: "Break a patent's claims into elements, keywords and search queries, with the claims, abstract, bibliography, description and family as context. Use it when the user asks what a patent claims, to break claims into elements, or to interpret claim scope in the light of the specification."
metadata:
  requires:
    skills: ["recipe-prior-art-search"]
---

# Recipe: Claim Analysis

**Tools.** This method calls the FlowLeap patent tools by name. In the FlowLeap Patent AI app the office tools are built in; `patent_analytics`, `get_fulltext` and the `patstat_*` tools are FlowLeap Data tools, named `mcp__flowleap__<name>`, which exist only when the firm or the user has FlowLeap Data. With the FlowLeap connector in another client, call the connector tool of the same name. When `get_claims` answers that it has no US source, call the tool that its answer names.

**Missing keys.** When a tool answers that an office's key is not configured or that the office rejected the stored key, that office is a user-action stop, never an exhausted route: finish the work with the other offices, name the gap in the result as a missing-key gap (not as "no results"), never fill it with data from the web, and at the end tell the user which key is missing or rejected. On the desktop app the user adds it in Settings > Patent data connectors; on the firm OS the firm administrator sets the keys, so tell the attorney to ask the administrator; in another client, relay the page that the tool's answer names. Never ask for a key value in the chat.

Read a patent's claims with full context, then decompose them into elements.

## Steps

### Step 1: Read the claims

Call `get_claims` (`patent_number`). Done when you have the full claim set and the independent claims are identified.

Office coverage: `get_claims` and `get_description` serve EP and WO; US claims come in English; US description is not served. For another office (CN, JP, KR, …) or a missing text, read a family member (`get_family`) in an office the tool serves, and label the text as the member's text.

### Step 2: Get context

- `get_abstract` (`patent_number`)
- `get_bibliography` (`patent_number`): title, applicants, inventors, IPC and CPC, dates.
- `get_description` (`patent_number`): the specification, the dictionary of the claims.

### Step 3: Check related patents

Call `get_family` (`patent_number`): the extended INPADOC family, with its members by office.

### Step 4: Decompose every independent claim yourself

No tool decomposes the claims for you: do it one independent claim at a time.

1. **Split the claim** into preamble, transition (`comprising` / `consisting of`) and body. Element boundaries fall at the semicolons and at `wherein` / `configured to` clauses.
2. **Keep the claim language verbatim per element.** Paraphrase in a second column if useful, but the verbatim text is what later maps against prior art. Tag each element structural or functional.
3. **Derive keywords and synonyms per element** from the element's own nouns. Check the description (Step 2) for the applicant's own alternative wording: the specification is the claim's dictionary. Name the passage each synonym comes from.
4. **Write a search query per key element combination.** Build each query with the search method of `recipe-prior-art-search` (`references/search-method.md`); `get_search_syntax` gives the field grammar of each provider. The element pairs, not single elements, usually carry the discrimination. Show each query as a Search Statement block, as that method says.

Repeat for every independent claim. For each dependent claim, note how it narrows its parent (the added element). Done when every independent claim has an element breakdown and each dependent claim's added limitation is recorded.

## Output

Complete claim data with supporting context, each part with the tool and the publication it came from:

- Full claims text (independent and dependent), with the member's number when it is a family member's text.
- Abstract, bibliographic data, and description for interpretation.
- Family members by office (the extended family).
- Element breakdown (verbatim language, structural or functional tag, keywords) and the search queries per independent claim, plus the narrowing limitation added by each dependent claim.
- Gaps: text not served, missing keys.

When the user asks for a file, in the FlowLeap Patent AI app save it with `write_patent_results` (no template, `filePath`, `content`). Elsewhere, write a Markdown file.
