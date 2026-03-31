# Greentic State

Production-grade JSON working-memory store with pluggable backends for Greentic flows. The crate exposes a `StateStore` trait that supports whole-document operations as well as targeted updates using JSON Pointer paths. Implementations are provided for an in-memory store (suitable for single-node workers and tests) and Redis (for cross-node coordination).

## Design Overview

- **Tenant-aware namespace** – Keys are derived from `TenantCtx` + caller prefix + `StateKey`, ensuring strict separation between tenants and flow executions.
- **JSON-first API** – Values are `serde_json::Value` with optional JSON Pointer paths for partial reads and writes.
- **TTL semantics** – Stores honour per-record TTLs, propagating expirations on updates while allowing TTL refreshes.
- **Bulk operations** – Prefix deletion removes all keys under `(tenant, prefix)` for clean flow teardowns.
- **Feature-gated backends** – The `redis` backend is optional (`default` feature) and can be disabled for embedded scenarios.
- **Safety guarantees** – `#![forbid(unsafe_code)]`, lazy expiry in-memory, and Lua-assisted atomic upserts on Redis.

## Quickstart

```rust
use greentic_state::{inmemory::InMemoryStateStore, StateKey, StatePath, StateStore, TenantCtx};
use greentic_types::{EnvId, TenantId};
use serde_json::json;

let ctx = TenantCtx::new(
    EnvId::try_from("dev").expect("valid env id"),
    TenantId::try_from("tenant-123").expect("valid tenant id"),
);
let prefix = "flow/example";
let key = StateKey::new("node/state");
let store = InMemoryStateStore::new();

// Whole-document write with TTL (in seconds)
store.set_json(&ctx, prefix, &key, None, &json!({"status": "ready"}), Some(300))?;

// Partial update via JSON Pointer
let path = StatePath::from_pointer("/status");
store.set_json(&ctx, prefix, &key, Some(&path), &json!("running"), None)?;

let current = store.get_json(&ctx, prefix, &key, None)?;
assert_eq!(current.unwrap(), json!({"status": "running"}));
```

### Redis backend

```rust
use greentic_state::redis_store::RedisStateStore;
use redis::Client;

let client = Client::open("redis://127.0.0.1/")?;
let store = RedisStateStore::new(client);
```

To run Redis locally:

```bash
docker run --rm -p 6379:6379 redis:7
export REDIS_URL=redis://127.0.0.1/
```

Run tests with Redis enabled:

```bash
cargo test --all-features
```

## Tenant Prefixing & FQN

Fully-qualified keys are generated via:

```
greentic:state:{env}.{tenant_id}[.{team}(.{user})]:{prefix}:{state_key}
```

Only the generated FQN should be used within backends. `StatePath` helpers understand a subset of RFC 6901 JSON Pointers (array indices and object keys).

## TTL & Expiration

- **In-memory store** stores the deadline alongside the value. Expiration is enforced lazily on read/write and during re-insertion.
- **Redis store** reuses Redis native TTLs. A Lua upsert script preserves existing TTLs when `ttl_secs` is `None`, resets the TTL when a value is provided, and clears TTL when `ttl_secs == Some(0)`.

## Partial Updates

`set_json` with a `StatePath` performs read-modify-write:

1. Fetch or create the base JSON document (`Value::Null` by default).
2. Navigate/allocate intermediate objects or arrays based on the path segments.
3. Write the new value at the target pointer, ensuring intermediate containers exist.
4. Persist the mutated document while preserving TTL semantics.

## Bulk Deletion

Use `del_prefix` to drop all keys under a namespace:

```rust
let removed = store.del_prefix(&ctx, "flow/example")?;
println!("Removed {removed} keys");
```

Redis uses `SCAN` + batched `DEL`, avoiding blocking the server on large keyspaces.

## Development & CI

- `cargo fmt --all`
- `cargo clippy -p greentic-state -p provider-common --all-targets --all-features -- -D warnings`
- `cargo test -p greentic-state -p provider-common --all-features`
- `bash ./tools/build_state_packs.sh`
- GitHub Actions workflows:
  - `auto-tag.yml`: tags crates on version bumps merged to `master`.
  - `ci.yml`: host-crate checks plus a dedicated `state-packs` job that builds `state-memory.gtpack` and `state-redis.gtpack`.
  - `publish.yml`: crates.io publish for the Rust crate plus GHCR publish for the two state `.gtpack` artifacts.

### State packs

This repo now owns the source and publishing pipeline for:

- `state-memory.gtpack`
- `state-redis.gtpack`

The relevant directories are:

- `components/state-provider-memory`
- `components/state-provider-redis`
- `packs/state-memory`
- `packs/state-redis`

The state pack build/publish entrypoints are:

- `bash ./tools/build_state_packs.sh`
- `bash ./tools/publish_state_packs_oci.sh`

### Local checks

Run `ci/local_check.sh` to mirror the main GitHub Actions pipeline before pushing. It handles fmt/clippy/build/tests (spinning up a disposable Redis via Docker when `REDIS_URL` is unset), dependency gate checks, and an optional `cargo publish --dry-run`. Useful toggles:

- `LOCAL_CHECK_ONLINE=1` – enable networked steps such as the publish dry run.
- `LOCAL_CHECK_STRICT=1` – fail when optional tools are missing instead of skipping.
- `LOCAL_CHECK_SKIP_REDIS=1` – skip Redis provisioning and the Redis-backed test step.
- `greentic-pack` – required for the state `.gtpack` build step in `ci/local_check.sh`.
- `LOCAL_CHECK_VERBOSE=1` – echo each command (`set -x`).
- `LOCAL_CHECK_BYPASS=1` – skip the `git pre-push` hook that calls the script.

By default the script runs offline and skips checks whose tooling is unavailable, matching CI behavior as closely as possible without requiring secrets.

## Stability & Maintenance

The crate follows semantic versioning. Publishing is tag-driven and idempotent—rerunning publish on the same version is a no-op. Performance considerations include zero-copy JSON navigation and Redis-side Lua scripts for atomic updates. Contributions should keep shared types in `greentic-types` and WIT bindings in `greentic-interfaces`.
