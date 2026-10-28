use crate::error::{from_redis, from_serde, internal};
use crate::key::{FqnKey, StatePath, fqn, fqn_prefix};
use crate::store::StateStore;
use crate::util::{get_at_path, set_at_path};
use greentic_types::{GResult, StateKey, TenantCtx};
use parking_lot::Mutex;
use redis::{Commands, Connection, RedisResult, Script};
use serde_json::Value;
use tracing::debug;

const UPSERT_LUA: &str = r#"
local key = KEYS[1]
local payload = ARGV[1]
local ttl_ms = tonumber(ARGV[2])

if ttl_ms ~= nil and ttl_ms > 0 then
  redis.call("SET", key, payload, "PX", ttl_ms)
  return ttl_ms
end

if ttl_ms == 0 then
  redis.call("SET", key, payload)
  redis.call("PERSIST", key)
  return ttl_ms
end

local current_ttl = redis.call("PTTL", key)
if current_ttl > 0 then
  redis.call("SET", key, payload, "PX", current_ttl)
else
  redis.call("SET", key, payload)
end
return current_ttl
"#;

/// Redis-backed [`StateStore`] implementation.
pub struct RedisStateStore {
    client: redis::Client,
    connection: Mutex<Option<Connection>>,
    upsert_script: Script,
}

/// Whether `err` means the connection itself is unusable, as opposed to Redis
/// refusing a command. Only the first kind is cured by opening a new one.
fn connection_lost(err: &redis::RedisError) -> bool {
    err.is_connection_dropped() || err.is_io_error() || err.is_timeout()
}

impl RedisStateStore {
    /// Creates a store using an existing Redis client.
    pub fn new(client: redis::Client) -> Self {
        Self {
            client,
            connection: Mutex::new(None),
            upsert_script: Script::new(UPSERT_LUA),
        }
    }

    /// Builds a store by connecting to the provided Redis URL.
    pub fn from_url(redis_url: impl AsRef<str>) -> GResult<Self> {
        let client = redis::Client::open(redis_url.as_ref())
            .map_err(|err| from_redis(err, "connect redis"))?;
        Ok(Self::new(client))
    }

    /// Runs `f` on the cached connection, opening one when there is none.
    ///
    /// A connection that died (Redis restarted, failed over, or closed an idle
    /// socket) is dropped from the slot and the command is retried ONCE on a
    /// fresh one. Without that the dead connection stayed cached and every
    /// later command failed with `Broken pipe` after Redis had come back.
    ///
    /// The retry can repeat a command whose reply was lost after Redis ran it.
    /// Every command this store sends is safe to repeat except
    /// `set_json_if_absent`, where a lost reply can make the retry report that
    /// the key already existed; callers of that method must treat `false` as
    /// "someone wrote it", which is what it already means.
    fn with_connection<T>(
        &self,
        mut f: impl FnMut(&mut Connection) -> RedisResult<T>,
    ) -> GResult<T> {
        let mut guard = self.connection.lock();
        let mut retried = false;
        loop {
            if guard.is_none() {
                *guard = Some(
                    self.client
                        .get_connection()
                        .map_err(|err| from_redis(err, "connect redis"))?,
                );
            }
            let Some(conn) = guard.as_mut() else {
                return Err(internal("redis connection not initialized"));
            };
            match f(conn) {
                Ok(value) => return Ok(value),
                Err(err) if connection_lost(&err) => {
                    *guard = None;
                    if retried {
                        return Err(from_redis(err, "redis command"));
                    }
                    retried = true;
                    debug!(error = %err, "redis connection lost; reconnecting once");
                }
                Err(err) => return Err(from_redis(err, "redis command")),
            }
        }
    }

    fn load_document(&self, key: &FqnKey) -> GResult<Option<Value>> {
        let raw: Option<String> = self.with_connection(|conn| conn.get(key.as_ref()))?;
        let value = raw
            .map(|payload| serde_json::from_str(&payload).map_err(from_serde))
            .transpose()?;
        Ok(value)
    }

    fn ttl_arg(ttl_secs: Option<u32>) -> i64 {
        match ttl_secs {
            Some(0) => 0,
            Some(ttl) => i64::from(ttl) * 1_000,
            None => -1,
        }
    }

    fn write_document(&self, key: &FqnKey, document: &Value, ttl_secs: Option<u32>) -> GResult<()> {
        let payload = serde_json::to_string(document).map_err(from_serde)?;
        let ttl = Self::ttl_arg(ttl_secs);
        self.with_connection(|conn| {
            self.upsert_script
                .key(key.as_ref())
                .arg(payload.as_str())
                .arg(ttl)
                .invoke::<i64>(conn)
        })?;
        Ok(())
    }

    fn entry_key(&self, tenant: &TenantCtx, prefix: &str, key: &StateKey) -> FqnKey {
        fqn(tenant, prefix, key)
    }
}

impl StateStore for RedisStateStore {
    fn get_json(
        &self,
        tenant: &TenantCtx,
        prefix: &str,
        key: &StateKey,
        path: Option<&StatePath>,
    ) -> GResult<Option<Value>> {
        let fqn = self.entry_key(tenant, prefix, key);
        let document = match self.load_document(&fqn)? {
            Some(doc) => doc,
            None => return Ok(None),
        };

        if let Some(path) = path {
            Ok(get_at_path(&document, path).cloned())
        } else {
            Ok(Some(document))
        }
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
        let document = if let Some(path) = path {
            let mut base = self.load_document(&fqn)?.unwrap_or(Value::Null);
            set_at_path(&mut base, path, value.clone())?;
            base
        } else {
            value.clone()
        };

        self.write_document(&fqn, &document, ttl_secs)
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
        let payload = serde_json::to_string(value).map_err(from_serde)?;
        let px_ms = match ttl_secs {
            Some(ttl) if ttl > 0 => Some(u64::from(ttl) * 1_000),
            _ => None,
        };
        let reply: Option<String> = self.with_connection(|conn| {
            let mut cmd = redis::cmd("SET");
            cmd.arg(fqn.as_ref()).arg(payload.as_str()).arg("NX");
            if let Some(ms) = px_ms {
                cmd.arg("PX").arg(ms);
            }
            cmd.query(conn)
        })?;
        Ok(reply.is_some())
    }

    fn del(&self, tenant: &TenantCtx, prefix: &str, key: &StateKey) -> GResult<bool> {
        let fqn = self.entry_key(tenant, prefix, key);
        let removed: i64 =
            self.with_connection(|conn| redis::cmd("DEL").arg(fqn.as_ref()).query(conn))?;
        Ok(removed > 0)
    }

    fn del_prefix(&self, tenant: &TenantCtx, prefix: &str) -> GResult<u64> {
        let pattern = format!("{}*", fqn_prefix(tenant, prefix));
        let mut cursor = 0_u64;
        let mut deleted = 0_u64;

        self.with_connection(|conn| -> RedisResult<()> {
            loop {
                let (next, keys): (u64, Vec<String>) = redis::cmd("SCAN")
                    .arg(cursor)
                    .arg("MATCH")
                    .arg(&pattern)
                    .arg("COUNT")
                    .arg(512)
                    .query(conn)?;

                if !keys.is_empty() {
                    let removed: i64 = redis::cmd("DEL").arg(keys.clone()).query(conn)?;
                    deleted += removed as u64;
                }

                if next == 0 {
                    break;
                }

                cursor = next;
            }
            Ok(())
        })?;

        if deleted > 0 {
            debug!(prefix = pattern, deleted, "bulk deleted redis keys");
        }

        Ok(deleted)
    }
}
