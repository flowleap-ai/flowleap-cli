<!-- BEGIN FLOWLEAP AGENT RULES (flowleap-cli v0.0.0-test) -->
## FlowLeap CLI — Agent Rules

Rendered by flowleap-cli v0.0.0-test from its bundled agent skills. Refresh after a CLI upgrade with `flowleap skills update`.

FlowLeap is a CLI for the FlowLeap Patent AI backend: patent/USPTO/OPS/academic/NPL/legal/citation search plus an agent-first tools facade.

- Add `--json` to every command for stable machine-readable output.
- Run `flowleap --json doctor` first to verify config, auth, and backend reachability.
- Authenticate with `flowleap auth login`, or set `FLOWLEAP_API_KEY`/`FLOWLEAP_TOKEN` for headless use.
- If an error carries a `providerKeysHint` (`provider_keys_required` / `provider_keys_invalid`), that office is gated on a patent-data key only the user can add (free from the office). It is a user-action stop, never an exhausted route: do not retry, do not invent keys, and do not substitute web-scraped patent data for that office — searches or single-document reads. Ask the user to run `flowleap setup`.
- With one office gated and the other live, deliver the live office's results in full, name the gap as a missing-key gap (never as limited coverage), never narrow a prior-art or FTO scope to the keyed office, and ask for the key at the end. `patstat`, `legal`, `academic`, and `npl` stay live keyless — offer them as different data, not as a substitute. Only that explicit error means gated: empty results, truncated payloads, and 5xx keep the normal retries.
- Discover every backend tool with `flowleap tools list`; run one with `flowleap tools run <name>`.

### Command Reference

#### flowleap — Start here — the umbrella skill for the FlowLeap Patent AI CLI.

```bash
flowleap --json doctor
flowleap skills install
flowleap upgrade --check
flowleap skills update
```

#### flowleap-academic — Search academic literature (Semantic Scholar + arXiv) through the FlowLeap backend with per-source and publication-year filters.

```bash
flowleap academic search <query> [flags]
```

#### flowleap-auth — Authenticate the FlowLeap CLI — OAuth 2.0 device flow login (user code + verification URL), long-lived fl_pat_ personal API tokens for headless agents, status checks, and targeted logout.

```bash
flowleap auth login
flowleap auth create-token --name my-agent --store
flowleap auth tokens
flowleap auth revoke-token <id>
flowleap auth status
flowleap auth logout
```

#### flowleap-citation — USPTO enriched citation data from office actions — citations by application, forward citations of a document, citation statistics and X-category novelty-destroying references.

```bash
flowleap --json citation search 16123456 --size 20
flowleap --json citation forward US10123456 --size 20
flowleap --json citation stats 16123456
flowleap --json citation novelty 16123456
```

#### flowleap-keys — Manage BYOK patent-data keys (EPO OPS consumer key/secret, USPTO ODP API key) for the FlowLeap CLI — check status, validate live, hand off to a human for the interactive setup wizard, and apply the key-gate doctrine (a gated office is a user-action stop, never an exhausted route).

```bash
flowleap --json keys list
flowleap --json keys test
flowleap --json doctor
flowleap --json keys set epo --key <consumer-key> --secret <consumer-secret>
```

#### flowleap-legal — Hybrid semantic/keyword search over patent-law reference documents (EPC, EPO Guidelines, MPEP, EU and WIPO materials) with jurisdiction filters.

```bash
flowleap --json legal search "inventive step problem solution approach" --jurisdiction epo --limit 5
flowleap --json legal jurisdictions
flowleap --json legal stats
flowleap --json legal docs
```

#### flowleap-npl — Search non-patent literature (scholarly works via OpenAlex) with year, open-access and publication-type filters.

```bash
flowleap --json npl "perovskite solar cell stability" --limit 5
flowleap --json npl "CRISPR delivery" --from-year 2020 --to-year 2024 --open-access
flowleap --json npl "transformer attention" --type preprint
```

#### flowleap-ops — Direct EPO Open Patent Services access through the FlowLeap backend — CQL search plus per-document bibliography, claims, description, family, legal status, and abstract.

```bash
flowleap ops search --cql <query> [flags]
flowleap ops biblio <doc>
flowleap ops claims <doc> --lang en
flowleap ops description <doc> --lang en
flowleap ops family <doc>
flowleap ops legal <doc>
flowleap ops abstract <doc>
flowleap convert-number US5443036.A --to docdb
```

#### flowleap-patent — Search EPO patents with CQL you write yourself — field reference, the discriminating-term rule, and the mandatory count probe.

```bash
flowleap patent search --query <query> [flags]
flowleap --json api request post /v1/patent-search --body '{"query":"ta=\"foreign object\" AND ta=charging","range":"1-1"}'
flowleap patstat query "SELECT symbol, title FROM flowleap.cpc_scheme WHERE title ILIKE '%photovoltaic%' ORDER BY symbol LIMIT 15" --question "candidate CPC codes for photovoltaics"
```

#### flowleap-patstat — Portfolio Analytics AND guarded SQL over the PATSTAT snapshot — structured-criteria aggregation by named applicant, CPC/IPC class, office, year, family, and grant status, with harmonized entity resolution and Data Edition provenance; plus agent-written SELECTs against the flowleap.* semantic views for any aggregate the typed commands don't cover (landscapes, grant rates, citation impact, inventor analytics).

```bash
flowleap --json patstat portfolio "Siemens AG" --from-year 2015 --to-year 2023
flowleap patstat docs --section examples
flowleap patstat query "SELECT office, COUNT(DISTINCT family_id) AS inventions FROM flowleap.applications a JOIN flowleap.applicants ap ON ap.application_id = a.application_id WHERE UPPER(ap.name) LIKE 'SIEMENS%' GROUP BY office ORDER BY inventions DESC" --question "where does Siemens hold the most inventions?"
flowleap --json tools run patstat_portfolio applicant="<applicant name>"
```

#### flowleap-patstat-graph — Graph Analytics over the PATSTAT snapshot — a named node and the relationships around it.

```bash
flowleap patstat graph resolve EP3477840
```

#### flowleap-shared — Shared reference for every FlowLeap skill — authentication (OAuth device flow, fl_pat_ personal API tokens), credential storage, config precedence, global flags, and output formats.

```bash
flowleap config set base-url https://api.flowleap.co
flowleap config get base-url
flowleap upgrade --check --json
```

#### flowleap-tools — Discover and run FlowLeap backend tools through the agent-first /v1/tools facade — one stable contract (search_patents, get_bibliography, get_claims, get_patent_summary, compare_patents, reference_search, …) instead of provider-specific endpoints.

```bash
flowleap --json tools list
flowleap --json tools describe get_bibliography
flowleap --json tools openapi
flowleap --json tools run get_bibliography patent_number=EP1000000
```

#### flowleap-uspto — Search USPTO Open Data Portal records with Lucene queries you write yourself, fetch granted patents, applications, continuity chains, and file-wrapper data (prosecution transactions, assignments, foreign priority, PTA, attorney of record), and list IFW documents and read office actions as OCR-extracted text.

```bash
flowleap --json uspto search --query 'applicationMetaData.inventionTitle:"machine learning"' --limit 5
flowleap --json api request post /v1/patent-search-uspto/search --body '{"q":"applicationMetaData.inventionTitle:\"charging case\"","pagination":{"limit":1,"offset":0}}'
flowleap --json uspto grant 11800000
flowleap --json uspto application 16123456
flowleap --json uspto continuity 16123456
flowleap --json uspto transactions 14412875
flowleap --json uspto assignments 14412875
flowleap --json uspto foreign-priority 14412875
```

### Workflow Triggers

- **persona-ip-analyst** — IP-analyst persona for the FlowLeap CLI — technology-landscape mapping, portfolio assessment, white-space identification, and filing-trend analytics. Trigger when the user asks the agent to act as an IP analyst, map a landscape, assess a competitor portfolio, or report patent filing trends.
- **persona-patent-attorney** — Patent-attorney persona for the FlowLeap CLI — prior-art search, claim-scope analysis, and freedom-to-operate clearance. Trigger when the user asks the agent to act as a patent attorney, review prior art, analyze claim scope, or clear FTO for a product.
- **persona-researcher** — Researcher persona for the FlowLeap CLI — parallel academic-literature and patent exploration to map what is published versus what is protected. Trigger when the user asks the agent to act as a researcher, survey a technology across papers and patents, or find gaps between academic work and filed IP.
- **persona-startup-founder** — Startup-founder persona for the FlowLeap CLI — novelty sanity checks, freedom-to-operate scans, and competitor-patent monitoring. Trigger when the user asks the agent to validate a startup's IP position, check whether an idea is already patented, or scope competitor patents before building.
- **recipe-academic-literature-review** — Map the gap between published research and filed patents — a scholarly sweep across Semantic Scholar, arXiv, and OpenAlex aligned against a matching patent search, output centered on the published-versus-protected gap rather than a ranked novelty list. Trigger when the user asks for a literature review, a state-of-the-art survey, or a comparison of academic research against patents.
- **recipe-audit-report** — Governance recipe for producing an auditable record of AI-assisted patent research — reproducible command log, data provenance for every finding, portfolio status verification, and an AI-usage disclosure section suitable for internal or filing-adjacent records. Trigger when the user asks for an audit trail of patent research, an AI-assistance disclosure, or a verifiable methodology write-up of analysis work.
- **recipe-claim-analysis** — Decompose a patent's claims into elements, keywords, and search queries with full supporting context — claims text, abstract, bibliography, description, and family. Trigger when the user asks to analyze what a patent claims, break claims into elements, or interpret claim scope against its specification.
- **recipe-claim-drafting** — Prosecution recipe for drafting patent claims grounded in prior art — search the closest art, study its claim language, decompose the invention into elements, iterate drafts through claim analysis, and check formal drafting rules against MPEP/EPO guidelines. Trigger when the user asks to draft or improve patent claims, write independent and dependent claims, or stress-test draft claims against known art and formal requirements.
- **recipe-custom-dashboard** — Presentation-craft recipe for turning verified FlowLeap patent data into a disposable, self-contained HTML dashboard — the agent writes one small zero-dependency Node program per question that fetches through the CLI, computes every number in code, and emits an offline single-file dashboard with inline SVG charts and a full provenance footer. Trigger when the user asks to make, build, generate, or visualize a dashboard, chart, or visual report of patent data — a company's portfolio, filing trends across competitors, a technology landscape / white-space map, or citation impact.
- **recipe-freedom-to-operate** — Freedom-to-operate clearance for a product or technology — per-feature query generation, dual-database blocking-patent search, and legal-status, family, and all-elements claim checks on every live candidate. Trigger when the user asks whether a product or technology can launch without infringing, or requests an FTO/clearance search.
- **recipe-infringement-charting** — Litigation recipe for an element-by-element infringement claim chart — verify the patent is in force where it matters, decompose the asserted claims, extract accused-product evidence (OCR datasheets and manuals), and map every element with literal and doctrine-of-equivalents columns. Trigger when the user asks whether a product infringes a patent, wants a claim chart, or needs element-by-element infringement mapping.
- **recipe-invalidity-analysis** — Litigation recipe for building an invalidity case against a target patent — pin the priority date, decompose claims into elements, hunt patent and non-patent prior art per element, and assemble an invalidity chart with X/Y/A reference tagging (single-reference novelty vs. combination obviousness vs. background). Trigger when the user asks to invalidate a patent, find 102/103 or Art. 54/56 prior art against granted claims, or assess how strong a patent really is.
- **recipe-invention-disclosure** — Prosecution recipe for turning an inventor conversation into a complete invention disclosure form (IDF) — structured technical capture, bar-date audit, novelty pre-check across patents and literature, landscape context, and a filing recommendation. Trigger when the user asks to document an invention, create an invention disclosure, or run a pre-filing novelty sanity check.
- **recipe-maintenance-fees** — Check US patent maintenance-fee status and deadlines — compute the 3.5/7.5/11.5-year windows from the grant date, read which fees were actually paid from USPTO transaction events, and report the next docketable deadline with surcharge dates. Trigger when the user asks whether maintenance fees are due or paid, when the next fee deadline is, whether a US patent is still in force fee-wise, or for a fee-status sweep over a portfolio. Dates and status only — never fee amounts.
- **recipe-office-action-response** — Prosecution recipe for turning an office action into a structured draft response — OCR the OA, pull every cited reference's claims and bibliography, map rejections element-by-element, and ground arguments in MPEP/EPO guideline citations. Trigger when the user asks to respond to an office action, analyze examiner rejections, or prepare arguments against novelty/obviousness (102/103, Art. 54/56) objections.
- **recipe-patent-landscape** — Patent-landscape analysis for a technology area — scoped dual-database search, key-player identification, full-corpus filing analytics, and CPC-versus-year white-space detection. Trigger when the user asks to map a technology space, identify who patents in an area, or report filing trends and white space.
- **recipe-patent-to-report** — Formatted single-patent dossier — pull bibliography, abstract, claims, description, family, legal status, prosecution timeline, figures, and related art into one structured Markdown report. Trigger when the user asks for a complete profile, dossier, or report on a specific patent.
- **recipe-prior-art-search** — Comprehensive prior-art / novelty search before filing — natural-language query generation, dual EPO/USPTO patent search, an academic literature sweep, and X/Y/A tagging of the closest hits. Trigger when the user asks to find prior art for an invention, run a novelty search, or check what already exists before filing.
<!-- END FLOWLEAP AGENT RULES -->
