# PRD 0002: npm Package Documentation and Agent-Mediated Setup

## Problem

The published `flowleap` npm package contains only `package.json`,
`download.mjs`, and `bin/flowleap`. Its package page therefore has no README
and the tarball omits the declared MIT license text.

npm Registry downloads are also being mistaken for users. In the week ending
2026-07-25, 716 of 778 Registry downloads were spread across six versions
published within roughly 26 hours. The similar per-version counts are
consistent with automated registry consumers, not six independent cohorts of
people. Native Binary fetches are a closer first-execution proxy but are still
not unique users.

## Goal

Publish a complete, predictable npm package page and give a FlowLeap user an
optional prompt that can leave their chosen local Agent harness persistently
integrated and Ready without requiring the user to understand the CLI.

## Audience and Positioning

- A FlowLeap user is a patent professional, not necessarily a developer or CLI
  operator.
- The CLI connects FlowLeap to a local, shell-capable Agent harness selected by
  the user.
- Web-only chat applications are outside the CLI's supported environment. This
  is a short compatibility note near the setup prompt, not the README headline.
- The agent setup prompt is part of the README but does not replace the normal
  command-line quickstart.

## Package Contents

The repository-root `README.md` and `LICENSE` remain canonical. Exact tracked
mirrors live at `npm/README.md` and `npm/LICENSE` so local `npm pack` produces
the same documented package as CI.

`npm/package.json` uses an explicit allowlist:

```json
{
  "files": [
    "bin/flowleap",
    "download.mjs",
    "README.md",
    "LICENSE"
  ]
}
```

The packed tarball must contain exactly those four artifacts plus
`package.json`.

## README Structure

1. Existing product introduction
2. Existing normal CLI quickstart
3. Optional “Let your agent set up FlowLeap” section
4. Local/shell-capable harness compatibility note
5. Universal copyable prompt
6. Exact Codex, Claude Code, Cursor, and Gemini MCP/skills command table
7. Existing requirements, command reference, and troubleshooting

The same README is rendered on GitHub and npm. It currently has no relative
links or embedded repository assets that require npm-specific rewriting.

## Universal Agent Setup Prompt

```text
Set up FlowLeap in this local agent harness.

Official sources:
- CLI guide: https://www.flowleap.co/en/cli
- Agent and MCP setup: https://www.flowleap.co/en/mcp
- Documentation: https://www.flowleap.co/en/docs
- Source repository: https://github.com/flowleap-ai/flowleap-cli

1. Check whether `flowleap` is installed.

   If it is missing, run:
   npm install -g flowleap

   If it is installed, run:
   flowleap upgrade --check --json

   If an update is available, run:
   flowleap upgrade

2. Identify the current agent harness and configure FlowLeap user-wide. When
   supported, configure both MCP and the matching FlowLeap skills/rules. Follow
   the official Agent and MCP setup documentation above. If MCP is unsupported,
   use the CLI with the harness-specific skills/rules.

3. Run:
   flowleap --json doctor

   Parse the JSON even when the command exits nonzero.

4. Execute every `nextSteps` entry owned by `actor: "agent"`. Present entries
   owned by `actor: "human"` to the user and wait for completion. Follow this
   harness's secret-handling policy; never expose, log, or invent credentials.

5. Repeat `flowleap --json doctor` until `ready` is true.

6. Verify the persistent MCP and/or skills integration without running a live
   patent search.

7. Report the installed version, integration scope, configured mechanisms,
   and final readiness.
```

The CLI owns secret redaction and safe credential commands. The harness owns
its approval and secret-entry UX. When secure entry is unavailable, the agent
hands the user off to `flowleap setup` in their own terminal and resumes with
`flowleap --json doctor`.

## Readiness Contract

The setup prompt relies on `doctor` as its single readiness authority. Ready
therefore requires all of:

- backend reachable;
- authenticated with durable credentials;
- active subscription entitlement, including an active trial;
- provider data access available through valid user keys or server coverage;
- no pending blocking next steps.

`/api/profile` is the authoritative subscription source. It returns a
normalized subscription object:

```json
{
  "subscription": {
    "entitled": true,
    "status": "trialing",
    "plan": "Basic",
    "action": null
  }
}
```

When blocked, `action` identifies the appropriate human action, such as
`subscribe-basic` or `resolve-billing`, with its URL. The backend owns billing
provider states, grace periods, and entitlement policy.

Doctor behavior:

- `entitled: true`: subscription does not block Ready.
- Authoritatively not entitled: `ready: false`; render the backend-provided
  human action.
- Missing subscription object or failed verification: status `unknown`,
  `ready: false`, and an agent-owned `verify-subscription` step. Never infer
  that an unknown account should purchase.

ADR 0002 records this public contract change.

## CI and Verification

CI must:

- fail when `README.md` and `npm/README.md` differ;
- fail when `LICENSE` and `npm/LICENSE` differ;
- run `npm pack --dry-run --json` from `npm/`;
- assert the exact five-file tarball allowlist;
- assert README and license presence explicitly;
- test doctor for entitled, trialing, blocked-with-action, and unknown states;
- preserve the existing actor-tagged `nextSteps` and exit-code contracts;
- run the repository's required `cargo fmt --check`, `cargo clippy`,
  `cargo test`, and `cargo build` checks.

## Rollout

1. Add the backward-compatible normalized subscription object to
   `/api/profile`.
2. Verify active, trialing, inactive, billing-action, and unknown scenarios.
3. Implement and verify the CLI/package/README changes.
4. Publish one coherent patch release, `0.4.1`; do not split this work across a
   burst of production versions.

## Release Policy

- Bundle ordinary fixes into meaningful production releases.
- Use prerelease versions such as `0.5.0-rc.1` for rapid validation.
- Treat npm download counts as Registry activity, never unique users.
- Treat native Binary fetches only as a rough execution proxy.

## Out of Scope

- Signup conversion optimization or product telemetry.
- Changing the README's primary quickstart into an agent-only journey.
- Supporting web-only chat applications.
- Running a live patent-data search as part of setup verification.
- Moving billing policy from the backend into the CLI.
