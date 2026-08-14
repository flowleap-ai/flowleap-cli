# ADR 0001 — Agent-mediated onboarding contract

## Status

Accepted (2026-07). Issues #40 (PRD), #41 (structured login), #43 (doctor
readiness contract).

## Context

The CLI's day-to-day users are AI agents, but onboarding was built for a human
at a TTY: `auth login` spoke through a spinner and browser side effects, and
`doctor --json` reported raw status with prose guidance — no machine-readable
answer to "what still blocks work, and who has to do it?". Agent-mediated
onboarding — the agent executing every step it can and relaying the rest to
the human — needed a first-class contract.

## Decision

1. **`flowleap doctor` exits 0 iff *ready-to-work*** (backend reachable AND
   authenticated AND no blocking next steps), else 1 — the existing
   generic-failure code, with the full checklist JSON always emitted first.
   *Rejected: always exiting 0* (diagnostics "succeeded") — that forces every
   `flowleap doctor && <work>` gate to parse JSON; the `keys test` precedent
   already exits nonzero on a bad verdict, and every audited consumer passes
   `--json` and tolerates nonzero exits. *Rejected: a new dedicated exit
   code* — the documented code table stays closed.

2. **`nextSteps` lists pending steps only, each with exactly one
   `actor` (`"human"` | `"agent"`)**, a stable kebab-case `id` (public
   contract: `auth-login`, `mint-personal-token`, `obtain-epo-keys`,
   `store-epo-keys`, `obtain-uspto-key`, `store-uspto-key`, `verify-keys`,
   `refresh-skills`), a `title`, and optional `run` / `url`. Always present;
   empty when complete.
   *Rejected: a full checklist with per-step `status`* — agents would re-derive
   "pending" from every entry, and completed steps invite re-running them.
   *Rejected: plain strings* — unactionable without stable ids, actors, and
   runnable commands. Obtaining keys (human, browser signup) and storing them
   (agent) are separate steps because a task needing both actors is two steps.

   **Amended (cold-start walkthrough, 2026-08-14):** a step may carry
   `advisory: true`, and readiness counts only the steps without it.
   `mint-personal-token` is the sole advisory step: a session token works
   today and merely expires later, so counting it against readiness made a
   machine that can run every command exit 1 with nothing broken — the
   `flowleap doctor && <work>` gate then fails for a durability suggestion.
   The step still appears (agents should act on it); it just does not claim
   the machine is unready. *Rejected: dropping the step when only advisory* —
   the durability gap is real and an agent that never sees it never closes it.

   `refresh-skills` (added by the same amendment) is blocking, not advisory:
   skill files written by an older CLI document commands and endpoints this
   build no longer calls, so an agent reading them walks into retired routes.
   It is a wrong-answer source, not an upgrade nag.

3. **Server-covered provider steps are omitted.** Doctor makes a best-effort
   authenticated `POST /v1/keys/validate`; providers with `source: "server"`
   (or valid user keys) produce no steps — the list means "what blocks you".
   *Rejected: marking covered steps `optional`* — an optional entry still
   reads as a nag and re-introduces per-step status. *Rejected: local-only
   detection* — it cannot distinguish "missing and blocking" from "missing but
   covered", the exact gap the PRD identified. When unauthenticated or the
   call fails, doctor falls back to local key presence with a note
   (`keyValidation.source: "local"`) and never errors — diagnosis works
   offline.

4. **Structured sign-in is one blocking NDJSON process** (#41): `--json auth
   login` emits `device_authorization` (URL + user code) immediately, then
   exactly one terminal `authorized`/`failed` event, with no browser or
   clipboard side effects. *Rejected: a two-command start/poll pair* — it
   pushes polling state (device code, interval, expiry) onto every agent and
   doubles the surface for the same outcome; one process that blocks until
   the human approves maps directly onto "run in background, await exit".

5. **`ready` is a new field; `ok` is not redefined.** `ok` keeps its
   reachability meaning byte-for-byte, per the repo's additive structured-hints
   policy. *Rejected: redefining `ok`* — existing `--json doctor` consumers
   (dashboard templates, audit recipe) depend on it.

## Consequences

- Agents drive onboarding as a checklist: run `agent` steps via `run`, relay
  `human` steps (title + `url`), re-run doctor until `ready` — and scripts
  gate with `flowleap doctor && <work>` without parsing anything.
- The step ids and NDJSON event names are a public contract once skills
  document them: renames are breaking changes governed by this ADR.
- Doctor's exit code now reflects readiness, not command success — a running
  but un-onboarded machine exits 1 by design.
- The best-effort validate call adds one authenticated request per doctor run;
  its failure can only widen `nextSteps` (local-presence fallback), never
  error the command.
