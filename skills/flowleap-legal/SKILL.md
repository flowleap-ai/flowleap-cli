---
name: flowleap-legal
description: Hybrid semantic/keyword search over patent-law reference documents (EPC, EPO Guidelines, MPEP, EU and WIPO materials) with jurisdiction filters. Trigger when an agent needs legal grounds, examination-guideline citations, statute text, or authoritative references for patent-law questions.
---

# FlowLeap Legal Search (patent-law RAG)

Auth and global flags: see `flowleap-shared`.

## Search

```bash
flowleap --json legal search "inventive step problem solution approach" --jurisdiction epo --limit 5
flowleap --json legal search "101 abstract idea two-step" --jurisdiction uspto --search-mode hybrid
```

Flags: `--jurisdiction epo|uspto|eu|wipo|all`, `--search-mode hybrid|semantic|keyword`,
`--limit N`, `--include-context` (neighboring chunks), `--comprehensive`
(grouped full-section results — best for drafting).

Each result carries `source`, `section`, `chunk_text`, `source_url` and scores —
cite `section` + `source_url` in agent output.

## Discovery

```bash
flowleap --json legal jurisdictions   # available jurisdictions and sources
```

Also available as `flowleap tools run reference_search --input '{"query":"...","jurisdiction":"EPO"}'`.
