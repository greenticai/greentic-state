use crate::error::invalid_input;
use crate::key::StatePath;
use greentic_types::{GResult, StateKey, TenantCtx};
use serde_json::Value;

/// JSON state store operations shared across backends.
pub trait StateStore: Send + Sync + 'static {
    /// Get the JSON value for `(tenant, prefix, key)`.
    /// When `path` is provided the returned value corresponds to that JSON Pointer.
    fn get_json(
        &self,
        tenant: &TenantCtx,
        prefix: &str,
        key: &StateKey,
        path: Option<&StatePath>,
    ) -> GResult<Option<Value>>;

    /// Set the JSON value for `(tenant, prefix, key)`.
    /// When `path` is provided the value is upserted at the JSON Pointer location.
    /// Passing `ttl_secs` refreshes the expiry; `None` keeps the existing TTL (if any),
    /// while `Some(0)` clears an existing TTL.
    fn set_json(
        &self,
        tenant: &TenantCtx,
        prefix: &str,
        key: &StateKey,
        path: Option<&StatePath>,
        value: &Value,
        ttl_secs: Option<u32>,
    ) -> GResult<()>;

    /// Delete the entire JSON value at `(tenant, prefix, key)`.
    /// Returns `true` when the key existed.
    fn del(&self, tenant: &TenantCtx, prefix: &str, key: &StateKey) -> GResult<bool>;

    /// Bulk delete all keys under `(tenant, prefix)` — used for flow cleanup, etc.
    /// Returns the number of entries removed.
    fn del_prefix(&self, tenant: &TenantCtx, prefix: &str) -> GResult<u64>;

    /// Atomically create `(tenant, prefix, key)` iff it is absent (an expired entry counts
    /// as absent).
    ///
    /// Returns `Ok(true)` when this call created the entry and `Ok(false)` when a live
    /// value already existed (it is left untouched). TTL rules mirror [`StateStore::set_json`]
    /// for a new key: `None` and `Some(0)` mean no deadline, `Some(n)` expires after `n` seconds.
    ///
    /// The default implementation returns an error: emulating this with get-then-set would
    /// be non-atomic and could silently let two callers both claim the key. Backends must
    /// override it with a genuinely atomic operation.
    fn set_json_if_absent(
        &self,
        tenant: &TenantCtx,
        prefix: &str,
        key: &StateKey,
        value: &Value,
        ttl_secs: Option<u32>,
    ) -> GResult<bool> {
        let _ = (tenant, prefix, key, value, ttl_secs);
        Err(invalid_input(
            "set_json_if_absent is not supported by this StateStore",
        ))
    }
}
