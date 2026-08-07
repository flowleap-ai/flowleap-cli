---
name: flowleap-auth
description: Authenticate the FlowLeap CLI — OAuth 2.0 device flow login (user code + verification URL), long-lived fl_pat_ personal API tokens for headless agents, status checks, and targeted logout. Trigger when a FlowLeap command fails with 401/unauthenticated, when setting up credentials for an agent or CI, or when the user asks to log in to FlowLeap or mint, list, or revoke API tokens.
---

# FlowLeap Auth

Global flags and configuration: see `flowleap-shared`.

## Commands

### Login via OAuth device flow

```bash
flowleap auth login
```

Starts the OAuth 2.0 device flow: the CLI requests a device code from the
backend, prints a user code plus verification URL (and opens the browser),
then polls until the login is approved. The resulting session JWT is stored
in `~/.config/flowleap/credentials.toml` (mode 0600). No local callback
server is involved, so it also works when the browser runs on another
machine — open the printed URL and enter the user code there. When stdout is
not a TTY, the browser auto-open and spinner are suppressed automatically.

### Structured login for agents (--json)

```bash
flowleap --json auth login
```

With `--json`, `auth login` becomes a blocking process that speaks NDJSON on
stdout — one compact JSON object per line, nothing else. It emits the
device-authorization event immediately, then polls until the human approves
and emits exactly one terminal event before exiting:

```json
{"event":"device_authorization","verification_uri":"https://flowleap.co/device","verification_uri_complete":"https://flowleap.co/device?code=ABCD-1234","user_code":"ABCD-1234","expires_in":900,"interval":5}
{"event":"authorized","stored":true}
```

Terminal events: `authorized` (exit 0 — the session token is stored, same as
the human flow) or `failed` with an `error` description (nonzero exit per the
standard exit-code table — denied, expired, or another error). Structured
mode has no side effects: no browser auto-open, no clipboard copy, no
spinner — the agent decides what to do with the URL.

Agent-mediated sign-in sequence:

1. Run `flowleap --json auth login` in the background and read the first
   NDJSON line (the `device_authorization` event).
2. Relay `verification_uri` and `user_code` to the human (or open
   `verification_uri_complete` yourself when running on the human's own
   machine).
3. Await the process's terminal event. `authorized` means the session token
   is stored and authenticated commands work immediately; `failed` means
   start over.
4. Session tokens expire. For durability, follow up with
   `flowleap --json auth create-token --name <name> --store` to mint and
   store a long-lived `fl_pat_` personal token.

### Login with a personal API token

```bash
flowleap auth login --api-key fl_pat_your_token_here
```

### Login with a session token

```bash
flowleap auth login --token eyJhbGci...
```

### Personal API tokens (headless/agent use)

```bash
flowleap auth create-token --name my-agent --store   # mint + store (shown once)
flowleap auth tokens                                 # list tokens
flowleap auth revoke-token <id>                      # revoke by id
```

Personal tokens use the `fl_pat_…` format and are long-lived. Minting
requires an OAuth session — API tokens cannot mint further tokens
(backend-enforced).

### Check Status

```bash
flowleap auth status
flowleap --json auth status
```

Shows base URL, credential source, default model, and — when the credential
works — the user profile. It **verifies** the credential against the backend
rather than reporting that one is merely stored, so an expired token is caught
here instead of at the next data command.

Branch on `verification.state` (or the exit code):

| State | Exit | What to do |
|-------|------|------------|
| `valid` | 0 | Proceed. |
| `rejected` | 3 | The credential is expired, revoked, or wrong. A human must re-run `flowleap auth login`; do not retry. |
| `absent` | 3 | No credential configured — see the login options above. |
| `unverified` | 7 | The backend could not be reached, so validity is **unknown**. Retry later; do NOT tell the user their credential is invalid. |

`verification.checked` is `true` only when a live check reached a verdict.
`unverified` means no conclusion was reached — treat it as "unknown", never as
a failure of the credential itself.

### Logout

```bash
flowleap auth logout                # clear everything, including patent-data keys
flowleap auth logout --session-only # clear only the OAuth session token
```

Plain `logout` clears all stored credentials, including EPO/USPTO provider
keys. Use `--session-only` to drop just the browser-session token — useful
when an expired session token is shadowing a still-valid `fl_pat_` API key
(the CLI prefers the session token when both are stored).

The CLI self-heals this case: on a 401 with a stored session token, it retries
once with the stored API key and prints a stderr warning suggesting
`logout --session-only`. The fallback is skipped when the token was passed
explicitly via `--token` or `FLOWLEAP_TOKEN`.

## Environment Variable Override

Set `FLOWLEAP_API_KEY` (an `fl_pat_…` token) or `FLOWLEAP_TOKEN` to bypass
stored credentials entirely. These take effect without running `auth login`.
