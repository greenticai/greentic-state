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
None. The runtime authenticates the state door with the unit's metering token
and derives the door from the metering endpoint, so setup asks for nothing.

## Configuration
All optional: `endpoint`, `token_ref`, `key_prefix`, `default_ttl_seconds`,
`request_timeout_ms`, `cache_max_entries`, `stable_component_state`. Only
`stable_component_state` is a declared setup question (`assets/setup.yaml`). See `docs/state-sorla.md`.
