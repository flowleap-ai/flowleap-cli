---
name: recipe-office-action-response
description: Prosecution recipe for turning an office action into a structured draft response — OCR the OA, pull every cited reference's claims and bibliography, map rejections element-by-element, and ground arguments in MPEP/EPO guideline citations. Trigger when the user asks to respond to an office action, analyze examiner rejections, or prepare arguments against novelty/obviousness (102/103, Art. 54/56) objections.
metadata:
  requires:
    skills: ["flowleap-shared", "flowleap-ops", "flowleap-legal", "flowleap-citation", "flowleap-uspto"]
---

# Recipe: Office Action Response

Turn an office action (OA) into a structured, evidence-backed draft response.

## Step 1: Intake — Read the Office Action

For a US application, fetch the OA straight from the USPTO file wrapper — no
local PDF needed. List the office actions, then pull the text (OCR'd
server-side):

```bash
flowleap --json uspto documents <application-number> --code CTNF   # non-final rejections
flowleap --json uspto documents <application-number> --code CTFR   # final rejections
flowleap uspto document-text <application-number> <documentIdentifier> > office-action.md
```

If the OA was supplied as a local file instead (or is not a US application):

```bash
flowleap ocr ./office-action.pdf > office-action.md
```

From the extracted text, record:
- Application number and examiner
- Response deadline
- Each rejection: statutory ground (e.g. 35 U.S.C. 102/103, EPC Art. 54/56),
  the claims it covers, and the references it relies on
- Any objections (formalities, claim clarity) separate from rejections

## Step 2: Pull Every Cited Reference

For each cited patent reference:

```bash
flowleap --json summary <cited-patent>       # biblio + legal status + family
flowleap --json ops claims <cited-patent>
flowleap --json ops abstract <cited-patent>
```

Verify the reference's effective date actually predates your priority date —
date-disqualified references are a complete answer to a rejection.

## Step 3: Examiner Citation Context

```bash
flowleap --json citation search <application-number> --examiner-cited-only
flowleap --json citation novelty <application-number>    # X-category refs
flowleap --json citation stats <application-number>
```

This shows how the examiner has used each reference (X = novelty-destroying,
Y = obviousness combination, A = background) across the prosecution.

## Step 4: Decompose the Rejected Claims

The as-rejected claim text is usually AMENDED and exists only in the file
wrapper — not in the published application. Pull the claims document (code
CLM) dated nearest before the OA (the OA's "Responsive to communication(s)
filed on <date>" line names the filing):

```bash
flowleap --json uspto documents <application-number> --code CLM
flowleap uspto document-text <application-number> <documentIdentifier> > claims-as-rejected.md
```

Decompose each rejected independent claim yourself: split on the
`comprising` transition and the semicolon/`wherein` boundaries, keep the claim
language verbatim per element (the method is in `recipe-claim-analysis`,
Step 4).

Build a mapping table per rejection: claim element → where the examiner says
the reference discloses it → what the reference actually says (quote it).
The argument lives where the mapping breaks.

## Step 5: Ground the Arguments in Law

```bash
# US obviousness framework
flowleap --json legal search "obviousness motivation to combine rationale" --jurisdiction uspto --comprehensive

# EPO inventive step framework
flowleap --json legal search "inventive step problem solution approach" --jurisdiction epo --comprehensive

# Claim amendments support
flowleap --json legal search "written description support for amendments" --jurisdiction uspto
```

Cite `section` + `source_url` from each result next to the argument it backs.

## Step 6: Draft the Response Shell

Per rejection:
1. Restate the rejection (ground, claims, references)
2. Element-mapping table with the disputed elements highlighted
3. Argument: missing element / improper combination / date disqualification,
   with reference quotes and legal citations
4. Amendment option: narrowing language sourced from the description, with a
   support citation

## Output

- OA summary (grounds, claims, references, deadline)
- Per-reference data package with date verification
- Element-mapping tables per rejection
- Argument shells with MPEP/EPO guideline citations
- Amendment options with written-description support
