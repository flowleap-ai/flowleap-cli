# FlowLeap CLI

Rust CLI for the FlowLeap Patent AI backend, and the canonical home of the
FlowLeap agent skills.

## Language

**CLI skill**:
A SKILL.md in this repo's `skills/` directory, written in the CLI dialect —
its instructions invoke `flowleap …` commands. Canonical: this is where
capability-skills are authored first. Baked into the binary at build time and
installed via `flowleap skills install/update`.
_Avoid_: calling these just "skills" when the app dialect could be meant.

**App skill**:
A SKILL.md in `flowleap-agent-v2 …/assets/skills/`, written in the VS Code
extension's tool dialect (`get_patent_summary`, `patent_api_request`, …) with
`user-invocable` frontmatter. Maintained separately — there is NO sync between
CLI skills and app skills; overlapping workflows (e.g. office-action response)
exist in both dialects and drift independently.

**Skill Pack**:
The marketplace distribution unit: a plugin in the `flowleap-plugins` monorepo
containing CLI skills copied byte-for-byte from a pinned flowleap-cli tag
(`sync.json` ref, drift-checked in CI). The website marketplace renders its
catalog from Skill Packs at build time. Skill Packs ship CLI skills only —
app skills never flow through them.

**MCP server**:
The `flowleap mcp` stdio bridge the CLI binary embeds: it mirrors the backend
tools registry as MCP tools and serves the backend's doctrine documents as MCP
resources and prompts. Speaks only what the registry publishes — it authors no
tool, no schema and no doctrine of its own. A **hosted MCP server** is the same
registry served by the backend over HTTP for clients that cannot run a binary;
the two differ in transport and sign-in only, never in tool list.
_Avoid_: "the MCP" alone where stdio and hosted could be meant; treating the
bridge as a place to add a tool or a rule (that is the registry's job).

**Hosted MCP server**:
The backend's `/mcp` endpoint: the same registry, doctrine resources and prompts as the **MCP server**, served over stateless Streamable HTTP with JSON replies to clients that cannot run a binary (claude.ai, Claude Desktop and mobile connectors, ChatGPT). A client signs in with a Clerk OAuth access token (scope `patent-data`) or a personal token; the per-tool gates and the per-identity rate limits are the ones every other transport uses. See ADR 0022.
_Avoid_: "remote MCP" or "MCP endpoint" alone (say hosted or stdio); "hosted tier" or "MCP plan" (MCP access is in every plan, ADR 0021); a tool, resource or prompt that only the hosted server lists.

**PATSTAT tool**:
A tool on the backend tools facade whose data comes from the analytics layer
(the PATSTAT snapshot): portfolio, guarded SQL query, docs, and the graph
verbs. A PATSTAT tool carries the Data Edition and the EPO attribution on
every result, needs no Patent-Data Key, and publishes its own gate (sign-in or
plan) and rate limit in the registry. The `patstat` commands are the CLI's
ergonomic verbs over these tools, the same way `ops` is over the document tools.
_Avoid_: "named non-facade exception" (the pre-2026-09 state, retired once the
tools are registered); "the PATSTAT API" (there is one facade).

**Agent-mediated onboarding**:
Onboarding driven by an agent on a human's behalf: the agent executes every
step it can and relays the rest to the human. Contrast with the interactive
wizard, where the human drives.

**Actor**:
Who performs a next step — `human` (browser sign-in, obtaining patent-data
keys) or `agent` (anything runnable headlessly). Every next step has exactly
one actor; a task needing both is two steps.

**Patent-Data Key**:
A credential the USER holds at a patent office — the EPO OPS consumer
key/secret pair, the USPTO ODP API key — that FlowLeap uses on the user's
behalf so that office's data flows. Free at each office and obtained through a
browser signup, so getting one is always a human step. Comes in two kinds, a
**Forwarded key** and a **Stored key**; for one office a forwarded key wins,
then a stored key, then (trial only) the server's own keys. `provider_keys_required` /
`provider_keys_invalid` / `trial_budget_exhausted` are the wire codes naming
the concept in error envelopes, `providerKeysHint` the envelope field.
_Avoid_: "provider keys" in prose (legacy CLI naming), and any wording that
reads as a FlowLeap paywall — the office issues the key, FlowLeap only carries
it.

**Forwarded key**:
A Patent-Data Key the client keeps on the user's machine and sends in the
request headers (`x-epo-ops-key` / `x-epo-ops-secret`, `x-uspto-odp-key`): for
this CLI, the keys in `credentials.toml` or the `FLOWLEAP_*_KEY` env vars. It
lives only for the request on the backend, is never logged, and always wins
over a Stored key for that office. `keys test` reports it as `source: "user"`
(the legacy wire name).
_Avoid_: "user key" alone (a stored key is the user's key too), "header key".

**Stored key**:
A Patent-Data Key the user asked FlowLeap to keep, entered once by the human on
the "Patent-data keys" page of the signed-in dashboard
(https://www.flowleap.co/en/dashboard/keys). Validated against the office
before save, encrypted at rest, write-only: no client can read the value back.
Used on every surface when the request carries no Forwarded key for that
office. `keys test` and `doctor` report it as `source: "stored"` and count it as
a key present (backend ADR 0023). An agent never asks for its value in the
chat: it gives the human the page link.
_Avoid_: "saved key" or "remembered key" (say stored), "vault", and any wording
that suggests FlowLeap can show the value back.

**Key gate**:
One office being unreachable because its Patent-Data Key is missing. A
**user-action stop**, not an exhausted route: only the user adding the key opens
that office, so no web-scraped substitute stands in for it — searches and
single-document reads alike. A gate is *read* from an explicit
`provider_keys_required` result, never *inferred* from an empty, truncated, or
errored one. Doctrine text: the `flowleap-keys` skill.
_Avoid_: calling a gated office a coverage gap or a dead route — both hide that
a two-minute human action fixes it.

**Trial data budget gate**:
The soft sibling of the Key gate (backend ADR 0017): during the trial, today's
SHARED data allowance on FlowLeap's own credentials is spent —
`trial_budget_exhausted` in the `providerKeysHint`, raised from the backend
code `trial_data_budget_exhausted` (429). Same doctrine as the Key gate, one
extra exit: the hint's `resetsAt` names when it lifts on its own, and the
user's own free Patent-Data Keys lift it permanently. Announced ahead by the
`trial_data_budget_low` warning on success envelopes.
_Avoid_: treating it as a rate limit to back off from and retry — the durable
fix is keys, not waiting.

**Trial period**:
The 7-day access window the backend grants at account creation (backend ADR
0018) — no card, no user action. Its expiry surfaces as the ordinary
subscription gate (402, exit 4, a "subscribe" ask), never as a Key gate or
Trial data budget gate stop (exit 9).
_Avoid_: "trial" alone where the Trial data budget gate could be meant — the
period is a clock, the budget is an allowance.

**Next step**:
A pending onboarding action that blocks work. Steps whose need is already
covered (e.g. a provider with a Stored key, or one the server has its own keys
for) are not next steps — the list means "what blocks you," not "what could be configured."

**Ready**:
Nothing blocks work: backend reachable, authenticated, no next steps.
Distinct from "reachable" — a reachable backend with no credentials is not
ready.

**Session token**:
The short-lived credential produced by the browser device-flow sign-in. It
expires on its own; a machine holding only a session token is signed in but
not durably set up.
_Avoid_: calling it just "the token" — that hides the expiry distinction.

**Personal token**:
The long-lived `fl_pat_…` credential a user mints for one machine or agent.
The durable way a machine stays authenticated; named at creation so it can be
listed and revoked individually.
_Avoid_: "API key" — the config field is historically named that, but the
domain concept is a personal token.

**Capability vs. skill**:
Skills instruct, tools reach. A *capability* (data access — a backend
endpoint/tool) is what agents call; a *skill* is instructions composing
existing capabilities into a workflow. A skill cannot substitute for a missing
capability, and a capability without a skill is undiscoverable in practice.
See AGENTS.md "Skills vs. tools" for the authoring policy.

**Designated contracting state** vs **extension/validation state**:
An EP application names the EPC member states it asks for protection in. Those
are its **designated contracting states**, on the INPADOC `AK` event and rolled
up as `designatedStates` by `ops legal` / `summary`. An **extension/validation
state** is a non-member state the applicant additionally asked the EP right be
extended or validated in (`AX` event, `extensionStates`). For an EP regional
filing the designation IS its **designated-state coverage** — which countries
the right can reach.
_Avoid_: bare "coverage" for this (that word means the extended family across
FlowLeap — say "designated-state coverage"); "the countries the family covers"
(a family names the **offices** a filing published in, and an EP regional
filing is one "EP" entry); reading an empty list as "unknown" — empty means the
record designates no states, and every non-EP publication designates none.
