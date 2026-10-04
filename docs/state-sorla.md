# state-sorla: durable conversation state through a state door

`state-sorla` is a state provider for deployed workers whose conversation state
must survive an idle period, a restart or a redeploy. The runtime does not hold
a database credential. It calls an HTTPS **state door** hosted by the platform
(the admin) with a per-unit bearer token; tenant, environment and unit come from
the token, never from the request.

Like `state-redis`, `state-provider-sorla` is a **configuration component
only** (describe, QA setup/upgrade/remove, apply-answers, i18n). It opens no
sockets. The HTTP key-value backend that talks to the door lives in the host
(greentic-start / runner), which reads this provider's config.

## Configuration

| key | type | default | notes |
|---|---|---|---|
| `endpoint` | string | required | `https` base URL of the door, e.g. `https://admin.example/api/v1/ingest/state`. `http` is accepted only for `localhost`, `127.0.0.1` and `[::1]` (development). No credentials, query or fragment. |
| `token_ref` | string | required | The NAME of the secret holding the per-unit token. Never the token. A value starting with `gtm_` is refused as a pasted token. Redacted in describe output. |
| `key_prefix` | string | `greentic` | Prefix for every key. 1 to 128 chars, no whitespace. |
| `default_ttl_seconds` | u64, optional | none | At most ten years. Absent means the server's retention policy. |
| `request_timeout_ms` | u64 | `5000` | 100 to 60000. |
| `cache_max_entries` | u64 | `10000` | 1 to 10000000. Bounds the host read cache. |

Numeric answers may be JSON numbers or numeric strings; an unparsable value is a
validation error, never ignored. `upgrade` keeps every key not present in the
answers and re-validates the merged result, so an upgrade cannot downgrade the
endpoint to plain `http`.

## Door contract (binding)

`POST {endpoint}/read`, `POST {endpoint}/write`, `POST {endpoint}/delete`, with
`Authorization: Bearer <token>`. These match the three operations of
`greentic:state/state-store`: opaque blobs, no list, no append, no TTL, no
compare-and-swap. The request body and error mapping are owned by the admin;
see `docs/superpowers/plans/2026-10-04-state-sorla-durable-conversation-state.md`
in greentic-designer.

## Resolution priority

The pack offers `greentic.cap.state.kv.v1` at **priority 40**. Lower wins: it is
preferred over `state-redis` (50) and `state-memory` (100). Rationale: a
deployment that was given a state door must not silently land on memory or on a
shared Redis. A named `state-sorla` that cannot reach its door must fail the
boot or the turn; falling back to memory would reintroduce the data loss this
provider exists to remove.

## Secrets

The pack declares one tenant secret requirement, `state_door_token`. The
designer stages it; `token_ref` names it.
