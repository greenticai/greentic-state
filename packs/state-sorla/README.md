# State SoRLa Pack

Durable key-value state provider. Conversation state is stored through an HTTPS
state door hosted by the platform (the admin). The deployed runtime holds a
per-unit bearer token and never a database credential.

## Pack ID
- `state-sorla`

## Components
- `state-provider-sorla`

## Capability offer
- `greentic.cap.state.kv.v1`, priority `40` (preferred over `state-redis` at 50
  and `state-memory` at 100; lower wins).

## Secrets
- `state_door_token` (tenant): per-unit bearer token for the state door. The
  provider config's `token_ref` holds the NAME of this secret, never the value.

## Configuration
`endpoint`, `token_ref`, `key_prefix`, `default_ttl_seconds`,
`request_timeout_ms`, `cache_max_entries`. See `docs/state-sorla.md`.
