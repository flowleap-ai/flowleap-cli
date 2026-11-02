---
name: recipe-claim-drafting
description: "Draft patent claims on the basis of the prior art: find the closest art, study its claim language, break the invention into old and novel elements, draft independent and dependent claim sets, check the formal rules of the EPC, the EPO Guidelines, 35 USC 112, 37 CFR 1.75 and the MPEP, and search again from each novel element until the closest art lacks one and that element is not obvious over the closest art combined with one other document. Use it when the user asks to draft or improve claims, to write independent and dependent claims, or to test draft claims against known art and formal requirements."
metadata:
  requires:
    skills: ["recipe-prior-art-search", "recipe-claim-analysis"]
---

# Recipe: Claim Drafting

**Tools.** This method calls the FlowLeap patent tools by name. In the FlowLeap Patent AI app the office tools are built in; `patent_analytics`, `get_fulltext` and the `patstat_*` tools are FlowLeap Data tools, named `mcp__flowleap__<name>`, which exist only when the firm or the user has FlowLeap Data. With the FlowLeap connector in another client, call the connector tool of the same name. When `get_claims` answers that it has no US source, call the tool that its answer names.

**Missing keys.** When a tool answers that an office's key is not configured or that the office rejected the stored key, that office is a user-action stop, never an exhausted route: finish the work with the other offices, name the gap in the result as a missing-key gap (not as "no results"), never fill it with data from the web, and at the end tell the user which key is missing or rejected. On the desktop app the user adds it in Settings > Patent data connectors; on the firm OS the firm administrator sets the keys, so tell the attorney to ask the administrator; in another client, relay the page that the tool's answer names. Never ask for a key value in the chat.

Draft claims that are novel over the closest art and formally correct. The claims are drafts for the attorney.

## Step 1: Find the closest art first

Claims drafted without the art get rejected. Find the art before you write.

Build each query with the search method of `recipe-prior-art-search` (`references/search-method.md`); `get_search_syntax` gives the field grammar of each provider.

Tune the queries for precision: the goal is the few closest documents to read in full. Add a second discriminating `ta` term and reduce the count toward tens, not thousands.

- Probe the count with `search_patents` (`provider: "epo_ops"`, `query: "<CQL>"`, `range: "1-1"`, `details: false`) and read `total`. Then search with `range: "1-20"`.
- Search the USPTO with `search_patents` (`provider: "uspto"`, `query: "<Lucene>"`, `limit: 20`). It searches US application titles and metadata, not claim text.

Read the claims of the 3 to 5 closest hits with `get_claims` (`patent_number`) and their abstracts with `get_abstract` (`patent_number`), and study their claim language. Quote claim 1 of each from the tool result. Prior art is the whole disclosure, not only the claims: for EP and WO hits, also read the description with `get_description` (`patent_number`) before you decide that a hit lacks an element. For a hit whose description is not served, say that only the claims and abstract were read.

Office coverage: `get_claims` and `get_description` serve EP and WO; US claims come in English; US description is not served. For another office (CN, JP, KR, …) or a missing text, read a family member (`get_family`) in an office the tool serves, and label the text as the member's text.

## Step 2: Element inventory of the invention

Write a one-paragraph summary of the invention, then break it into elements: list each element that the invention needs to work (the mechanism, not the benefit), keep the language concrete, and note keywords and synonyms for each element. The method is in `recipe-claim-analysis`, Step 4.

Sort the elements: which are old (in the closest art, with the document and passage), and which are the point of novelty. The independent claim needs the old elements for operability and at least one novel element for allowance.

## Step 3: Draft the claim set

- **Independent claim**: preamble (technical field), transition (`comprising`), and the minimum set of elements that is both operable and novel.
- **Dependent claims**: one fallback feature each, in order of commercial value. They are the amendment reserve for prosecution.
- Draft parallel method and apparatus sets, and a computer-readable medium set where it applies.

## Step 4: Check the formal drafting rules

The rules: EPC Art. 84 and Guidelines F-IV (clarity, conciseness, support); EPC Rule 43 (two-part form where appropriate, Rule 43(1); one independent claim per category, Rule 43(2), with its exceptions); 35 USC 112 and MPEP 2173 (definiteness); MPEP 2181 (means-plus-function); 37 CFR 1.75 (claim form, dependent and multiple dependent claims).

No FlowLeap tool reads these rule pages, and the FlowLeap Patent AI app has no web tool. In the app, check each claim against the rules as you know them, write each rule as "check: <citation>" for the attorney, and state that the rule page was not read in this run. In a client with a tool that reads web pages, read each rule from the office's own page and quote it with its URL and edition.

Check each claim for antecedent basis, single-sentence form, no result-only language without structure, and support in the description.

## Step 5: Iterate against the art

For each draft independent claim, write a search from its novel element in the same way as Step 1; the phrase of the novel element is the discriminating term. Call `search_patents` (`provider: "epo_ops"`, `query: "<CQL from the novel element>"`, `range: "1-10"`).

If the searches find art that reads on the draft, narrow the novel element and search again. Novelty is not enough: check obviousness too (EPC Art. 56; 35 USC 103). Take the closest document, find the element it lacks, and ask whether one other document found in Steps 1 or 5 discloses that element for the same technical purpose, so that a skilled person would combine the two. If one does, narrow the claim and search again.

Stop when the closest art lacks an element and no other document found supplies it. Name the element, the closest document, and each second document you checked.

## Output

- Closest-art table: reference, claim 1 language (quoted from `get_claims`), the element it lacks, and whether its description was read.
- Obviousness check: for each independent claim, the closest document, the second documents checked, and why none supplies the missing element.
- Element inventory, split into old and novel elements.
- The full claim set (independent and dependent claims) with a support map into the description.
- Formal-rules checklist with the citation of each rule, and for each rule whether its page was read or it is a check item.
- The queries and their counts.

In the FlowLeap Patent AI app, save the drafts with `write_patent_results` without a template (`filePath`, `content`: the claim set and its tables). Elsewhere, write them as a Markdown file.
