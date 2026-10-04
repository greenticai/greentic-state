use crate::key::{FqnKey, StatePath, fqn, fqn_prefix};
use crate::store::StateStore;
use crate::util::{get_at_path, set_at_path};
use dashmap::DashMap;
use dashmap::mapref::entry::Entry;
use greentic_types::{GResult, StateKey, TenantCtx};
use serde_json::Value;
use std::sync::Arc;
use time::{Duration, OffsetDateTime};

/// In-memory state store backed by [`DashMap`].
#[derive(Default, Clone)]
pub struct InMemoryStateStore {
    entries: Arc<DashMap<String, StoredValue>>,
}

#[derive(Clone)]
struct StoredValue {
    value: Value,
    expires_at: Option<OffsetDateTime>,
}

impl StoredValue {
    fn new(value: Value, expires_at: Option<OffsetDateTime>) -> Self {
        Self { value, expires_at }
    }

    fn is_expired(&self, now: OffsetDateTime) -> bool {
        self.expires_at
            .map(|deadline| deadline <= now)
            .unwrap_or(false)
    }
}

impl InMemoryStateStore {
    /// Creates a new empty store.
    pub fn new() -> Self {
        Self::default()
    }

    fn entry_key(&self, tenant: &TenantCtx, prefix: &str, key: &StateKey) -> FqnKey {
        fqn(tenant, prefix, key)
    }

    fn compute_deadline(now: OffsetDateTime, ttl_secs: Option<u32>) -> Option<OffsetDateTime> {
        match ttl_secs {
            Some(0) => None,
            Some(ttl) => Some(now + Duration::seconds(ttl.into())),
            None => None,
        }
    }

    fn materialize_value(&self, fqn: &FqnKey, path: Option<&StatePath>) -> GResult<Option<Value>> {
        let now = OffsetDateTime::now_utc();
        let Some(entry) = self.entries.get(fqn.as_str()) else {
            return Ok(None);
        };
        if entry.is_expired(now) {
            drop(entry);
            self.entries.remove(fqn.as_str());
            return Ok(None);
        }

        let value = if let Some(path) = path {
            match get_at_path(&entry.value, path) {
                Some(segment) => segment.clone(),
                None => return Ok(None),
            }
        } else {
            entry.value.clone()
        };
        Ok(Some(value))
    }

    fn insert_new(
        &self,
        fqn: &FqnKey,
        path: Option<&StatePath>,
        value: &Value,
        ttl_secs: Option<u32>,
    ) -> GResult<()> {
        let now = OffsetDateTime::now_utc();
        let mut stored = if path.is_some() {
            Value::Null
        } else {
            value.clone()
        };

        if let Some(path) = path {
            set_at_path(&mut stored, path, value.clone())?;
        }

        let expires_at = Self::compute_deadline(now, ttl_secs);
        self.entries.insert(
            fqn.as_str().to_owned(),
            StoredValue::new(stored, expires_at),
        );
        Ok(())
    }
}

impl StateStore for InMemoryStateStore {
    fn get_json(
        &self,
        tenant: &TenantCtx,
        prefix: &str,
        key: &StateKey,
        path: Option<&StatePath>,
    ) -> GResult<Option<Value>> {
        let fqn = self.entry_key(tenant, prefix, key);
        self.materialize_value(&fqn, path)
    }

    fn set_json(
        &self,
        tenant: &TenantCtx,
        prefix: &str,
        key: &StateKey,
        path: Option<&StatePath>,
        value: &Value,
        ttl_secs: Option<u32>,
    ) -> GResult<()> {
        let fqn = self.entry_key(tenant, prefix, key);
        let now = OffsetDateTime::now_utc();

        match self.entries.entry(fqn.as_str().to_owned()) {
            Entry::Occupied(mut occupied) => {
                if occupied.get().is_expired(now) {
                    occupied.remove();
                    return self.insert_new(&fqn, path, value, ttl_secs);
                }

                let entry = occupied.get_mut();
                if let Some(ttl) = ttl_secs {
                    entry.expires_at = Self::compute_deadline(now, Some(ttl));
                }

                if let Some(path) = path {
                    set_at_path(&mut entry.value, path, value.clone())?;
                } else {
                    entry.value = value.clone();
                }
                Ok(())
            }
            Entry::Vacant(vacant) => {
                let mut stored = if path.is_some() {
                    Value::Null
                } else {
                    value.clone()
                };
                if let Some(path) = path {
                    set_at_path(&mut stored, path, value.clone())?;
                }
                let expires_at = Self::compute_deadline(now, ttl_secs);
                vacant.insert(StoredValue::new(stored, expires_at));
                Ok(())
            }
        }
    }

    fn set_json_if_absent(
        &self,
        tenant: &TenantCtx,
        prefix: &str,
        key: &StateKey,
        value: &Value,
        ttl_secs: Option<u32>,
    ) -> GResult<bool> {
        let fqn = self.entry_key(tenant, prefix, key);
        let now = OffsetDateTime::now_utc();
        let expires_at = Self::compute_deadline(now, ttl_secs);

        // The shard lock is held by the entry guard for the whole decision.
        match self.entries.entry(fqn.as_str().to_owned()) {
            Entry::Vacant(vacant) => {
                vacant.insert(StoredValue::new(value.clone(), expires_at));
                Ok(true)
            }
            Entry::Occupied(mut occupied) => {
                let entry = occupied.get_mut();
                if entry.is_expired(now) {
                    *entry = StoredValue::new(value.clone(), expires_at);
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
        }
    }

    fn del(&self, tenant: &TenantCtx, prefix: &str, key: &StateKey) -> GResult<bool> {
        let fqn = self.entry_key(tenant, prefix, key);
        Ok(self.entries.remove(fqn.as_str()).is_some())
    }

    fn del_prefix(&self, tenant: &TenantCtx, prefix: &str) -> GResult<u64> {
        let pattern = fqn_prefix(tenant, prefix);
        let keys: Vec<String> = self
            .entries
            .iter()
            .filter(|entry| entry.key().starts_with(&pattern))
            .map(|entry| entry.key().clone())
            .collect();

        let mut count = 0;
        for key in keys {
            if self.entries.remove(&key).is_some() {
                count += 1;
            }
        }
        Ok(count)
    }
}
