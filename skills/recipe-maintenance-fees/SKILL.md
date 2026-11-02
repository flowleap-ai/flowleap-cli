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

Scan the events for these. Match on the description text; the codes are the ones seen in live records:

- **Payment events**: description "Payment of Maintenance Fee, <4th/8th/12th> Year, <Large/Small/Micro> Entity". Codes seen live: `M1552`, `M1553` (large), `M2552`, `M2553` (small), `M3551`, `M3552` (micro). Record the event date and code per window. Check any other code against its description text before you rely on it.
- **Surcharge events**: a late payment within the six-month grace period carries a surcharge event on the same date. Seen live: `M2556` ("11.5 yr surcharge- late pmt w/in 6 mo, Small Entity") and `M3555` ("Surcharge for Late Payment, Micro Entity"). Report the window as paid in the surcharge period.
- **Reminder**: `REM.` ("Maintenance Fee Reminder Mailed").
- **Expiry events**: code `EXP.`, description "Expire Patent". Report the lapse and its date.
- **Reinstatement**: an expiry is not final when a later event accepts the delayed payment. Seen live: `PMFP` ("Petition to Accept Late Payment of Maintenance Fee Payment Filed") and `PMFG` ("Petition Decision - Accept Late Payment of Maintenance Fees - Granted"). A record can hold `EXP.`, then `PMFG`, then a later `EXP.`: the latest expiry or reinstatement event decides the status. Any other event after `EXP.` (for example a code starting `EXPX`) is a check item: read its description, and do not report the patent as lapsed until you have.

**Required phrasing:** a window with no payment event is "no payment event
recorded" — never "unpaid." ODP ingestion lags; a fee paid recently may not
appear yet. Live records can also lack an earlier payment event while a later window is paid (for example `M2552` and `M2553` with no 4th-year event): report the earlier window as "no payment event recorded; a later window is paid, so this window did not cause a lapse" (take the current status from the latest window and the latest `EXP.` or `PMFG` event) and add "check: a late payment or a reinstatement of this window (37 CFR 1.378; intervening rights, 35 USC 41(c)(2)); not proof that the fee was paid on time".

## Step 3: Compute the windows

All dates run from **grant date** (PTA does not move them):

| Window | Opens | Due | Surcharge until | Expires if unpaid |
|--------|-------|-----|-----------------|-------------------|
| 4th-year | grant + 3y | grant + 3.5y | grant + 4y | grant + 4y |
| 8th-year | grant + 7y | grant + 7.5y | grant + 8y | grant + 8y |
| 12th-year | grant + 11y | grant + 11.5y | grant + 12y | grant + 12y |

A due date or a surcharge date that falls on a Saturday, Sunday or federal holiday in the District of Columbia moves to the next business day (35 USC 21(b)). Give the computed date and, when it moves, the docketed date.

Ground the rule text rather than asserting it: read 37 CFR 1.362 (the windows), 37 CFR 1.20(e) to (h) (the maintenance fees and the surcharge), 35 USC 21(b) and MPEP 2506 (times for payment), and, for a lapsed patent, 37 CFR 1.378 (petition to accept delayed payment) from the office's own page with your web tool, and quote each rule with its URL and edition. The FlowLeap Patent AI app has no web tool: there, write each rule as a check item ("check: 37 CFR 1.362", "check: 37 CFR 1.20(e) to (h)", "check: 35 USC 21(b) and MPEP 2506") and write that the page was not read.

If all three windows are past, report historical status only.

## Step 4: Report

Per patent, a three-row table — Window | Due | Surcharge until | Status —
where Status is one of: paid on `<date>` (event `<code>`) / upcoming /
**in surcharge period** / no payment event recorded / lapsed on `<date>` /
reinstated on `<date>` (event `<code>`). For a lapsed patent, add the item
"check: petition to accept delayed payment, 37 CFR 1.378".
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
