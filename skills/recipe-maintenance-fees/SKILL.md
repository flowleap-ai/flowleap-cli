---
name: recipe-maintenance-fees
description: Check US patent maintenance-fee status and deadlines — compute the 3.5/7.5/11.5-year windows from the grant date, read which fees were actually paid from USPTO transaction events, and report the next docketable deadline with surcharge dates. Trigger when the user asks whether maintenance fees are due or paid, when the next fee deadline is, whether a US patent is still in force fee-wise, or for a fee-status sweep over a portfolio. Dates and status only — never fee amounts.
metadata:
  requires:
    skills: ["flowleap-shared", "flowleap-uspto", "flowleap-legal"]
---

# Recipe: Maintenance-Fee Check

Compute maintenance-fee deadlines for US utility patents and report what is
paid, due, and docketable — from official USPTO data, never from memory.

In a chat client with the FlowLeap connector, call the tools named in the `flowleap-shared` connector table instead of these commands. The steps stay the same.

**Scope guard — dates and status only.** Fee *amounts* depend on the current
USPTO fee schedule and entity status and change over time. Never state dollar
amounts; link to https://www.uspto.gov/learning-and-resources/fees-and-payment
and report the entity status found on the record so the user can look up the
right column.

## Step 1: Applicability guard (run before any computation)

```bash
flowleap --json uspto grant <patent-number>        # or: uspto application <app-number>
```

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

```bash
flowleap --json uspto transactions <app-number>
```

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

Ground the rule text rather than asserting it (37 CFR 1.362, MPEP Chapter
2500):

```bash
flowleap --json legal search "maintenance fee due dates surcharge" --jurisdiction uspto
```

If all three windows are past, report historical status only.

## Step 4: Report

Per patent, a three-row table — Window | Due | Surcharge until | Status —
where Status is one of: paid on `<date>` (event `<code>`) / upcoming /
**in surcharge period** / no payment event recorded / lapsed on `<date>`.
Then exactly one action line:

> **Next action: docket `<the nearest unmet deadline>`**

**Portfolio mode:** given a list of patents (or hits from
`flowleap uspto search`), run the same check per patent and present one
summary table sorted by next deadline, nearest first.

Every response ends with the verification footer:

> Computed from USPTO ODP transaction records as of `<retrieval date>` —
> verify in USPTO Patent Center before docketing. Fee amounts: USPTO fee
> schedule (entity status on record: `<status>`).

## Related

- Full prosecution history: `flowleap uspto transactions` (see
  `flowleap-uspto`); single-patent dossier: `recipe-patent-to-report`.
- Official PTA day counts (term, not fees): `flowleap uspto adjustment`.
