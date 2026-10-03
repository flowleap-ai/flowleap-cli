---
name: recipe-custom-dashboard
description: Presentation-craft recipe for turning verified FlowLeap patent data into a disposable, self-contained HTML dashboard — the agent writes one small zero-dependency Node program per question that fetches through the CLI, computes every number in code, and emits an offline single-file dashboard with inline SVG charts and a full provenance footer. Trigger when the user asks to make, build, generate, or visualize a dashboard, chart, or visual report of patent data — a company's portfolio, filing trends across competitors, a technology landscape / white-space map, or citation impact.
metadata:
  requires:
    skills: ["flowleap-shared", "flowleap-patstat", "flowleap-citation"]
---

# Recipe: Custom Dashboard (Disposable, Verified-Data)

Instead of a fixed dashboard product, write a **small program per question**: fetch
verified data through the FlowLeap CLI, compute every number in code, and emit one
self-contained HTML file. This skill owns **presentation craft only** — the
*analysis* (which query, which corpus, how to read the result) lives in the
analytical recipes. Do the analysis there first, then render here.

In a chat client with the FlowLeap connector, call the tools named in the `flowleap-shared` connector table instead of these commands. A chat client has no file system, so it writes no bundle and no `generate.mjs`: describe the data, and hand the numbers to the user exactly as the tools returned them, with each call's tool, parameters, and dataset identity.

Vocabulary (agent-v2 `CONTEXT.md`): **Portfolio Analytics** = grounded PATSTAT
aggregates for one harmonized applicant (`data_edition` stamped). **Topic
Analytics** = full-corpus filing analytics over the quarterly Google-Patents slice
(corpus-slice stamped). **Verified-Data Contract** = every figure on the page is
interpolated from a computed variable, never typed by the model.

## Mandatory rules

These are product decisions, not style preferences. A dashboard that breaks one of
them is wrong even if it looks right.

1. **Numbers only from code — including prose.** Every value in the HTML (chart,
   table, and every narrative sentence) is interpolated from a computed variable.
   The model never literal-types a figure. Use the template's `renderNarrative(computed)`
   helper for sentences; embed backend-authored `summary` strings **verbatim** as
   canonical captions. In chat, the dashboard is the source of record: the script
   prints a summary block — quote its numbers, never re-type them from memory.
2. **Data via CLI subprocess only.** Scripts spawn `flowleap … --json` with
   `child_process.execFile`. Never hold a token, never raw-`fetch` the backend. The
   binary is injectable via `FLOWLEAP_CLI_BIN` (default `flowleap`) so the program
   stays testable, but it is always a subprocess.
3. **Node-only, zero-dependency.** One self-contained `.mjs` per dashboard
   (Node ≥ 18). No `npm install`, no build step, no chart library. The inline SVG
   chart helper is **copied into** each program — keep it copy-in, not imported.
4. **Bundle layout.** Workspace-relative `dashboards/<subject-slug>/`:
   - `generate.mjs` — the program. Refinements edit **this file** and re-run.
   - `dashboard.html` — regenerated output. **Never hand-edited.**
   - `data/` — one raw-JSON sidecar per backend call.

   Slugs are **subject-based**, no timestamps (`dashboards/siemens-portfolio/`, not
   `dashboards/2026-07-23-run/`). Re-running refines in place; to keep a prior state,
   copy the bundle or commit it first. **Never overwrite a bundle you did not create**
   without showing the user its current contents.
5. **Provenance footer, per data source.** For each backend call: the tool/verb +
   its parameters, the dataset identity (`data_edition` for Portfolio Analytics; the
   corpus-slice stamp for Topic Analytics), and retrieved-at. A source with no
   dataset identity gets an explicit **"dataset identity unavailable"** row — never
   omit it. Plus one page-level generation timestamp and the CLI/backend versions.
6. **Reproduce block.** The HTML embeds the exact commands/script that produced the
   data, including any **pinned exact applicant name**. Applicant ambiguity is
   resolved once with the user and then pinned as a constant in `generate.mjs`.
   **Never auto-pick a 422 candidate** — a non-interactive run exits non-zero listing
   the candidates, and the affected section is not rendered.
7. **Self-contained HTML.** Inline CSS + inline SVG only. No CDN, no external fonts,
   no runtime JS dependency. It must open from `file://` offline.
8. **Local by default, never auto-published.** Dashboards can contain pre-filing
   invention data. Open in the system browser (`open` / `xdg-open`). Publishing to any
   hosted surface happens **only on explicit user request**. In-IDE viewing:
   `file://` will not load in the IDE's Simple Browser — run a one-line static server
   (`python3 -m http.server 8080` from the bundle dir) and open `http://localhost:8080/dashboard.html`.

## The four templates

Copy the matching template from `references/` into `dashboards/<slug>/generate.mjs`,
pin the constants at the top, and run it. Each is a single runnable `.mjs` with the
data contract documented in its header comment and the SVG chart helper embedded.

| Template | Question | Data source | Analytics kind |
|---|---|---|---|
| `template-portfolio.mjs` | One company's filings by year × office + grant ratio | `flowleap patstat portfolio` | Portfolio |
| `template-filing-trends.mjs` | Several companies' filings over time, compared | `flowleap patstat portfolio` (per applicant) | Portfolio |
| `template-landscape-whitespace.mjs` | CPC × year heatmap → white space in a technology area | `flowleap analytics` (scoped per CPC) | Topic |
| `template-citation-impact.mjs` | Forward-citation distribution for a patent (how influential) | `flowleap citation forward` / `stats` | — |

Portfolio and filing-trends need the PATSTAT layer. Landscape and citation-impact do
not. **Degrade gracefully:** if `patstat` returns `patstat_unavailable`, the template
exits non-zero and tells the user that the landscape and citation-impact dashboards
remain usable — it does not fail the whole recipe.

## Chart craft (baked into the helper)

The copied helper enforces the house style so you do not have to: an Okabe–Ito
colorblind-safe categorical palette, a single-hue sequential ramp for heatmaps, a
white background, readable axes with units and year labels, thousands-separated
numbers, and **no gradients or chartjunk**. Every chart carries its title and a
`Source:` label inside the SVG. Do not hand-tune colors per chart — extend the helper
if a new mark is needed.

## Reproduce & provenance (what the footer must contain)

```
Generated:      <ISO timestamp>              # one per page
CLI / backend:  flowleap <ver> / <backend ver>  # flowleap --version, flowleap --json doctor
Per source:
  verb + params           e.g. patstat portfolio "Siemens AG" --from-year 2015 --to-year 2024
  dataset identity        data_edition: "PATSTAT Global 2025 Spring"   (or corpus-slice stamp)
                          — or the literal "dataset identity unavailable"
  retrieved-at            <ISO timestamp>
Reproduce:      the exact commands, with the PINNED applicant constant, copy-pasteable
Data:           links to ./data/*.json sidecars
```

The templates build every one of these fields from computed variables and the
response envelope — read `renderFooter()` in any template and keep its markers.

## Routing

A standalone trigger ("make me a dashboard of X") routes here for the **rendering**,
but the analysis belongs to a matching recipe: portfolio/filing questions →
Portfolio Analytics data; landscape/white-space → `recipe-patent-landscape` (this is
its final rendering step); citation impact → the `flowleap-citation` family. Run the
analysis, pin the resolved parameters, then render with the template.

## Output

- A `dashboards/<subject-slug>/` bundle: `generate.mjs`, `dashboard.html`, `data/`.
- An offline, single-file `dashboard.html` with inline SVG charts and the full
  provenance + reproduce footer.
- A printed summary block (computed numbers + file paths) for the agent to paste in
  chat — the quoted numbers come from the script, never re-typed.
