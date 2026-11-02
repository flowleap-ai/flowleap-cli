---
name: recipe-maintenance-fees
description: "Check the maintenance-fee status and deadlines of US patents: the 3.5, 7.5 and 11.5-year windows from the grant date, the fees actually paid according to the USPTO transaction events, and the next deadline with its surcharge dates. Use it when the user asks whether maintenance fees are due or paid, when the next fee is due, whether a US patent is still in force for fees, or for a fee check across a portfolio. Dates and status only, never fee amounts."
metadata:
  requires:
    skills: ["recipe-patent-to-report"]
---

# Recipe: Maintenance-Fee Check

**Tools.** This method calls the FlowLeap patent tools by name. In the FlowLeap Patent AI app the office tools are built in; `patent_analytics`, `get_fulltext` and the `patstat_*` tools are FlowLeap Data tools, named `mcp__flowleap__<name>`, which exist only when the firm or the user has FlowLeap Data. With the FlowLeap connector in another client, call the connector tool of the same name. When `get_claims` answers that it has no US source, call the tool that its answer names.

**Missing keys.** When a tool answers that an office's key is not configured or that the office rejected the stored key, that office is a user-action stop, never an exhausted route: finish the work with the other offices, name the gap in the result as a missing-key gap (not as "no results"), never fill it with data from the web, and at the end tell the user which key is missing or rejected. On the desktop app the user adds it in Settings > Patent data connectors; on the firm OS the firm administrator sets the keys, so tell the attorney to ask the administrator; in another client, relay the page that the tool's answer names. Never ask for a key value in the chat.

Compute maintenance-fee deadlines for US utility patents and report what is
paid, due, and docketable, from official USPTO data, never from memory.

**Scope guard — dates and status only.** Fee *amounts* depend on the current
USPTO fee schedule and entity status and change over time. Never state dollar
amounts; link to https://www.uspto.gov/learning-and-resources/fees-and-payment
and report the entity status found on the record so the user can look up the
right column.

## Step 1: Applicability guard (run before any computation)

Call `get_us_grant` (`patent_number`), or `get_us_application` (`application_number`) when you have the application number.

From `applicationMetaData`, check the application type **first**:

- **Design or plant patent** → answer "this patent type has no maintenance
  fees" and stop. Utility patents only.
- **Reissue** → out of scope for computation: maintenance fees follow the
  ORIGINAL patent's schedule (MPEP 2504). Say so, point at the original
  patent, and stop.
- **Utility** → record `grantDate`, `applicationNumberText`, and
  `entityStatusData` (large/small/micro — determines which fee column
  applies), then continue.

No grant date (pre-grant application) → there is nothing to compute; say so.

## Step 2: Read what was actually paid

Call `get_transactions` (`application_number`): the events of `eventDataBag`, each with its code, description and date.

Scan the events for:

- **Payment events** — codes `M1551`/`M1552`/`M1553` (4th/8th/12th-year,
  large entity) and `M2551`/`M2552`/`M2553` (small/micro variants). Record
  the event date per window.
- **Expiry events** — descriptions like "Patent Expired for Failure to Pay
  Maintenance Fees" (code starting `EXP`). An expiry event ends the analysis:
  report the lapse and its date.

**Required phrasing:** a window with no payment event is "no payment event
recorded" — never "unpaid." ODP ingestion lags; a fee paid recently may not
appear yet.

## Step 3: Compute the windows

All dates run from **grant date** (PTA does not move them):

| Window | Opens | Due | Surcharge until | Expires if unpaid |
|--------|-------|-----|-----------------|-------------------|
| 4th-year | grant + 3y | grant + 3.5y | grant + 4y | grant + 4y |
| 8th-year | grant + 7y | grant + 7.5y | grant + 8y | grant + 8y |
| 12th-year | grant + 11y | grant + 11.5y | grant + 12y | grant + 12y |

Ground the rule text rather than asserting it: read 37 CFR 1.362 and MPEP
Chapter 2500 from the office's own page with your web tool, and quote the
rule with its URL and edition. The FlowLeap Patent AI app has no web tool:
there, write the rule as a "check: 37 CFR 1.362" item and write that the page
was not read.

If all three windows are past, report historical status only.

## Step 4: Report

Per patent, a three-row table — Window | Due | Surcharge until | Status —
where Status is one of: paid on `<date>` (event `<code>`) / upcoming /
**in surcharge period** / no payment event recorded / lapsed on `<date>`.
Then exactly one action line:

> **Next action: docket `<the nearest unmet deadline>`**

**Portfolio mode:** given a list of patents, the hits of `search_patents`
(`provider: "uspto"`), or the applications of
`search_uspto_portfolio_by_customer_number` (`customer_number`), run the same
check per patent and present one summary table sorted by next deadline,
nearest first.

Every response ends with the verification footer:

> Computed from USPTO ODP transaction records as of `<retrieval date>` —
> verify in USPTO Patent Center before docketing. Fee amounts: USPTO fee
> schedule (entity status on record: `<status>`).

When the user asks for a file, in the FlowLeap Patent AI app save the report
with `write_patent_results` (no template, `filePath`, `content`). Elsewhere,
write a Markdown file.

## Related

- Full prosecution history: `get_transactions`; single-patent dossier:
  `recipe-patent-to-report`.
- Official PTA day counts (term, not fees): `get_patent_term_adjustment`
  (`application_number`).
