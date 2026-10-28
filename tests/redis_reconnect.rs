//! A Redis restart must not leave the store holding a dead connection.
//!
//! Uses a tiny in-process RESP server instead of Docker: the first client
//! connection is closed by the server after it has answered a few commands
//! (what a Redis restart or failover looks like to a client), and every later
//! connection is served normally.

#[cfg(feature = "redis")]
mod redis_reconnect {
    use greentic_state::redis_store::RedisStateStore;
    use greentic_state::{StateKey, StateStore, TenantCtx};
    use greentic_types::{EnvId, TenantId};
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;

    fn ctx() -> TenantCtx {
        TenantCtx::new(
            EnvId::try_from("dev").expect("valid env id"),
            TenantId::try_from("tenant").expect("valid tenant id"),
        )
    }

    /// Answers every pipelined command in `buf`: `GET` is a nil bulk string,
    /// anything else (`CLIENT SETINFO`, `SELECT`, ...) is `+OK`. Returns how
    /// many commands it saw.
    fn reply_to(buf: &[u8], out: &mut Vec<u8>) -> usize {
        let text = String::from_utf8_lossy(buf);
        let mut seen = 0;
        let mut lines = text.split("\r\n").peekable();
        while let Some(line) = lines.next() {
            if !line.starts_with('*') {
                continue;
            }
            let args: usize = line[1..].parse().unwrap_or(0);
            let mut name = String::new();
            for i in 0..args {
                let _len = lines.next();
                let arg = lines.next().unwrap_or_default();
                if i == 0 {
                    name = arg.to_ascii_uppercase();
                }
            }
            seen += 1;
            if name == "GET" {
                out.extend_from_slice(b"$-1\r\n");
            } else {
                out.extend_from_slice(b"+OK\r\n");
            }
        }
        seen
    }

    fn serve(mut stream: TcpStream, close_after_get: bool, gets: Arc<AtomicUsize>) {
        let mut buf = [0_u8; 4096];
        loop {
            let n = match stream.read(&mut buf) {
                Ok(0) | Err(_) => return,
                Ok(n) => n,
            };
            let mut out = Vec::new();
            reply_to(&buf[..n], &mut out);
            if stream.write_all(&out).is_err() {
                return;
            }
            if String::from_utf8_lossy(&buf[..n])
                .to_ascii_uppercase()
                .contains("GET")
            {
                gets.fetch_add(1, Ordering::SeqCst);
                if close_after_get {
                    // Drop the connection once it has answered a GET.
                    return;
                }
            }
        }
    }

    /// Returns the address and the number of connections the server accepted.
    fn fake_redis() -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        let conns = Arc::new(AtomicUsize::new(0));
        let accepted = Arc::clone(&conns);
        let gets = Arc::new(AtomicUsize::new(0));
        thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let first = accepted.fetch_add(1, Ordering::SeqCst) == 0;
                let gets = Arc::clone(&gets);
                thread::spawn(move || serve(stream, first, gets));
            }
        });
        (format!("redis://{addr}"), conns)
    }

    #[test]
    fn a_dropped_connection_is_replaced_on_the_next_command() {
        let (url, conns) = fake_redis();
        let store = RedisStateStore::from_url(&url).expect("client");
        let key = StateKey::from("k");

        // First command opens connection #1; the server then closes it.
        let first = store
            .get_json(&ctx(), "p", &key, None)
            .expect("first command works");
        assert_eq!(first, None);

        // The cached connection is now dead. The store must notice, open a
        // new one and answer, instead of failing for ever.
        let second = store
            .get_json(&ctx(), "p", &key, None)
            .expect("second command reconnects");
        assert_eq!(second, None);
        assert_eq!(conns.load(Ordering::SeqCst), 2, "one reconnect, not more");

        // And it keeps working on the new connection.
        let third = store
            .get_json(&ctx(), "p", &key, None)
            .expect("third command works");
        assert_eq!(third, None);
    }

    #[test]
    fn an_unreachable_server_is_still_an_error() {
        // Nothing listens here: the failure must surface, not be swallowed.
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        drop(listener);
        let store = RedisStateStore::from_url(format!("redis://{addr}")).expect("client");
        let key = StateKey::from("k");
        assert!(store.get_json(&ctx(), "p", &key, None).is_err());
    }
}
