---
name: flowleap-keys
description: Manage BYOK patent-data keys (EPO OPS consumer key/secret, USPTO ODP API key) for the FlowLeap CLI — check status, validate live, hand off to a human for the interactive setup wizard, and apply the key-gate doctrine (a gated office is a user-action stop, never an exhausted route). Trigger when a FlowLeap command fails with provider_keys_required or provider_keys_invalid, when patent data calls error about EPO/USPTO credentials, or when the user asks to configure provider keys.
---

# FlowLeap Patent-Data Keys (BYOK)

Patent data flows through provider APIs that may need the USER's own
credentials: EPO OPS (consumer key + secret — always a pair) and USPTO ODP
(single API key). The concept is **patent-data keys**; `provider_keys_required`
and `provider_keys_invalid` are the wire codes that name it in error envelopes.
Keys live in `credentials.toml` (0600) and are forwarded per-request; the CLI
never prints them (verbose/dry-run redact).

## Diagnose

```bash
flowleap --json keys list    # what's configured locally (masked)
flowleap --json keys test    # live verdicts: source user|server|none, valid true|false|null
flowleap --json doctor       # providerKeys section + pending steps in nextSteps
```

`keys test` needing nothing locally is fine when `source` is `server` — the
backend has its own keys and commands work without BYOK.

Doctor's `nextSteps` lists patent-data keys only when they actually **block**
work: server-covered providers produce no steps. A blocking provider appears
as an obtain/store pair — `obtain-epo-keys` / `obtain-uspto-key` (`actor:
"human"`, carries the signup `url` — relay it to the user) then
`store-epo-keys` / `store-uspto-key` (`actor: "agent"`, carries the `run`
command — execute it once the user hands you the keys) — followed by
`verify-keys` (`actor: "agent"`, runs `keys test`). When doctor cannot reach
the validation endpoint (unauthenticated/offline) it falls back to local key
presence and says so in `keyValidation.note`. See `flowleap-shared` for the
full `nextSteps`/`ready`/exit contract.

## The key-gate doctrine

**A key gate is a USER-ACTION STOP, never an exhausted route.** A
`provider_keys_required` error means that office needs a key only the user can
obtain — it is not a transient error, not a zero-result, and not a route you
have exhausted. So for the gated office:

- **Never substitute web-scraped data for it** — not for searches, and not for
  single-document reads. "Give me the claims of EP…" with no EPO OPS key is
  declined *for that office* with the key named as the one-step fix; it is never
  quietly served from Google Patents, Espacenet, freepatentsonline, or a web
  search instead. Only the user adding the key opens that office.
- **The keys are FREE** from each office (EPO OPS and USPTO ODP both issue them
  at no cost, browser signup). Never frame the ask as a paywall, an upsell, or a
  FlowLeap limitation.
- **A gate is READ, never INFERRED.** An office is gated only when a command you
  actually ran returned an explicit `provider_keys_required` (or
  `provider_keys_invalid`) code. Never conclude a gate from an empty result set,
  a truncated or partial payload, a 5xx, a timeout, or from `keys list` showing
  nothing. Anything short of that code is an ordinary dead or empty route, and
  normal persistence and fallbacks apply to it in full — reformulate, try the
  alternate office or route, then the usual web fallback.
- The forbid rule covers **only** an office gated on a missing patent-data key.
  Offices with no backend route at all (CN/JP/KR) and routes that are genuinely
  dead or empty *with a working key* keep their existing fallbacks unchanged.

**Proceed, then ask.** When one provider is live and the other is gated:

1. Complete the LIVE office fully — every search, read, and analysis the task
   asks of it. A missing key for one office is not a reason to do less work in
   the other.
2. Deliver those results as the normal deliverable.
3. Name the gap explicitly as a **missing-key gap**, never as a data or coverage
   finding: "EP coverage is missing because your EPO OPS key is not set" — not
   "no EP results were found", not "EP coverage is limited", not silence.
4. Ask for the missing key once, at the END of the turn, after the results.
5. **Never silently narrow scope.** A prior-art, novelty, patentability,
   freedom-to-operate, invalidity, or landscape task keeps the scope the work
   requires; the unsearched office is stated as an open gap in the deliverable,
   so no one mistakes a configuration detail for a clearance result.

**Keyless pivot — offer it as DIFFERENT data, never as a substitute.** These
need no patent-data key and stay live while an office is gated; label each for
what it actually is:

- `flowleap patstat …` (portfolio, guarded SQL, graph) — aggregates from a
  twice-yearly SNAPSHOT, not documents and not current. It does not answer
  "what prior art exists for this claim".
- `flowleap legal search …` — patent LAW (MPEP, EPC, guidelines). It gives the
  legal standard, never what has been published or filed.
- `flowleap academic search …` / `flowleap npl …` — scholarly LITERATURE. Papers
  are prior art in their own right, but they are a different corpus from patents.

Say plainly that this is different data, not a stand-in for the gated office's
live search.

**Resume — merge, do not restart.** When the user says they added the missing
key, re-run ONLY the previously gated office and merge its results into the
deliverable you already produced. Do not redo the live office's work. Keys reach
the request headers on the next invocation: no restart, no new session.

## The agent protocol — when keys are missing or rejected

Failed commands carry a `providerKeysHint` in the JSON error envelope:

```json
"providerKeysHint": {
  "code": "provider_keys_required",      // or provider_keys_invalid
  "provider": "epo",
  "requiresHumanIntervention": true,
  "nonInteractive": { "command": "flowleap keys set epo --key … --secret …",
                       "env": ["FLOWLEAP_EPO_KEY", "FLOWLEAP_EPO_SECRET"] },
  "signup": "https://developers.epo.org (free, 'My apps' → create app)"
}
```

**Getting keys requires a browser signup — an agent cannot complete this
alone. Do not retry, do not invent keys.** Tell the user:

> This command needs EPO OPS credentials. Please run `flowleap setup` in a
> terminal (guided, ~2 minutes; free keys from https://developers.epo.org),
> then I'll continue.

If the user hands you keys directly, apply them non-interactively — they are
validated live before saving, and rejected keys are NOT saved:

```bash
flowleap --json keys set epo --key <consumer-key> --secret <consumer-secret>
flowleap --json keys set uspto --key <api-key>
flowleap --json keys test
```

Or per-session via env: `FLOWLEAP_EPO_KEY`, `FLOWLEAP_EPO_SECRET`,
`FLOWLEAP_USPTO_KEY`.

## Human commands (mention, never run yourself)

`flowleap setup` — full onboarding wizard (backend check → auth check →
per-provider prompts with hidden input, live validation, skippable steps with
explicit warnings). Refuses to run without a TTY. `flowleap keys rm epo|uspto`
removes stored keys.
