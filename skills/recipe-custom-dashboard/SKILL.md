---
name: recipe-custom-dashboard
description: "Turn verified patent data into a self-contained HTML dashboard with charts: the agent saves each tool answer unchanged as a JSON file, a small zero-dependency Node program checks each file against its tool input and computes every number from the files, and a footer gives the tool, input, dataset identity and save method of each source. Without Node it reports the verified numbers as the tools returned them. Use it when the user asks for a dashboard, chart or visual report of patent data, such as a company's portfolio, filing trends across competitors, a technology landscape or white-space map, or citation impact."
metadata:
  requires:
    skills: ["recipe-patent-landscape"]
---

# Recipe: Custom Dashboard (Disposable, Verified-Data)

**Tools.** This method calls the FlowLeap patent tools by name. In the FlowLeap Patent AI app the office tools are built in; `patent_analytics`, `get_fulltext` and the `patstat_*` tools are FlowLeap Data tools, named `mcp__flowleap__<name>`, which exist only when the firm or the user has FlowLeap Data. With the FlowLeap connector in another client, call the connector tool of the same name. When `get_claims` answers that it has no US source, call the tool that its answer names.

**Missing keys.** When a tool answers that an office's key is not configured or that the office rejected the stored key, that office is a user-action stop, never an exhausted route: finish the work with the other offices, name the gap in the result as a missing-key gap (not as "no results"), never fill it with data from the web, and at the end tell the user which key is missing or rejected. On the desktop app the user adds it in Settings > Patent data connectors; on the firm OS the firm administrator sets the keys, so tell the attorney to ask the administrator; in another client, relay the page that the tool's answer names. Never ask for a key value in the chat.

Write a **small program per question** instead of using a fixed dashboard product. You call the tools and save each answer, unchanged, as a JSON file. The program reads those files, checks them, computes every number from them in code, and writes one self-contained HTML file. This skill owns **presentation only**. The analysis (which query, which corpus, how to read the result) lives in the analytical recipes. Do the analysis there first, then render here.

Terms used in this skill:

- **Portfolio Analytics**: the answer of `patstat_portfolio`, application counts from EPO PATSTAT for the entities that one applicant name matched. Its dataset identity is `data_edition`.
- **Topic Analytics**: the answer of `patent_analytics`, publication counts over a quarterly slice of Google Patents. Its dataset identity is `data_slice`.
- **Verified data**: every figure on the page comes from a saved tool answer, which the agent copied or wrote out unchanged, and the program computes each figure from those files. The agent types no figure into the program, the page or the chat.

## Mandatory rules

These are product decisions, not style preferences. A dashboard that breaks one of them is wrong even if it looks right.

1. **Numbers only from code, including prose.** Every value in the HTML (chart, table, and every narrative sentence) is interpolated from a variable that the program computed from the saved answers. Never type a figure into the program or the chat. A data file holds only a tool answer, copied or written out unchanged (rule 2); the one exception is `saved-how.json`, the agent's own record of how it saved each file. Use the template's `renderNarrative(c)` helper for sentences; embed tool-authored `summary` strings **verbatim** as captions. In chat, quote the summary block that the program prints; never re-type numbers from memory.
2. **Data only from saved tool answers.** The program never calls a tool, holds a key, or reaches the network. Save each answer to the `data/` file that `node generate.mjs --plan` names, **right after the call**, before the next call:
   - When the answer says that the full result is stored in a file, copy that file with the shell (`cp <stored path> data/<file>`).
   - Otherwise write the answer text exactly as the tool returned it, with the file-write tool. In the FlowLeap Patent AI app this is the usual path.
   - Never edit, reformat, shorten, or summarize an answer. Never merge two answers into one file.
   - When the copy is refused and the answer is too long to write unchanged, make the call smaller (for example smaller citation pages) and save each part.
   - After each save, record how you saved the file in `data/saved-how.json` (`{ "<file>": "copied" }` or `"written"`). The footer row of each source shows "recorded by the agent as copied from the stored result" or "recorded by the agent as written out"; a file without an entry shows "save method not recorded".

   The program checks each file: it must echo the pinned input, and its rows must add up to its totals where the answer carries both. A failed check exits 4 and names the file: call the tool again and save the new answer. Never edit the file to pass a check. The checks find a file for another input and a file whose rows do not add up. They cannot prove that a written-out file is the unchanged answer: the footer says which files the agent wrote out.
3. **Node only, zero dependencies.** One self-contained `.mjs` per dashboard (Node 18 or later). No `npm install`, no build step, no chart library. The inline SVG chart helper is **copied into** each program; keep it copied, not imported. Run `node --version` first. When it fails, write no dashboard file: report the verified numbers as described in **Without a dashboard file**.
4. **Bundle layout.** Workspace-relative `dashboards/<subject-slug>/`:
   - `generate.mjs`: the program. Refinements edit **this file** and re-run.
   - `dashboard.html`: regenerated output. **Never edit it by hand.**
   - `data/`: one saved answer per tool call.

   Slugs name the subject, with no timestamps (`dashboards/siemens-portfolio/`, not `dashboards/2026-07-23-run/`). Re-running refines in place; to keep a prior state, copy the bundle first. **Never overwrite a bundle you did not create** without showing the user its current contents.
5. **Provenance footer, per tool call.** For each call: the tool name and its exact input, the dataset identity (`data_edition` for Portfolio Analytics, `data_slice` for Topic Analytics), and the time the answer was saved. A source with no dataset identity gets an explicit **"dataset identity unavailable"** row; never omit it. Plus one page-level generation timestamp.
6. **Reproduce block.** The HTML lists every tool call with its input and its `data/` file, then `node generate.mjs`, including any **pinned exact applicant name**. Resolve applicant ambiguity once with the user, then pin the name as a constant in `generate.mjs`. **Never auto-pick a candidate.** When `patstat_portfolio` answers that a name is ambiguous, show the candidates to the user and do not save that answer. A saved ambiguous answer stops the portfolio program (exit 3, candidates listed); the filing-trends program skips that applicant and notes it.
7. **Self-contained HTML.** Inline CSS and inline SVG only. No CDN, no external fonts, no runtime JS dependency. It must open from `file://` offline.
8. **Local by default, never auto-published.** Dashboards can contain pre-filing invention data. In the FlowLeap Patent AI app, give the file to the user with `present` (`files: [{ path: "dashboards/<slug>/dashboard.html", description }]`). In another client, open it in the system browser (`open` or `xdg-open`); when the editor's built-in browser does not load `file://`, run `python3 -m http.server 8080 --bind 127.0.0.1` from the bundle folder and open `http://127.0.0.1:8080/dashboard.html`. Publish to a hosted surface **only on explicit user request**.

## The four templates

Copy the matching template from `references/` into `dashboards/<slug>/generate.mjs`, pin the constants at the top, and follow the steps below. Each template documents its answer fields, file names and checks in its header comment, with the SVG chart helper embedded.

| Template | Question | Tool calls and files | Needs |
|---|---|---|---|
| `template-portfolio.mjs` | One company's filings by year × office + grant ratio | `patstat_portfolio` (`applicant`, `from_year`, `to_year`) → `data/portfolio.json` | FlowLeap Data |
| `template-filing-trends.mjs` | Several companies' filings over time, compared | `patstat_portfolio` per applicant → `data/portfolio-<applicant-slug>.json` | FlowLeap Data |
| `template-landscape-whitespace.mjs` | CPC × publication-year heatmap of publication counts → white space in a technology area | `patent_analytics` (`keywords` or `phrases`, `dateFrom`, `dateTo`) once broad → `data/analytics-overview.json`, then once per CPC with `cpc: ["<code>"]` → `data/analytics-cpc-<code>.json` | FlowLeap Data |
| `template-citation-impact.mjs` | Forward citations of a patent in USPTO office actions (a partial measure of influence: other offices, applicants and literature are not in the data; the page says so) | `search_enriched_citations` (`cited_document`, `size`, `offset`, `examiner_cited_only`) per page → `data/citations-<offset>.json` | USPTO key |

**Degrade.** Without FlowLeap Data, `patstat_portfolio` and `patent_analytics` do not exist: say so, and offer the citation-impact dashboard, which uses an office tool. When a saved `patstat_portfolio` answer reports `patstat_unavailable`, the program exits 2 and says that the landscape and citation-impact dashboards remain usable.

## Steps

1. **Analysis first.** Run the matching analytical recipe and pin its resolved parameters: the exact applicant names, the years, the keywords or phrases, the CPC codes, or the cited document.
2. **Copy and pin.** Copy the template to `dashboards/<slug>/generate.mjs` and set the constants.
3. **Plan.** Run `node generate.mjs --plan`. It prints each tool call (tool and input) and the `data/` file for its answer.
4. **Call and save.** Call each tool with exactly that input. Save each answer at once, as rule 2 says.
5. **Render.** Run `node generate.mjs`. A stop names its cause:
   - exit 4: a file is missing, is not JSON, does not echo its input, or does not add up. For citation impact it also names the next page to fetch when `total` exceeds the pages saved. Make the named call, save the answer, and run again.
   - exit 3: an applicant is ambiguous, or its name matched no applications of its own but only other entities. Ask the user, pin the name, call again.
   - exit 2: PATSTAT is not available. Offer the other dashboards.
6. **Deliver.** Give the file to the user (rule 8) and quote the printed summary block.

Tool notes:

- `patstat_portfolio`: keep the default `offices: "top"` so the answer stays short enough to save unchanged. Report the answer's `notes` about other entities that the name matched but the totals leave out. A full legal name can match no applications of its own: on PATSTAT 2026 Spring, "Siemens AG" answers `totals.applications: 0` with 40 `other_matches`, while "Siemens" matches 38,658 applications. When `totals.applications` is 0 and `applicant.other_matches_total` is above 0, show the other matches to the user and agree on a name, usually a shorter prefix; the templates stop on such an answer (portfolio exits 3, filing trends skips that applicant).
- `patent_analytics`: `byYear` counts publications by publication year; `byCountry` counts each office's publication separately; `topCPC` counts classification entries, not documents. Label the basis on the page, and take each heatmap row from its own scoped call, never from `topCPC`.
- `patent_analytics` input: `keywords` are OR-combined title/abstract substrings; each of `phrases` is one exact substring. `cpc` matches by prefix. The answer's `data_slice` is its dataset identity.
- `search_enriched_citations`: give `cited_document` as offices print it in citations, with spaces and kind code (`US 20150284881 A1`). When the compact form (`US20150284881A1`) answers `total: 0`, call again with the spaced form before you report zero citations. Use `size: 25` pages so each answer stays short enough to save unchanged.

## Chart craft (built into the helper)

The copied helper enforces the house style: an Okabe–Ito colorblind-safe categorical palette, a single-hue sequential ramp for heatmaps, a white background, readable axes with units and year labels, thousands-separated numbers, and **no gradients or chartjunk**. Every chart carries its title and a `Source:` label inside the SVG. Do not hand-tune colors per chart; extend the helper if a new mark is needed.

## Reproduce and provenance (what the footer must contain)

```
Generated:      <ISO timestamp>              # one per page
Per tool call:
  tool + input            e.g. patstat_portfolio {"applicant":"Siemens","from_year":2015,"to_year":2024}
  dataset identity        data_edition: PATSTAT 2026 Spring   (or data_slice: ...)
                          or the literal "dataset identity unavailable"
  retrieved               <ISO time the answer file was saved>
  saved                   recorded by the agent as copied from the stored result | recorded by the agent as written out | save method not recorded
Reproduce:      every tool call with its input and data/ file, then node generate.mjs
Data:           links to ./data/*.json, the saved answers
```

The templates build every one of these fields from the saved answers and the pinned constants. Read `renderFooter()` in any template and keep its markers.

## Without a dashboard file

When Node is not available, or the user wants no file, report the numbers as the tools returned them: each number with its tool, input and dataset identity, quoted from the answer (for example the `summary`, `totals` and `by_year` rows of `patstat_portfolio`). Do not compute sums, shares or rankings by hand; say that a dashboard file needs Node.

## Routing

A standalone request ("make me a dashboard of X") routes here for the **rendering**, but the analysis belongs to a matching recipe: portfolio and filing questions use Portfolio Analytics (`patstat_portfolio`); landscape and white space use `recipe-patent-landscape` (this is its final rendering step); citation impact uses `search_enriched_citations`. Run the analysis, pin the resolved parameters, then render with the template.

## Output

- A `dashboards/<subject-slug>/` bundle: `generate.mjs`, `dashboard.html`, `data/`.
- An offline, single-file `dashboard.html` with inline SVG charts and the full provenance and reproduce footer.
- A printed summary block (computed numbers and the file path) to quote in chat. The quoted numbers come from the program, never re-typed.
