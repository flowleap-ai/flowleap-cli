# ADR 0002 — Subscription is a readiness blocker

## Status

Accepted (2026-07-26).

`flowleap doctor` must treat a missing subscription entitlement as blocking:
`ready` is false and `nextSteps` includes a human `subscribe-basic` step with
the backend-provided upgrade URL or the FlowLeap pricing fallback. An active
entitlement remains required in addition to reachability, authentication, and
provider readiness.

This keeps `doctor` as the single authority used by human and agent-mediated
onboarding. The alternative—leaving `doctor` unaware of subscriptions and
teaching the npm README prompt to probe a patent-data route—would produce two
different definitions of readiness and let other `doctor` consumers report
false readiness.

## Consequences

Machines that previously reported `ready: true` may correctly become not ready
after upgrading when their account lacks the required entitlement. The
`subscribe-basic` step id becomes part of the public next-step contract.

An entitlement that cannot be verified is `unknown`, not `inactive`: `ready`
is false and `nextSteps` carries an agent-owned `verify-subscription` step.
Only an authoritative inactive verdict may create the human
`subscribe-basic` step. This fails closed without telling an already-subscribed
user to purchase again.

`/api/profile` is the authoritative entitlement source. Its subscription
object returns a normalized `entitled` verdict, display status and plan, and
an optional human action such as `subscribe-basic` or `resolve-billing` with
the correct URL. Billing-provider states and grace-period policy remain backend
concerns; doctor renders the normalized verdict rather than duplicating that
policy or inferring entitlement by probing a patent-data route and interpreting
HTTP 402.
