---
name: recipe-invention-disclosure
description: "Turn a conversation with an inventor into a complete invention disclosure form: structured technical capture, a bar-date check, a novelty check across patents and literature with the difference from each close hit recorded, landscape context and a filing recommendation. Use it when the user asks to document an invention, to create an invention disclosure, or to run a novelty check before filing."
metadata:
  requires:
    skills: ["recipe-prior-art-search"]
---

# Recipe: Invention Disclosure (IDF)

**Tools.** This method calls the FlowLeap patent tools by name. In the FlowLeap Patent AI app the office tools are built in; `patent_analytics`, `get_fulltext` and the `patstat_*` tools are FlowLeap Data tools, named `mcp__flowleap__<name>`, which exist only when the firm or the user has FlowLeap Data. With the FlowLeap connector in another client, call the connector tool of the same name. When `get_claims` answers that it has no US source, call the tool that its answer names.

**Missing keys.** When a tool answers that an office's key is missing or not configured, that office is a user-action stop, never an exhausted route: finish the work with the other offices, name the gap in the result as a missing-key gap (not as "no results"), never fill it with data from the web, and at the end tell the user which key is missing and where the tool's answer says to add it. Never ask for a key value in the chat.

Record an invention completely enough that an attorney can draft from it.

## Step 1: Structured capture

Interview for these sections. Ask past marketing language until you get the mechanisms:

1. **Problem**: what fails today, and why the known approaches are not sufficient.
2. **Prior approaches**: what the inventors know exists, including their own earlier work.
3. **The inventive concept**: the mechanism of the solution, not the benefit.
4. **Key elements**: the minimum set of elements that the invention needs to work.
5. **Variations**: alternative embodiments, ranges, materials, algorithms.
6. **Evidence**: prototypes, test data, figures.
7. **People and dates**: the inventors and the contribution of each; the conception date; **each public disclosure, sale, offer or publication, with its date**. Bar dates differ by jurisdiction: flag each event older than 12 months.

## Step 2: Novelty pre-check

Build each query with the search method of `recipe-prior-art-search` (`references/search-method.md`); `get_search_syntax` gives the field grammar of each provider.

The "key elements" list of Step 1 is the term extraction. Keep the phrases that discriminate (the mechanism, not the benefit). The queries hold only search terms, so the disclosure text itself goes to no search provider.

- Probe the count with `search_patents` (`provider: "epo_ops"`, `query: "<CQL>"`, `range: "1-1"`, `details: false`) and read `total`. Then search with `range: "1-20"`.
- Search the USPTO with `search_patents` (`provider: "uspto"`, `query: "<Lucene>"`, `limit: 20`). It searches US application titles and metadata, not claim text.
- Search the literature, where the inventors' own field often publishes first: `search_academic` (`query`: the concept keywords, `max_results: 15`) and `search_npl` (`query`: the concept keywords, `limit: 10`).

For each close patent hit, read the claims with `get_claims` (`patent_number`) and record how the invention differs, with the claim passage that the difference rests on. For a close paper, record the difference from its abstract and say that only the abstract was read.

Office coverage: `get_claims` and `get_description` serve EP and WO; US claims come in English; US description is not served. For another office (CN, JP, KR, …) or a missing text, read a family member (`get_family`) in an office the tool serves, and label the text as the member's text.

This is a sanity check, not a professional prior-art search: say so in the IDF. For a full search, use `recipe-prior-art-search`.

## Step 3: Landscape context

With FlowLeap Data, call `patent_analytics` (`keywords: ["<technology term>"]`, `dateFrom: "2018-01-01"`) for the filing trend and the top assignees, and cite its `data_slice`. Without it, give the `total` of the Step 2 count probe as a live search count and state that the corpus analytics were not available.

The trend and the top assignees tell the reviewer whether the area is crowded, growing or abandoned. This is context for the filing decision.

## Step 4: Assemble the IDF

Sections: title; inventors and contributions; problem; prior approaches (including the Step 2 findings with the differences recorded); detailed description of the inventive concept; key elements; variations; evidence; disclosure and bar-date audit; landscape summary.

## Step 5: Recommendation

Close with one of: **file now** (novelty check clean, bar clock running), **develop further** (concept not yet enabled), or **defensive publication** (crowded area, low claim scope), with the reasons.

## Output

A complete IDF document, with the raw novelty pre-check data as an appendix: each query with its tool and count, each hit reviewed, and each difference recorded with its source passage. Name each office or source that was not searched, with the reason.

In the FlowLeap Patent AI app, save the IDF with `write_patent_results` without a template (`filePath`, `content`: the IDF and its appendix). Elsewhere, write it as a Markdown file.
