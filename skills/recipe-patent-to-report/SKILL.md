---
name: recipe-patent-to-report
description: Formatted single-patent dossier — pull bibliography, abstract, claims, description, family, legal status, prosecution timeline, figures, and related art into one structured Markdown report. Trigger when the user asks for a complete profile, dossier, or report on a specific patent.
metadata:
  requires:
    skills: ["flowleap-shared", "flowleap-ops", "flowleap-patent"]
---

# Recipe: Patent to Report

Extract all data from a patent document and assemble it into one structured
report.

In a chat client with the FlowLeap connector, call the tools named in the `flowleap-shared` connector table instead of these commands. The steps stay the same.

## Steps

### Step 1: One-Call Snapshot

```bash
flowleap --json summary <patent-number>    # biblio + legal status + family + term
flowleap --json timeline <patent-number>   # chronological prosecution events
```

### Step 2: Gather Full Text

```bash
flowleap --json ops abstract <patent-number>
flowleap --json ops claims <patent-number>
flowleap --json ops description <patent-number>
```

### Step 3: Figures

```bash
flowleap figures <patent-number>                          # figure metadata
flowleap figures <patent-number> --out figure.png --page 3  # save one page
```

### Step 4: Find Related Patents

```bash
flowleap --json patent search --query "<key terms from abstract>" --limit 10
```

## Output

A Markdown dossier for the patent with one section per heading below. Done when
every section is present (or explicitly marked "not available"):

- **Bibliography** — title, applicant/inventor, filing and publication dates, classification
- **Abstract**
- **Claims** — independent and dependent
- **Description summary** — condensed from the full description
- **Figures** — figure list, with saved pages referenced
- **Family** — members by jurisdiction
- **Legal status** — active/expired/abandoned and estimated remaining term
- **Prosecution timeline** — register + INPADOC legal events
- **Related art** — patents surfaced from the abstract key terms
