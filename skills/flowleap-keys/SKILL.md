---
name: flowleap-keys
description: The Patent-Data Key doctrine for the FlowLeap CLI. No user needs a Patent-Data Key (EPO OPS consumer key/secret, USPTO ODP API key); a key is optional and lifts the daily limits for that office. Covers the publication index, the live data budget, dated answers, "not now" answers, key status and validation, and the key-gate fallback for a server that still answers data_keys_required. Trigger when a FlowLeap command fails with provider_keys_required, provider_keys_invalid or trial_budget_exhausted, when an answer carries live false or a liveUnavailableHint, when the daily budget is spent, or when the user asks to configure provider keys.
user-invocable: false
---

# FlowLeap Patent-Data Keys

In a chat client with the FlowLeap connector or in the FlowLeap Patent AI app, call the tools named in the `flowleap-shared` tool table instead of these commands, and never run a `flowleap` command there. No tool replaces the keys commands. In a chat client the user adds a stored key on the Patent-data keys page; in the app the firm administrator or the desktop user sets the keys (see `flowleap-shared`). Never ask for the key value in the chat.

## The rule: no user needs a Patent-Data Key

FlowLeap serves patent data in three layers (backend ADR 0026). Pick the layer
first.

| Layer | Question | Command | Key and quota |
|---|---|---|---|
| Discovery | Find candidates: "which patents are about X" | `flowleap patent search --provider index` (the **publication index**) | No key, no office quota, no daily budget |
| Analytics | Count and rank: "how many", "who leads", "trend since" | `flowleap patstat …` | No key |
| Live documents | This document now: text, claims, legal status, register, file wrapper | `flowleap ops …`, `flowleap uspto …`, the EPO and USPTO searches | The **House keys**, inside the **live data budget**; or the user's own key |

- **No user needs a Patent-Data Key.** A trialing user and a License holder get
  the same product. A live office read uses FlowLeap's House keys.
- **A key is optional.** The user's own free key for an office (a forwarded key
  or a stored key) lifts the daily limits for that office, and gives the user
  an own office quota. Say this one time, when a limit stops the work. Do not
  ask for a key before a limit stops the work.
- **Never ask for the key value in the chat.** Give the human the Patent-data
  keys page link or the local setup command.
- **The FlowLeap Patent AI app is different.** The app runs the office tools on
  the data keys of the firm or of the desktop user, calls no House keys and has
  no publication index. The key rules of `flowleap-shared` apply there.

A server can be older than this rule, or can have keyless access off. Then
`--provider index` is refused (the CLI prints "The publication index is not
enabled on this server"), and a License user without a key gets the key gate
`provider_keys_required`. Both are served answers that you read, not states
that you guess. The fallback for each is in this skill: use the EPO search, and
apply "The key-gate fallback" below.

## Discovery first

Start a search in the publication index. Then add a live search for what the
index cannot hold.

1. Search the index with plain words and filters (see `flowleap-patent`):
   `flowleap --json patent search --provider index --query '"solid state" battery' --cpc H01M`.
2. Read `asOf` in the answer. It is the edition date. A publication newer than
   `asOf` is not in the index.
3. Add an EPO search for the publications after `asOf`:
   `flowleap --json patent search --query 'ti="solid state battery" and pd>=<asOf as YYYYMMDD>'`.
4. Read each candidate that matters with the live document commands.
5. When you show index results, give the credit in `attribution.text` (the data
   is CC BY 4.0).

The index holds the number, dates, kind, English title and abstract, CPC and
IPC, applicants, inventors and simple family of each publication. It does not
hold claims, legal status or citations. If the server refuses
`--provider index`, do the same search with the EPO search only, and tell the
user that the index is not available on this server.

## The live data budget

Each user has a daily budget of live office calls on the House keys:

- 300 live calls for each office (EPO, USPTO) in one UTC day;
- 50 drawing pages in one UTC day;
- a cache hit, an index search, a PATSTAT call and a call on the user's own
  key cost nothing.

The budget resets at the next UTC midnight. To use it well: search in the
index, count in PATSTAT, and use live calls for the documents that you read.

When the budget is spent, or when the shared quota of the House keys is busy,
the backend gives one of two answers.

### A dated answer

`ops legal`, `ops biblio`, `ops abstract` and `ops family` answer from
FlowLeap's hosted data when the live call cannot run. The answer has:

```json
{ "live": false, "source": "patstat", "asOf": "2026-10-01",
  "liveUnavailable": { "reason": "live_budget_spent", "retryAfterSeconds": 18720 } }
```

- `live: true` with `source: "office"` is the answer of the office now.
- `live: false` is a **dated answer**. `source` is `patstat` (legal status,
  family) or `publication_index` (bibliography, abstract). `asOf` is the date
  for which the copy is true.
- `liveUnavailable.reason` is `shared_quota_busy` or `live_budget_spent`.
  `retryAfterSeconds` is the wait before the live call can run.
- Table and human output print these fields as `note:` lines on stderr. JSON
  output has the fields unchanged.

Rules for a dated answer:

- Tell the user the date: "last known status as of `<asOf>`".
- **A dated legal status is never proof that a right is in force.** A lapse, a
  revocation or a missed fee after `asOf` is not in it. For a
  freedom-to-operate, validity or infringement conclusion, get the live answer
  (`live: true`): wait `retryAfterSeconds`, then run the command again. Until
  then, record the status as not confirmed.
- A dated bibliography has no priority claims and no cited references. A dated
  family has publication rows only.
- An answer without `live` comes from an older server. It is a live answer.

### A "not now" answer

EP and WO text, the EP Register, drawings and the US file wrapper have no
hosted copy. When the live call cannot run, the command fails and the JSON
envelope has a `liveUnavailableHint`:

```json
"liveUnavailableHint": {
  "code": "live_unavailable",
  "reason": "live_budget_spent",        // or shared_quota_busy
  "retryAfterSeconds": 18720,
  "provider": "epo",
  "resetsAt": "2026-11-23T00:00:00.000Z",
  "requiresHumanIntervention": false,
  "keysPageUrl": "https://www.flowleap.co/en/dashboard/keys"
}
```

The backend codes are `live_data_budget_exhausted` (429, exit 6, a License
holder), `trial_data_budget_exhausted` (429, exit 9, a trial; the envelope also
has a `providerKeysHint` with the code `trial_budget_exhausted`) and
`upstream_unavailable` with `shared_quota` (503, with a `sharedQuotaHint`).

What to do:

1. Do not send the same call again before `retryAfterSeconds`. A call sent
   early fails again.
2. `shared_quota_busy` is a short wait (about one minute). Wait, then continue.
3. `live_budget_spent` is a long wait (to the next UTC midnight). Continue with
   the work that needs no live call for that office: the index, PATSTAT, the
   other office, `academic` and `npl`, and the dated answers.
4. Tell the user one time, at the end of the turn: what is not done, when the
   budget resets (`resetsAt`), and that an own free key for that office lifts
   the limit. Give the `keysPageUrl`. Do not ask for the key value.
5. In the deliverable, write each step that did not run as an open step with
   its reason. Do not write it as "no result".
6. Do not replace the live office data with web-scraped patent data.

## The two kinds of key

A Patent-Data Key is a credential that the USER holds at a patent office: the
EPO OPS consumer key and secret (always a pair), or the USPTO ODP API key. Each
office gives it free of charge after a browser signup. A key reaches the
backend in one of two ways:

- **Forwarded key** — kept on this machine in `credentials.toml` (0600) and
  sent with each request as `x-epo-ops-key` / `x-epo-ops-secret` /
  `x-uspto-odp-key`. The CLI never prints it (verbose/dry-run redact). A
  forwarded key always wins for its office.
- **Stored key** — the human adds it once on the FlowLeap "Patent-data keys"
  page (https://www.flowleap.co/en/dashboard/keys). FlowLeap keeps it
  encrypted and never shows the value again. It is used when the request
  carries no forwarded key for that office.

For one office the order is: forwarded key, stored key, House keys. A call on
the user's own key does not use the live data budget. Key validation
(`keys test`, `keys set`) runs outside the tools facade and works before a
subscription exists, so setup can always be diagnosed.

## Diagnose

```bash
flowleap --json keys list    # what's configured locally (masked); alias: keys status
flowleap --json keys test    # live verdicts: source user|stored|server|none, valid true|false|null
flowleap --json doctor       # providerKeys section + pending steps in nextSteps
```

`source` names the key a data call uses for that office: `user` (a forwarded
key), `stored` (the user's stored key), `server` (FlowLeap's House keys) or
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

## The key-gate fallback (a server that answers `data_keys_required`)

An older server, or a server with keyless access off, gives a License user
without a key the code `data_keys_required`. A server that has no House key for
an office does the same. The doctrine below applies then, unchanged.

### Which codes mean gated

Exactly four backend codes, all from the closed error-code registry. They never
change once shipped, so they are the only safe thing to match on:

| Backend code | Status | Meaning |
|--------------|--------|---------|
| `data_keys_required` | 400 | No forwarded key and no stored key for this office, and the server does not serve its House key to this user (an older server, keyless access off, or no House key for the office). Carries `provider`. Never a 402 — this is not a billing problem |
| `patent_provider_key_invalid` | 400 | The user's own key was rejected upstream. Carries `provider`. Never a passthrough 401/403, which clients map to re-sign-in |
| `odp_api_key_missing` | 503 | No USPTO ODP key configured server-side and none forwarded |
| `trial_data_budget_exhausted` | 429 | Trial only: today's data budget on FlowLeap's House keys is spent. Carries `provider` and `resets_at`, plus Retry-After. Lifts on its own at the next UTC day; the user's own free keys lift it permanently. With keyless access on, it also carries `liveUnavailable`: apply the section on a "not now" answer above |

The CLI folds those four into one `providerKeysHint` on the JSON error
envelope, with its own `code` (`provider_keys_required`,
`provider_keys_invalid`, or `trial_budget_exhausted`) and the `provider`.
Either layer is a valid match; both are codes.

**The budget gate is the soft one.** `trial_budget_exhausted` is a wait, not a
missing key: the hint carries `resetsAt`, the office opens again at that time,
and an own free key is optional. Never substitute scraped data for the office.
Apply the section on a "not now" answer above.
A success envelope may warn first: a `trial_data_budget_low` entry in
`body.warnings` (with `remaining` and `resets_at`) means the wall is near —
finish the current work, then surface the key ask.

**Never match on message text.** Backend wording is freely editable by policy, so
a reword must not be able to invent or erase a gate. An error whose *message*
mentions `EPO_CLIENT_ID` but whose *code* is something else is not a gate — and
a gate whose message changes tomorrow is still a gate. Read `error.code` (or
`providerKeysHint.code`); ignore the prose.

### The key-gate doctrine

**A key gate is a USER-ACTION STOP, never an exhausted route.** A gate means
that office needs a key only the user can obtain — it is not a transient error,
not a zero-result, and not a route you have exhausted. So for the gated office:

- **Never substitute web-scraped data for it** — not for searches, and not for
  single-document reads. "Give me the claims of EP…" with no EPO OPS key is
  declined *for that office* with the key named as the one-step fix; it is never
  quietly served from Google Patents, Espacenet, freepatentsonline, or a web
  search instead. Only the user adding the key opens that office.
- **The keys are FREE** from each office (EPO OPS and USPTO ODP both issue them
  at no cost, browser signup). Never frame the ask as a paywall, an upsell, or a
  FlowLeap limitation.
- **A gate is READ, never INFERRED.** An office is gated only when a command you
  actually ran returned an explicit gate **code** (the exact set is in
  "Which codes mean gated" above). Never conclude a gate from an empty result
  set, a truncated or partial payload, a 5xx, a timeout, from an error message
  that happens to name a credential, or from `keys list` showing nothing.
  Anything short of those codes is an ordinary dead or empty route, and normal
  persistence and fallbacks apply to it in full — reformulate, try the alternate
  office or route, then the usual web fallback.
- The forbid rule covers **only** an office gated on a missing patent-data key.
  Offices with no backend route at all (CN/JP/KR) and routes that are genuinely
  dead or empty *with a working key* keep their existing fallbacks unchanged.

**Where the human adds the key (CLI and chat client).** The human adds the key on the FlowLeap
"Patent-data keys" page (https://www.flowleap.co/en/dashboard/keys) or in the
client's own key setting (`flowleap setup` / `flowleap keys set` for this CLI).
A stored key from that page is used whenever no forwarded key is present, so
one visit to the page opens the office on every surface. When the hint carries
`keysPageUrl` or a `nextStep`, give the human that link.

**Never ask for the key value in the chat.** A key typed into a chat reaches
the transcript and every log that keeps it. Give the human the page link or
the local setup command instead; the human types the key there.

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

**Pivot to data that needs no key — offer it as DIFFERENT data, never as a
substitute.** These need no patent-data key and stay live while an office is
gated; label each for what it actually is:

- `flowleap patent search --provider index` (when the server has the
  publication index) — one dated edition of titles and abstracts. It finds
  candidates; it does not give claims, legal status or the newest publications.

- `flowleap patstat …` (portfolio, guarded SQL, graph) — aggregates from a
  twice-yearly SNAPSHOT, not documents and not current. It does not answer
  "what prior art exists for this claim".
- Patent LAW (MPEP, EPC, guidelines) comes from the office's own page, read
  with your web tool (`flowleap-legal`), not from a FlowLeap key. It gives the
  legal standard, never what has been published or filed.
- `flowleap academic search …` / `flowleap npl …` — scholarly LITERATURE. Papers
  are prior art in their own right, but they are a different corpus from patents.

Say plainly that this is different data, not a stand-in for the gated office's
live search.

**Resume — merge, do not restart.** When the user says they added the missing
key, re-run ONLY the previously gated office and merge its results into the
deliverable you already produced. Do not redo the live office's work. Keys reach
the request headers on the next invocation: no restart, no new session.

### The agent protocol — when keys are missing or rejected

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
