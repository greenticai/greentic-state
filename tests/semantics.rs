use greentic_state::{StateKey, StateStore, TenantCtx, inmemory::InMemoryStateStore};
use greentic_types::{EnvId, TenantId};
use serde_json::json;
use uuid::Uuid;

fn ctx(tenant: &str) -> TenantCtx {
    TenantCtx::new(
        EnvId::try_from("dev").expect("valid env id"),
        TenantId::try_from(tenant).expect("valid tenant id"),
    )
}

#[test]
fn in_memory_delete_removes_entry() {
    let store = InMemoryStateStore::new();
    let ctx = ctx("tenant-a");
    let prefix = "flow/delete";
    let key = StateKey::new("node/a");

    store
        .set_json(&ctx, prefix, &key, None, &json!({"a": 1}), None)
        .expect("set");

    let removed = store.del(&ctx, prefix, &key).expect("delete");
    assert!(removed, "expected delete to return true for existing key");

    let value = store.get_json(&ctx, prefix, &key, None).expect("get");
    assert!(value.is_none(), "expected deleted key to be gone");

    let removed_again = store.del(&ctx, prefix, &key).expect("delete");
    assert!(
        !removed_again,
        "expected delete to return false for missing key"
    );
}

#[test]
fn in_memory_prefix_delete_is_tenant_scoped() {
    let store = InMemoryStateStore::new();
    let ctx_a = ctx("tenant-a");
    let ctx_b = ctx("tenant-b");
    let prefix = "flow/shared";
    let key = StateKey::new("node/a");

    store
        .set_json(&ctx_a, prefix, &key, None, &json!({"a": 1}), None)
        .expect("set a");
    store
        .set_json(&ctx_b, prefix, &key, None, &json!({"b": 2}), None)
        .expect("set b");

    let removed = store.del_prefix(&ctx_a, prefix).expect("delete prefix");
    assert_eq!(removed, 1, "expected only tenant-a entries removed");

    let still_there = store.get_json(&ctx_b, prefix, &key, None).expect("get");
    assert!(still_there.is_some(), "expected tenant-b entry to remain");
}

#[cfg(feature = "redis")]
#[test]
fn redis_prefix_delete_is_tenant_scoped() {
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

    let ctx_a = ctx("tenant-a");
    let ctx_b = ctx("tenant-b");
    let prefix = format!("flow/shared-{}", Uuid::new_v4());
    let key = StateKey::new("node/a");

    store
        .set_json(&ctx_a, &prefix, &key, None, &json!({"a": 1}), None)
        .expect("set a");
    store
        .set_json(&ctx_b, &prefix, &key, None, &json!({"b": 2}), None)
        .expect("set b");

    let removed = store.del_prefix(&ctx_a, &prefix).expect("delete prefix");
    assert_eq!(removed, 1, "expected only tenant-a entries removed");

    let still_there = store.get_json(&ctx_b, &prefix, &key, None).expect("get");
    assert!(still_there.is_some(), "expected tenant-b entry to remain");
}

#[test]
fn in_memory_set_if_absent_creates_once() {
    let store = InMemoryStateStore::new();
    let ctx = ctx("tenant-a");
    let key = StateKey::new("node/a");

    assert!(
        store
            .set_json_if_absent(&ctx, "flow/ifabsent", &key, &json!({"v": 1}), None)
            .expect("first")
    );
    assert!(
        !store
            .set_json_if_absent(&ctx, "flow/ifabsent", &key, &json!({"v": 2}), None)
            .expect("second")
    );
    let value = store
        .get_json(&ctx, "flow/ifabsent", &key, None)
        .expect("get");
    assert_eq!(value, Some(json!({"v": 1})));
}

#[test]
fn in_memory_set_if_absent_is_scoped_by_tenant_and_prefix() {
    let store = InMemoryStateStore::new();
    let key = StateKey::new("node/a");
    let v = json!(1);

    assert!(
        store
            .set_json_if_absent(&ctx("tenant-a"), "p1", &key, &v, None)
            .expect("a/p1")
    );
    assert!(
        store
            .set_json_if_absent(&ctx("tenant-b"), "p1", &key, &v, None)
            .expect("b/p1")
    );
    assert!(
        store
            .set_json_if_absent(&ctx("tenant-a"), "p2", &key, &v, None)
            .expect("a/p2")
    );
}

#[test]
fn in_memory_set_if_absent_ttl_zero_never_expires() {
    let store = InMemoryStateStore::new();
    let ctx = ctx("tenant-a");
    let key = StateKey::new("node/a");

    assert!(
        store
            .set_json_if_absent(&ctx, "flow/zero", &key, &json!(1), Some(0))
            .expect("create")
    );
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert!(
        !store
            .set_json_if_absent(&ctx, "flow/zero", &key, &json!(2), None)
            .expect("again")
    );
}

#[test]
fn in_memory_set_if_absent_race_has_exactly_one_winner() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let store = Arc::new(InMemoryStateStore::new());
    let wins = Arc::new(AtomicUsize::new(0));
    let handles: Vec<_> = (0..32)
        .map(|i| {
            let store = Arc::clone(&store);
            let wins = Arc::clone(&wins);
            std::thread::spawn(move || {
                let created = store
                    .set_json_if_absent(
                        &ctx("tenant-a"),
                        "flow/race",
                        &StateKey::new("node/a"),
                        &json!(i),
                        None,
                    )
                    .expect("race call");
                if created {
                    wins.fetch_add(1, Ordering::SeqCst);
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().expect("thread");
    }
    assert_eq!(wins.load(Ordering::SeqCst), 1);
}

#[test]
fn default_set_if_absent_is_an_error() {
    use greentic_state::StatePath;
    use greentic_types::GResult;
    use serde_json::Value;

    struct Legacy;
    impl StateStore for Legacy {
        fn get_json(
            &self,
            _: &TenantCtx,
            _: &str,
            _: &StateKey,
            _: Option<&StatePath>,
        ) -> GResult<Option<Value>> {
            Ok(None)
        }
        fn set_json(
            &self,
            _: &TenantCtx,
            _: &str,
            _: &StateKey,
            _: Option<&StatePath>,
            _: &Value,
            _: Option<u32>,
        ) -> GResult<()> {
            Ok(())
        }
        fn del(&self, _: &TenantCtx, _: &str, _: &StateKey) -> GResult<bool> {
            Ok(false)
        }
        fn del_prefix(&self, _: &TenantCtx, _: &str) -> GResult<u64> {
            Ok(0)
        }
    }

    let result =
        Legacy.set_json_if_absent(&ctx("tenant-a"), "p", &StateKey::new("k"), &json!(1), None);
    assert!(result.is_err(), "default must refuse, not silently claim");
}

#[cfg(feature = "redis")]
#[test]
fn redis_set_if_absent_creates_once() {
    use greentic_state::redis_store::RedisStateStore;
    use std::env;

    let Ok(url) = env::var("REDIS_URL") else {
        return;
    };
    let Ok(store) = RedisStateStore::from_url(&url) else {
        return;
    };
    let ctx = ctx("tenant-a");
    let prefix = format!("flow/ifabsent-{}", Uuid::new_v4());
    let key = StateKey::new("node/a");

    assert!(
        store
            .set_json_if_absent(&ctx, &prefix, &key, &json!({"v": 1}), Some(60))
            .expect("first")
    );
    assert!(
        !store
            .set_json_if_absent(&ctx, &prefix, &key, &json!({"v": 2}), None)
            .expect("second")
    );
    assert_eq!(
        store.get_json(&ctx, &prefix, &key, None).expect("get"),
        Some(json!({"v": 1}))
    );
}
