---
name: recipe-audit-report
description: "Produce an auditable record of AI-assisted patent research: a reproducible log of each tool call with its exact input and saved answer, the data source of every finding, a dated check of each patent's status, a re-run of the load-bearing calls, and an AI-use disclosure for internal or filing-related records. Use it when the user asks for an audit trail of patent research, a disclosure of AI assistance, or a verifiable write-up of the method."
metadata:
  requires:
    skills: ["recipe-custom-dashboard"]
---

# Recipe: Audit Report (AI-Assisted Research)

**Tools.** This method calls the FlowLeap patent tools by name. In the FlowLeap Patent AI app the office tools are built in; `patent_analytics`, `get_fulltext` and the `patstat_*` tools are FlowLeap Data tools, named `mcp__flowleap__<name>`, which exist only when the firm or the user has FlowLeap Data. With the FlowLeap connector in another client, call the connector tool of the same name. When `get_claims` answers that it has no US source, call the tool that its answer names.

**Missing keys.** When a tool answers that an office's key is not configured or that the office rejected the stored key, that office is a user-action stop, never an exhausted route: finish the work with the other offices, name the gap in the result as a missing-key gap (not as "no results"), never fill it with data from the web, and at the end tell the user which key is missing or rejected. On the desktop app the user adds it in Settings > Patent data connectors; on the firm OS the firm administrator sets the keys, so tell the attorney to ask the administrator; in another client, relay the page that the tool's answer names. Never ask for a key value in the chat.

Make AI-assisted patent research verifiable: every finding traceable to a tool call, every tool call repeatable with its exact input.

## Step 1: Record the Environment

Record:

- The date and time of the session, and its time zone.
- The host: the FlowLeap Patent AI app, or the client with the FlowLeap connector.
- The model that did the work, as the host names it.
- The tools available that day: the office tools, and whether the FlowLeap Data tools (`patstat_*`, `patent_analytics`, `get_fulltext`) exist in the session.
- When FlowLeap Data is available, the answer of `server_info` (no input): the backend identity, its version and the configured providers.
- Each office whose tool answered that its key is missing or not configured, as a missing-key gap.

Never record key values or other credential material.

## Step 2: Provenance Discipline During Research

For every substantive finding, log the tool name and its exact input, and keep the answer. Save each answer unchanged as a JSON file in the workspace (for example `audit/<nn>-<tool>.json`), right after the call, by the method of `recipe-custom-dashboard` rule 2: copy the stored file when the answer says the full result is stored in one, otherwise write the answer text exactly as the tool returned it. Never edit a saved answer.

Note per call:

- the source system: EPO OPS, EP Register, USPTO ODP, OpenAlex, Semantic Scholar, arXiv, PATSTAT, or the Google Patents slice of `patent_analytics`;
- the query string, the limits (`limit`, `size`, `offset`, `range`) and the date filters;
- the dataset identity where the answer carries one (`data_edition` of the `patstat_*` tools, `data_slice` of `patent_analytics`);
- for `search_patents`, the answer's `total` and `effectiveQuery` (the query the provider received);
- for `search_academic`, the status of each source (`ok`, `rate_limited`, `error`): a failed source is a gap, not a zero.

The office tools can serve an answer from a cache (for example USPTO ODP lookups for up to 24 hours), so a recent register change can be missing: Step 4 re-runs the calls that matter.

## Step 3: Verify Asset Status Claims

Back each statement like "patent X is in force" with a dated check, and give the event dates from the record:

- `get_patent_summary` (`patent_number`): legal status, family and estimated term in one call.
- `get_prosecution_timeline` (`patent_number`): the events behind the status (EP register events merged with INPADOC legal events).
- For an EP publication, `get_register_events` (`patent_number`) and `get_register_biblio` (`patent_number`) for the EP Register status.
- For a US patent, `get_us_grant` (`patent_number`), then `get_transactions` (`application_number`) for the issue and fee events.

`get_patent_term` estimates 20 years from filing without term adjustments, extensions or lapse: label it as an estimate, and use `get_patent_term_adjustment` (`application_number`) for the official US adjustment.

## Step 4: Reproducibility Pass

At report time, call each load-bearing tool again with the same input and save the answer beside the first one (for example `audit/<nn>-<tool>.recheck.json`). Compare the two files in the shell (`diff`) or read both. Patent registers move: note each finding that changed between research and reporting, with both dates.

## Step 5: AI-Usage Disclosure Section

State plainly:

- Which steps were AI-assisted (search strategy, summarization, charting) and which tool or model performed them.
- Which outputs a human verified, and how (for example claims read in full, legal-status events checked against the register).
- Known limitations: search recall is not exhaustive; OCR and summarization can introduce errors; results reflect database coverage on the query date; the missing-key gaps of Step 1.

For filings, the current duty-of-disclosure guidance is on the office's own page (37 CFR 1.56, MPEP 2001). The FlowLeap Patent AI app has no web tool: state that the page was not read, cite the two references, and tell the user to read the current text on the USPTO page. In a client with a web tool, read the page and quote it with its URL and edition.

## Visual deliverable

If the research being audited produced a `recipe-custom-dashboard` bundle, cite it rather than re-deriving its numbers: its provenance footer and reproduce block already list each tool call, its input, its dataset identity and its saved answer, so Steps 2 and 4 can point at that bundle. Re-run its tool calls for Step 4 and run `node generate.mjs` again on the new answers.

In the FlowLeap Patent AI app, each report saved with a template through `write_patent_results` carries a generated provenance section that lists its recorded tool calls: cite it for that report.

## Output

- Environment record (date, host, model, tools available, `server_info` answer, missing-key gaps)
- Methodology section (sources, queries, filters, dataset identities, per finding)
- Status-verification table with check dates and record dates
- Call appendix: every tool call, in order, with its input and the file of its saved answer
- Reproducibility notes: the findings that changed, with both dates
- AI-usage disclosure with human-verification points and limitations

In the FlowLeap Patent AI app, save the report with `write_patent_results` (no `template`: a free-form document, with `filePath` and `content`). Elsewhere the report is a Markdown file.
