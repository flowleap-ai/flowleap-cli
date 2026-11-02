---
name: flowleap-keys
description: "Set up and check the patent-office data keys (EPO OPS key and secret, USPTO ODP key) of the FlowLeap command-line tool: key status, live validation, the gate codes, and the hand-over to the human for the setup wizard. Use it when a FlowLeap command fails with provider_keys_required or provider_keys_invalid, or when the user asks to configure patent-office keys for the command-line tool."
user-invocable: false
---

# FlowLeap Patent-Data Keys (BYOK)

In a chat client with the FlowLeap connector or in the FlowLeap Patent AI app, call the tools named in the `flowleap-shared` tool table instead of these commands, and never run a `flowleap` command there. No tool replaces the keys commands. In a chat client the user adds a stored key on the Patent-data keys page; in the app the firm administrator or the desktop user sets the keys (see `flowleap-shared`). Never ask for the key value in the chat.

Patent data flows through provider APIs that may need the USER's own
credentials: EPO OPS (consumer key + secret — always a pair) and USPTO ODP
(single API key). The concept is **patent-data keys**; `provider_keys_required`
and `provider_keys_invalid` are the wire codes that name it in error envelopes.
A key reaches the backend in one of two ways:

- **Forwarded key** — kept on this machine in `credentials.toml` (0600) and
  sent with each request as `x-epo-ops-key` / `x-epo-ops-secret` /
  `x-uspto-odp-key`. The CLI never prints it (verbose/dry-run redact). A
  forwarded key always wins for its office.
- **Stored key** — the human adds it once on the FlowLeap "Patent-data keys"
  page (https://www.flowleap.co/en/dashboard/keys). FlowLeap keeps it
  encrypted and never shows the value again. It is used when the request
  carries no forwarded key for that office.

The gate applies wherever patent data is read — the commands and the Tools
facade alike, since the commands run on the facade. Key validation
(`keys test`, `keys set`) runs outside the facade and works before a
subscription exists, so setup can always be diagnosed.

## Diagnose

```bash
flowleap --json keys list    # what's configured locally (masked); alias: keys status
flowleap --json keys test    # live verdicts: source user|stored|server|none, valid true|false|null
flowleap --json doctor       # providerKeys section + pending steps in nextSteps
```

`source` names the key a data call uses for that office: `user` (a forwarded
key), `stored` (the user's stored key), `server` (the backend's own keys) or
`none`. `keys test` needing nothing locally is fine when `source` is `stored`
or `server` — commands work without a local key.

`source` is that closed union and nothing else. When a provider was never
reached — an EPO failure short-circuits the USPTO check — its verdict carries
`source: null` with `checked: false` and `valid: null`, meaning "no verdict",
not "no key". Read `checked` before concluding anything about a provider. A
provider that is invalid or missing everywhere makes `keys test` exit **9**,
the same patent-data-key code a gated data command returns.

Doctor's `nextSteps` lists patent-data keys only when they actually **block**
work: providers with a stored key or server coverage produce no steps (doctor
reports the source per office in `keyValidation.providers`). A blocking
provider appears as an obtain/store pair — `obtain-epo-keys` /
`obtain-uspto-key` (`actor: "human"`, carries the signup `url` — relay it to
the user) then `store-epo-keys` / `store-uspto-key` (`actor: "agent"`, carries
the `run` command — execute it only when the user hands you the keys without
being asked; otherwise the human adds the key on the Patent-data keys page) —
followed by `verify-keys` (`actor: "agent"`, runs `keys test`). A stored key
the office rejected is different: doctor gives one `store-*` step with
`actor: "human"` and the page `url` ("Replace your … key"), then
`verify-keys`. A key-gate error names the same step ids with `actor: "human"`:
that is the page route, and doctor shows no step once the stored key is
present. When doctor cannot reach the validation endpoint
(unauthenticated/offline) it falls back to local key presence and says so in
`keyValidation.note`. See `flowleap-shared` for the
full `nextSteps`/`ready`/exit contract.

## Which codes mean gated

Exactly four backend codes, all from the closed error-code registry. They never
change once shipped, so they are the only safe thing to match on:

| Backend code | Status | Meaning |
|--------------|--------|---------|
| `data_keys_required` | 400 | The user pays, forwarded no patent-data key for this office, and the server fallback is denied. Carries `provider`. Never a 402 — this is not a billing problem |
| `patent_provider_key_invalid` | 400 | The user's own key was rejected upstream. Carries `provider`. Never a passthrough 401/403, which clients map to re-sign-in |
| `odp_api_key_missing` | 503 | No USPTO ODP key configured server-side and none forwarded |
| `trial_data_budget_exhausted` | 429 | Trial only (backend ADR 0017): today's SHARED trial data budget on FlowLeap's credentials is spent. Carries `provider` and `resets_at`, plus Retry-After. Lifts on its own at the next UTC day; the user's own free keys lift it permanently |

The CLI folds those four into one `providerKeysHint` on the JSON error
envelope, with its own `code` (`provider_keys_required`,
`provider_keys_invalid`, or `trial_budget_exhausted`) and the `provider`.
Either layer is a valid match; both are codes.

**The budget gate is the soft one.** `trial_budget_exhausted` follows the same
doctrine in `flowleap-shared` (never substitute scraped data, keys are free, ask the user),
with one extra exit: the hint carries `resetsAt`, so with the user's blessing an
agent may also pause the gated office until then instead of requiring keys now.
A success envelope may warn first: a `trial_data_budget_low` entry in
`body.warnings` (with `remaining` and `resets_at`) means the wall is near —
finish the current work, then surface the key ask.

**Never match on message text.** Backend wording is freely editable by policy, so
a reword must not be able to invent or erase a gate. An error whose *message*
mentions `EPO_CLIENT_ID` but whose *code* is something else is not a gate — and
a gate whose message changes tomorrow is still a gate. Read `error.code` (or
`providerKeysHint.code`); ignore the prose.

## The key-gate doctrine

The doctrine for a gated office (a user-action stop, never an exhausted route;
proceed, then ask; the keyless pivot; resume) is in `flowleap-shared`, section
"The key gate". It applies to every gate code below.

**Where the human adds the key (CLI and chat client).** The human adds the key on the FlowLeap
"Patent-data keys" page (https://www.flowleap.co/en/dashboard/keys) or in the
client's own key setting (`flowleap setup` / `flowleap keys set` for this CLI).
A stored key from that page is used whenever no forwarded key is present, so
one visit to the page opens the office on every surface. When the hint carries
`keysPageUrl` or a `nextStep`, give the human that link.

## The agent protocol — when keys are missing or rejected

Failed commands carry a `providerKeysHint` in the JSON error envelope, raised
from the four backend codes above and from nothing else (a
`trial_budget_exhausted` hint additionally carries `resetsAt`). Only while
the backend keeps stored keys, a `provider_keys_required` hint, and a
`provider_keys_invalid` hint when the rejected key is the stored key, also
carry `keysPageUrl` and a `nextStep` with `actor: "human"` — the same shape and
ids as doctor's `nextSteps`. A `trial_budget_exhausted` hint never carries
them. Relay the step's `title` and `url` to the human:

```json
"providerKeysHint": {
  "code": "provider_keys_required",      // or provider_keys_invalid / trial_budget_exhausted
  "provider": "epo",
  "requiresHumanIntervention": true,
  "nonInteractive": { "command": "flowleap keys set epo --key … --secret …",
                       "env": ["FLOWLEAP_EPO_KEY", "FLOWLEAP_EPO_SECRET"] },
  "signup": "https://developers.epo.org (free, 'My apps' → create app)",
  "keysPageUrl": "https://www.flowleap.co/en/dashboard/keys",
  "nextStep": { "id": "store-epo-keys", "actor": "human",
                "title": "Add your EPO OPS consumer key and secret on the FlowLeap Patent-data keys page",
                "url": "https://www.flowleap.co/en/dashboard/keys" }
}
```

**Getting keys requires a browser signup — an agent cannot complete this
alone. Do not retry, do not invent keys.** With the CLI, tell the user:

> This command needs EPO OPS credentials (free from https://developers.epo.org).
> Please add them on the FlowLeap Patent-data keys page
> (https://www.flowleap.co/en/dashboard/keys), or run `flowleap setup` in a
> terminal (guided, ~2 minutes), then I'll continue.

Do not ask the user to paste the key into the chat. If the user hands you keys
without being asked, apply them non-interactively — they are validated live
before saving, and rejected keys are NOT saved:

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
