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
key/secret pair, the USPTO ODP API key — that FlowLeap forwards per request so
that office's data flows. Free at each office and obtained through a browser
signup, so getting one is always a human step. `provider_keys_required` /
`provider_keys_invalid` / `trial_budget_exhausted` are the wire codes naming
the concept in error envelopes, `providerKeysHint` the envelope field.
_Avoid_: "provider keys" in prose (legacy CLI naming), and any wording that
reads as a FlowLeap paywall — the office issues the key, FlowLeap only carries
it.

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

**Next step**:
A pending onboarding action that blocks work. Steps whose need is already
covered (e.g. a provider the server has its own keys for) are not next
steps — the list means "what blocks you," not "what could be configured."

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
