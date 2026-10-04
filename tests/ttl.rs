use greentic_state::{StateKey, StateStore, TenantCtx, inmemory::InMemoryStateStore};
use greentic_types::{EnvId, TenantId};
use serde_json::json;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

fn ctx() -> TenantCtx {
    TenantCtx::new(
        EnvId::try_from("dev").expect("valid env id"),
        TenantId::try_from("tenant").expect("valid tenant id"),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn in_memory_ttl_expires() {
    let store = InMemoryStateStore::new();
    let ctx = ctx();
    let prefix = "flow/ttl-in-memory";
    let key = StateKey::new("node/a");

    store
        .set_json(&ctx, prefix, &key, None, &json!({"ttl": true}), Some(1))
        .expect("set");

    sleep(Duration::from_millis(1_100)).await;

    let value = store.get_json(&ctx, prefix, &key, None).expect("get");
    assert!(value.is_none(), "expected value to expire");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn in_memory_ttl_preserved_on_none_update() {
    let store = InMemoryStateStore::new();
    let ctx = ctx();
    let prefix = "flow/ttl-preserve";
    let key = StateKey::new("node/a");

    store
        .set_json(&ctx, prefix, &key, None, &json!({"ttl": true}), Some(1))
        .expect("set");

    sleep(Duration::from_millis(600)).await;

    store
        .set_json(&ctx, prefix, &key, None, &json!({"ttl": "still"}), None)
        .expect("update");

    sleep(Duration::from_millis(500)).await;

    let value = store.get_json(&ctx, prefix, &key, None).expect("get");
    assert!(value.is_none(), "expected TTL to be preserved");
}

#[cfg(feature = "redis")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn redis_ttl_expires_when_configured() {
    use greentic_state::redis_store::RedisStateStore;
    use std::env;

    let url = match env::var("REDIS_URL") {
        Ok(url) => url,
        Err(_) => return,
    };
    let store = match RedisStateStore::from_url(&url) {
        Ok(store) => store,
        Err(_) => return,
    };

    let ctx = ctx();
    let prefix = format!("flow/ttl-redis-{}", Uuid::new_v4());
    let key = StateKey::new("node/a");

    store
        .set_json(&ctx, &prefix, &key, None, &json!({"redis": true}), Some(1))
        .expect("set redis ttl");

    sleep(Duration::from_millis(1_100)).await;

    let value = store
        .get_json(&ctx, &prefix, &key, None)
        .expect("get redis TTL");
    assert!(value.is_none(), "expected redis TTL to expire value");
}

#[cfg(feature = "redis")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn redis_ttl_preserved_on_none_update() {
    use greentic_state::redis_store::RedisStateStore;
    use std::env;

    let url = match env::var("REDIS_URL") {
        Ok(url) => url,
        Err(_) => return,
    };
    let store = match RedisStateStore::from_url(&url) {
        Ok(store) => store,
        Err(_) => return,
    };

    let ctx = ctx();
    let prefix = format!("flow/ttl-preserve-{}", Uuid::new_v4());
    let key = StateKey::new("node/a");

    store
        .set_json(&ctx, &prefix, &key, None, &json!({"ttl": true}), Some(1))
        .expect("set redis ttl");

    sleep(Duration::from_millis(600)).await;

    store
        .set_json(&ctx, &prefix, &key, None, &json!({"ttl": "still"}), None)
        .expect("update redis");

    sleep(Duration::from_millis(500)).await;

    let value = store
        .get_json(&ctx, &prefix, &key, None)
        .expect("get redis TTL");
    assert!(value.is_none(), "expected redis TTL to be preserved");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn in_memory_expired_entry_can_be_reclaimed_if_absent() {
    let store = InMemoryStateStore::new();
    let ctx = ctx();
    let prefix = "flow/ttl-reclaim";
    let key = StateKey::new("node/a");

    assert!(
        store
            .set_json_if_absent(&ctx, prefix, &key, &json!({"n": 1}), Some(1))
            .expect("create")
    );
    assert!(
        !store
            .set_json_if_absent(&ctx, prefix, &key, &json!({"n": 2}), Some(1))
            .expect("live")
    );

    sleep(Duration::from_millis(1_100)).await;

    assert!(
        store
            .set_json_if_absent(&ctx, prefix, &key, &json!({"n": 3}), None)
            .expect("reclaim")
    );
    let value = store.get_json(&ctx, prefix, &key, None).expect("get");
    assert_eq!(value, Some(json!({"n": 3})));
}

#[cfg(feature = "redis")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn redis_expired_entry_can_be_reclaimed_if_absent() {
    use greentic_state::redis_store::RedisStateStore;
    use std::env;

    let Ok(url) = env::var("REDIS_URL") else {
        return;
    };
    let Ok(store) = RedisStateStore::from_url(&url) else {
        return;
    };
    let ctx = ctx();
    let prefix = format!("flow/ttl-reclaim-{}", Uuid::new_v4());
    let key = StateKey::new("node/a");

    assert!(
        store
            .set_json_if_absent(&ctx, &prefix, &key, &json!(1), Some(1))
            .expect("create")
    );
    sleep(Duration::from_millis(1_100)).await;
    assert!(
        store
            .set_json_if_absent(&ctx, &prefix, &key, &json!(2), None)
            .expect("reclaim")
    );
}
