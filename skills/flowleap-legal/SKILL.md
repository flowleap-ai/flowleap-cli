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

Tools-facade equivalents: `reference_search` (the search) and
`get_legal_jurisdictions` (the discovery call).

```bash
flowleap --json tools run reference_search query='inventive step' jurisdiction=EPO
```

Patent-law reference needs no patent-data key, so it stays live while EPO or
USPTO is key-gated. Offer it as *different* data — it gives the legal standard,
never what has been published or filed — and never as a substitute for a gated
office's search (see `flowleap-keys`).

`legal stats` and `legal docs` no longer exist: their endpoints retired with no
facade successor, so the subcommands were removed rather than left to answer
`410`.
