# Acceptance run 5: the hosted MCP server in claude.ai and ChatGPT, 2026-09-30

Issue: backend [#498](https://github.com/abdullahatrash/flowleap-backend/issues/498), gates 2 and 3 of the hosted MCP server rollout (spec [#489](https://github.com/abdullahatrash/flowleap-backend/issues/489)). Run 4 proved the hosted server with a personal token from Claude Code. This run proves the two consumer clients that the directories list: a custom connector on claude.ai (gate 2) and a developer-mode connector in ChatGPT (gate 3). Both use OAuth through Clerk, not a token.

**Overall verdict: PASS.** Both clients completed the OAuth sign-in on the founder's own account, listed the tools, and answered the same portfolio question from PATSTAT.

| Gate | Client | Sign-in | Question | Result |
|---|---|---|---|---|
| 2 | claude.ai custom connector (web app, Opus 5.5) | OAuth, Clerk consent screen | "use flowleap mcp to get siemens portfolio since 2016" | PASS: `patstat_portfolio` called; a bar chart of applications and grants by filing year 2016 to 2025 and a table by office (EP 9,391 applications, WO 8,508, US 4,610, ...) |
| 3 | ChatGPT developer-mode connector | OAuth, Clerk consent screen | Siemens portfolio since 2016 | PASS (2026-09-30 16:33): `patstat_portfolio` called; yearly counts per office; the PATSTAT edition named |

Evidence: screenshots kept by the founder (`gate2-claude-ai-portfolio.jpg`, `gate3-chatgpt-portfolio.jpg`), not committed.

## What the gates prove

- The OAuth path of ADR 0022 works in both clients: the protected-resource metadata, the authorization-server document proxied from Clerk, client ID metadata documents with DCR on, PKCE, the `patent-data` scope and the consent screen.
- The server info of #509 shows in both clients: the name "FlowLeap Patent AI" and the logo. Before #509 ChatGPT showed the connector without a logo (the founder's screenshot of 2026-09-30 16:33).
- The registry is the same 50 tools as the stdio bridge; claude.ai reported "Loaded tools, used flowleap integration".

## Transport

| Item | Value |
|---|---|
| Server | `https://api.flowleap.co/mcp`, stateless Streamable HTTP, JSON replies |
| Backend `apiVersion` at gate 2 | `1.0.0+6c28b07f` (main after #533) |
| `/v1/health` `mcp` | enabled, 50 tools, 6 resources, 3 prompts, OAuth issuer `https://clerk.flowleap.co` |
| `/v1/health` `storedKeys` | enabled, key id 1 (#514, #521) |

## Follow-ups seen on the way

- Before stored keys (#511), a paid user in ChatGPT could not use the office tools: the connector relayed `data_keys_required` with no place to go. Run 6 (`2026-09-30-stored-keys-run-6.md`) records the fix.
- Rate limits reach the agent as tool errors over the hosted server, where the stdio bridge waits in-band: backend #510.
