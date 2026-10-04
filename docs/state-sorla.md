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

Every key is optional; a pack with zero answers is valid. Defaults and bounds
match the runtime (greentic-start `src/sorla_state/config.rs`).

| key | type | default | notes |
|---|---|---|---|
| `endpoint` | string | none | Explicit `https` door base URL. When empty the runtime derives the door from the unit's metering endpoint. A present value must be https; `http` is accepted only for `localhost`, `127.0.0.1` and `[::1]`. No credentials, query or fragment. |
| `token_ref` | string | none | Optional NAME of a secret. Never the token. The runtime normally ignores it and uses the metering token. A value starting with `gtm_` or containing whitespace is refused. Redacted in describe output. |
| `key_prefix` | string | `greentic-state` | No control characters. |
| `default_ttl_seconds` | u64 | none | `0` or absent means no expiry. At most `u32::MAX`. |
| `request_timeout_ms` | u64 | `5000` | 100 to 60000. |
| `cache_max_entries` | u64 | `1024` | 0 to 100000. `0` disables the read cache. |
| `stable_component_state` | bool | `false` | When on, component state (keys not starting `pack/`) is keyed per environment instead of per revision, so it survives a redeploy; flow state stays per revision (greentic-start#675). The only key the pack's `setup.yaml` DECLARES, since 1.2.0-dev.1: greentic-setup writes a pack-config entry only for declared questions. |

Numeric answers may be JSON numbers or numeric strings; an unparsable value is a
validation error. `upgrade` keeps keys not present in the answers and
re-validates the merged result, so it cannot downgrade the endpoint to http.

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

None. The pack declares no secret requirement, so greentic-setup shows no
unanswered required field. The credential is the unit's `metering` token.

## Setup does not emit a pack-config without an answer

greentic-setup writes `state/pack-configs/<pack_id>.json` from the answers the
wizard collected (`emit_pack_config_input`, `src/qa/persist.rs`), not from the
defaults apply-answers would produce, and it writes nothing when no answer has
a value. With zero answers there is no `state/pack-configs/state-sorla.json`.
A runtime that selects the provider only from a non-empty pack-config must
therefore either also select it on the pack's presence, or the operator must
answer at least one question (e.g. `key_prefix`).
