# FlowLeap CLI

Rust CLI for the FlowLeap Patent AI backend, and the canonical home of the
FlowLeap agent skills.

## Language

**FlowLeap user**:
A patent professional who accesses FlowLeap capabilities through an agent
harness of their choice. They need not understand or operate the CLI directly.
_Avoid_: Developer, CLI operator

**Agent harness**:
The user-chosen local environment that hosts the agent operating FlowLeap,
provides shell and filesystem access, and can persist MCP configuration or
agent instructions, such as Codex, Claude Code, Cursor, or Gemini CLI.
Web-only chat applications are outside the CLI's supported environment.
_Avoid_: FlowLeap app, web chat

**Harness-integrated**:
The CLI is Ready and the chosen agent harness has a persistent FlowLeap
integration through MCP, installed skills/rules, or both. The harness can
discover and invoke FlowLeap in later sessions without repeating setup.
_Avoid_: Ready, installed

**Registry download**:
An npm registry download event for a published `flowleap` package version. It
may come from automation, caching, CI, or a person and does not prove an
installation, CLI execution, or user.
_Avoid_: download user, install, adoption

**Binary fetch**:
A retrieval of a native FlowLeap release asset, normally triggered when the
npm wrapper is executed for the first time for one version and platform. It is
a closer execution proxy than a Registry download but is still not a unique
person.
_Avoid_: active user

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
Who performs a next step — `human` (browser sign-in, obtaining provider
keys) or `agent` (anything runnable headlessly). Every next step has exactly
one actor; a task needing both is two steps.

**Next step**:
A pending onboarding action that blocks work. Steps whose need is already
covered (e.g. a provider the server has its own keys for) are not next
steps — the list means "what blocks you," not "what could be configured."

**Ready**:
Nothing blocks work: backend reachable, authenticated, subscription entitlement
active, no next steps. Distinct from "reachable" and "authenticated" — neither
proves that patent-data work is authorized.

**Subscription entitlement**:
The account state that authorizes FlowLeap patent-data work, granted by an
active Basic subscription or qualifying trial. Authentication identifies the
account; it does not provide this entitlement.
_Avoid_: signup, authentication

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
