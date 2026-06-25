mod support;

use bytes::Bytes;
use fire_redis::{
    RespCodec, Value,
    persistence::{AofFsyncPolicy, PersistenceConfig},
};
use std::{collections::HashSet, path::Path, time::Duration};
use support::{recv, send_cmd, start_test_server_with_persistence};
use tokio::net::TcpStream;
use tokio_util::codec::Framed;

fn rdb_only_config(dir: &Path) -> PersistenceConfig {
    PersistenceConfig {
        rdb_enabled: true,
        rdb_file: dir.join("dump.rdb"),
        rdb_save_conditions: vec![],
        aof_enabled: false,
        aof_file: dir.join("appendonly.aof"),
        aof_fsync: AofFsyncPolicy::No,
        aof_rewrite_percentage: 100,
        aof_rewrite_min_size: 0,
    }
}

fn aof_only_config(dir: &Path) -> PersistenceConfig {
    PersistenceConfig {
        rdb_enabled: false,
        rdb_file: dir.join("dump.rdb"),
        rdb_save_conditions: vec![],
        aof_enabled: true,
        aof_file: dir.join("appendonly.aof"),
        aof_fsync: AofFsyncPolicy::Always,
        aof_rewrite_percentage: 100,
        aof_rewrite_min_size: 0,
    }
}

fn combined_config(dir: &Path) -> PersistenceConfig {
    PersistenceConfig {
        rdb_enabled: true,
        rdb_file: dir.join("dump.rdb"),
        rdb_save_conditions: vec![],
        aof_enabled: true,
        aof_file: dir.join("appendonly.aof"),
        aof_fsync: AofFsyncPolicy::Always,
        aof_rewrite_percentage: 100,
        aof_rewrite_min_size: 0,
    }
}

async fn connect_and_populate(addr: std::net::SocketAddr) -> Framed<TcpStream, RespCodec> {
    let stream = TcpStream::connect(addr).await.unwrap();
    let mut framed = Framed::new(stream, RespCodec);

    send_cmd(&mut framed, &["SET", "str_key", "str_value"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::SimpleString("OK".to_string())
    );

    send_cmd(&mut framed, &["RPUSH", "list_key", "a", "b", "c"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(3));

    send_cmd(&mut framed, &["SADD", "set_key", "x", "y", "z"]).await;
    let sadd_resp = recv(&mut framed).await;
    assert_eq!(sadd_resp, Value::Integer(3));

    send_cmd(&mut framed, &["HSET", "hash_key", "f1", "v1", "f2", "v2"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(2));

    send_cmd(&mut framed, &["ZADD", "zset_key", "1.5", "m1", "2.5", "m2"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(2));

    framed
}

async fn verify_populated_data(addr: std::net::SocketAddr) {
    let stream = TcpStream::connect(addr).await.unwrap();
    let mut framed = Framed::new(stream, RespCodec);

    send_cmd(&mut framed, &["GET", "str_key"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::BulkString(Some(Bytes::from("str_value")))
    );

    send_cmd(&mut framed, &["LRANGE", "list_key", "0", "-1"]).await;
    assert_eq!(
        support::sorted_bulk_strings(recv(&mut framed).await),
        vec!["a".to_string(), "b".to_string(), "c".to_string()]
    );

    send_cmd(&mut framed, &["SMEMBERS", "set_key"]).await;
    assert_eq!(
        support::sorted_bulk_strings(recv(&mut framed).await),
        vec!["x".to_string(), "y".to_string(), "z".to_string()]
    );

    send_cmd(&mut framed, &["HGETALL", "hash_key"]).await;
    let hash_pairs = recv(&mut framed).await;
    let mut pairs: Vec<(String, String)> = match hash_pairs {
        Value::Array(Some(values)) => values
            .chunks(2)
            .map(|chunk| {
                (
                    String::from_utf8(match &chunk[0] {
                        Value::BulkString(Some(b)) => b.to_vec(),
                        other => panic!("unexpected hash field: {:?}", other),
                    })
                    .unwrap(),
                    String::from_utf8(match &chunk[1] {
                        Value::BulkString(Some(b)) => b.to_vec(),
                        other => panic!("unexpected hash value: {:?}", other),
                    })
                    .unwrap(),
                )
            })
            .collect(),
        other => panic!("expected array, got: {:?}", other),
    };
    pairs.sort();
    assert_eq!(
        pairs,
        vec![
            ("f1".to_string(), "v1".to_string()),
            ("f2".to_string(), "v2".to_string()),
        ]
    );

    send_cmd(&mut framed, &["ZRANGE", "zset_key", "0", "-1"]).await;
    assert_eq!(
        support::sorted_bulk_strings(recv(&mut framed).await),
        vec!["m1".to_string(), "m2".to_string()]
    );

    send_cmd(&mut framed, &["ZSCORE", "zset_key", "m1"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::BulkString(Some(Bytes::from("1.5")))
    );
}

#[tokio::test]
async fn test_rdb_full_recovery_includes_all_types_and_ttl() {
    let dir = std::env::temp_dir().join(format!("fire-redis-rdb-test-{}", uuid_like()));
    tokio::fs::create_dir_all(&dir).await.unwrap();
    let config = rdb_only_config(&dir);

    let (addr, mut server) = start_test_server_with_persistence(config.clone()).await;
    let pm = server.persistence_manager().unwrap().clone();
    let handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut framed = connect_and_populate(addr).await;

    // Set a TTL on one key.
    send_cmd(&mut framed, &["EXPIRE", "str_key", "10"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(1));

    // Persist and shut down gracefully.
    pm.save().await.unwrap();
    pm.shutdown().await.unwrap();
    handle.abort();

    // Restart with the same RDB file.
    let (addr2, mut server2) = start_test_server_with_persistence(config).await;
    let handle2 = tokio::spawn(async move {
        server2.run().await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    verify_populated_data(addr2).await;

    let stream = TcpStream::connect(addr2).await.unwrap();
    let mut framed = Framed::new(stream, RespCodec);
    send_cmd(&mut framed, &["TTL", "str_key"]).await;
    let ttl = match recv(&mut framed).await {
        Value::Integer(v) => v,
        other => panic!("unexpected TTL: {:?}", other),
    };
    assert!(
        (5..=10).contains(&ttl),
        "expected TTL around 10s, got {}",
        ttl
    );

    handle2.abort();
    let _ = tokio::fs::remove_dir_all(&dir).await;
}

#[tokio::test]
async fn test_aof_replay_and_rewrite_preserves_ttl() {
    let dir = std::env::temp_dir().join(format!("fire-redis-aof-test-{}", uuid_like()));
    tokio::fs::create_dir_all(&dir).await.unwrap();
    let config = aof_only_config(&dir);

    let (addr, mut server) = start_test_server_with_persistence(config.clone()).await;
    let pm = server.persistence_manager().unwrap().clone();
    let handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut framed = connect_and_populate(addr).await;

    send_cmd(&mut framed, &["SET", "ttl_key", "ttl_value"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::SimpleString("OK".to_string())
    );
    send_cmd(&mut framed, &["EXPIRE", "ttl_key", "10"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(1));

    // Rewrite AOF so that TTL must be emitted by value_to_commands.
    pm.rewrite_aof().await.unwrap();
    pm.shutdown().await.unwrap();
    handle.abort();

    let (addr2, mut server2) = start_test_server_with_persistence(config).await;
    let handle2 = tokio::spawn(async move {
        server2.run().await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr2).await.unwrap();
    let mut framed = Framed::new(stream, RespCodec);
    send_cmd(&mut framed, &["GET", "ttl_key"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::BulkString(Some(Bytes::from("ttl_value")))
    );
    send_cmd(&mut framed, &["TTL", "ttl_key"]).await;
    let ttl = match recv(&mut framed).await {
        Value::Integer(v) => v,
        other => panic!("unexpected TTL: {:?}", other),
    };
    assert!(
        (5..=10).contains(&ttl),
        "expected TTL around 10s, got {}",
        ttl
    );

    verify_populated_data(addr2).await;

    handle2.abort();
    let _ = tokio::fs::remove_dir_all(&dir).await;
}

#[tokio::test]
async fn test_combined_rdb_and_aof_load() {
    let dir = std::env::temp_dir().join(format!("fire-redis-combined-test-{}", uuid_like()));
    tokio::fs::create_dir_all(&dir).await.unwrap();
    let config = combined_config(&dir);

    let (addr, mut server) = start_test_server_with_persistence(config.clone()).await;
    let pm = server.persistence_manager().unwrap().clone();
    let handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    let mut framed = connect_and_populate(addr).await;

    // Save RDB baseline.
    pm.save().await.unwrap();

    // Additional writes after the RDB snapshot must come from AOF.
    send_cmd(&mut framed, &["SET", "aof_key", "aof_value"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::SimpleString("OK".to_string())
    );
    send_cmd(&mut framed, &["SADD", "set_key", "w"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(1));

    pm.shutdown().await.unwrap();
    handle.abort();

    let (addr2, mut server2) = start_test_server_with_persistence(config).await;
    let handle2 = tokio::spawn(async move {
        server2.run().await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr2).await.unwrap();
    let mut framed = Framed::new(stream, RespCodec);
    send_cmd(&mut framed, &["GET", "str_key"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::BulkString(Some(Bytes::from("str_value")))
    );
    send_cmd(&mut framed, &["GET", "aof_key"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::BulkString(Some(Bytes::from("aof_value")))
    );
    send_cmd(&mut framed, &["SMEMBERS", "set_key"]).await;
    let members: HashSet<String> = support::sorted_bulk_strings(recv(&mut framed).await)
        .into_iter()
        .collect();
    assert_eq!(
        members,
        HashSet::from([
            "w".to_string(),
            "x".to_string(),
            "y".to_string(),
            "z".to_string()
        ])
    );

    handle2.abort();
    let _ = tokio::fs::remove_dir_all(&dir).await;
}

#[tokio::test]
async fn test_expired_keys_are_not_restored() {
    let dir = std::env::temp_dir().join(format!("fire-redis-expiry-test-{}", uuid_like()));
    tokio::fs::create_dir_all(&dir).await.unwrap();
    let config = rdb_only_config(&dir);

    let (addr, mut server) = start_test_server_with_persistence(config.clone()).await;
    let pm = server.persistence_manager().unwrap().clone();
    let handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr).await.unwrap();
    let mut framed = Framed::new(stream, RespCodec);
    send_cmd(&mut framed, &["SET", "short_lived", "value"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::SimpleString("OK".to_string())
    );
    send_cmd(&mut framed, &["EXPIRE", "short_lived", "1"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(1));

    tokio::time::sleep(Duration::from_millis(1200)).await;

    pm.save().await.unwrap();
    pm.shutdown().await.unwrap();
    handle.abort();

    let (addr2, mut server2) = start_test_server_with_persistence(config).await;
    let handle2 = tokio::spawn(async move {
        server2.run().await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr2).await.unwrap();
    let mut framed = Framed::new(stream, RespCodec);
    send_cmd(&mut framed, &["EXISTS", "short_lived"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(0));

    handle2.abort();
    let _ = tokio::fs::remove_dir_all(&dir).await;
}

#[tokio::test]
async fn test_no_persistence_data_lost_on_restart() {
    let dir = std::env::temp_dir().join(format!("fire-redis-no-persist-{}", uuid_like()));
    tokio::fs::create_dir_all(&dir).await.unwrap();
    let config = fire_redis::persistence::PersistenceConfig::disabled();

    let (addr, mut server) = start_test_server_with_persistence(config.clone()).await;
    let handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr).await.unwrap();
    let mut framed = Framed::new(stream, RespCodec);

    send_cmd(&mut framed, &["SET", "mykey", "hello"]).await;
    assert_eq!(recv(&mut framed).await, Value::SimpleString("OK".to_string()));

    // With persistence disabled, persistence_manager() returns None.
    // Just drop the server — no save possible.
    handle.abort();

    // Start a fresh server using the same (disabled) config.
    let (addr2, mut server2) = start_test_server_with_persistence(config).await;
    let handle2 = tokio::spawn(async move {
        server2.run().await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Data must be lost since persistence is disabled.
    let stream2 = TcpStream::connect(addr2).await.unwrap();
    let mut framed2 = Framed::new(stream2, RespCodec);
    send_cmd(&mut framed2, &["GET", "mykey"]).await;
    assert_eq!(recv(&mut framed2).await, Value::Null);

    handle2.abort();
    let _ = tokio::fs::remove_dir_all(&dir).await;
}

#[tokio::test]
async fn test_del_before_second_rdb_save() {
    let dir = std::env::temp_dir().join(format!("fire-redis-del-rdb-{}", uuid_like()));
    tokio::fs::create_dir_all(&dir).await.unwrap();
    let config = rdb_only_config(&dir);

    let (addr, mut server) = start_test_server_with_persistence(config.clone()).await;
    let pm = server.persistence_manager().unwrap().clone();
    let handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr).await.unwrap();
    let mut framed = Framed::new(stream, RespCodec);

    // Set two keys and save.
    send_cmd(&mut framed, &["SET", "k1", "v1"]).await;
    assert_eq!(recv(&mut framed).await, Value::SimpleString("OK".to_string()));
    send_cmd(&mut framed, &["SET", "k2", "v2"]).await;
    assert_eq!(recv(&mut framed).await, Value::SimpleString("OK".to_string()));
    pm.save().await.unwrap();

    // Delete one key and save again.
    send_cmd(&mut framed, &["DEL", "k1"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(1));
    pm.save().await.unwrap();

    pm.shutdown().await.unwrap();
    handle.abort();

    // Restart.
    let (addr2, mut server2) = start_test_server_with_persistence(config).await;
    let handle2 = tokio::spawn(async move {
        server2.run().await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    let stream2 = TcpStream::connect(addr2).await.unwrap();
    let mut framed2 = Framed::new(stream2, RespCodec);

    // k1 must be gone; k2 must still exist.
    send_cmd(&mut framed2, &["GET", "k1"]).await;
    assert_eq!(recv(&mut framed2).await, Value::Null);
    send_cmd(&mut framed2, &["GET", "k2"]).await;
    assert_eq!(
        recv(&mut framed2).await,
        Value::BulkString(Some(Bytes::from("v2")))
    );

    handle2.abort();
    let _ = tokio::fs::remove_dir_all(&dir).await;
}

#[tokio::test]
async fn test_flushall_before_persist() {
    let dir = std::env::temp_dir().join(format!("fire-redis-flushall-rdb-{}", uuid_like()));
    tokio::fs::create_dir_all(&dir).await.unwrap();
    let config = rdb_only_config(&dir);

    let (addr, mut server) = start_test_server_with_persistence(config.clone()).await;
    let pm = server.persistence_manager().unwrap().clone();
    let handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr).await.unwrap();
    let mut framed = Framed::new(stream, RespCodec);

    // Populate, flush all, then save.
    send_cmd(&mut framed, &["SET", "a", "1"]).await;
    assert_eq!(recv(&mut framed).await, Value::SimpleString("OK".to_string()));
    send_cmd(&mut framed, &["SET", "b", "2"]).await;
    assert_eq!(recv(&mut framed).await, Value::SimpleString("OK".to_string()));

    send_cmd(&mut framed, &["FLUSHALL"]).await;
    assert_eq!(recv(&mut framed).await, Value::SimpleString("OK".to_string()));

    pm.save().await.unwrap();
    pm.shutdown().await.unwrap();
    handle.abort();

    // Restart — should be empty.
    let (addr2, mut server2) = start_test_server_with_persistence(config).await;
    let handle2 = tokio::spawn(async move {
        server2.run().await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    let stream2 = TcpStream::connect(addr2).await.unwrap();
    let mut framed2 = Framed::new(stream2, RespCodec);

    send_cmd(&mut framed2, &["EXISTS", "a"]).await;
    assert_eq!(recv(&mut framed2).await, Value::Integer(0));
    send_cmd(&mut framed2, &["EXISTS", "b"]).await;
    assert_eq!(recv(&mut framed2).await, Value::Integer(0));

    handle2.abort();
    let _ = tokio::fs::remove_dir_all(&dir).await;
}

#[tokio::test]
async fn test_aof_ttl_replayed_correctly() {
    let dir = std::env::temp_dir().join(format!("fire-redis-aof-ttl-{}", uuid_like()));
    tokio::fs::create_dir_all(&dir).await.unwrap();
    let config = aof_only_config(&dir);

    let (addr, mut server) = start_test_server_with_persistence(config.clone()).await;
    let pm = server.persistence_manager().unwrap().clone();
    let handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr).await.unwrap();
    let mut framed = Framed::new(stream, RespCodec);

    // Set a key and wait so the TTL has partially elapsed before shutdown.
    send_cmd(&mut framed, &["SET", "ttl_key", "value"]).await;
    assert_eq!(recv(&mut framed).await, Value::SimpleString("OK".to_string()));
    send_cmd(&mut framed, &["EXPIRE", "ttl_key", "10"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(1));

    // Wait 2 seconds so the TTL partially burns down.
    tokio::time::sleep(Duration::from_millis(2000)).await;

    // Verify TTL has decreased.
    send_cmd(&mut framed, &["TTL", "ttl_key"]).await;
    let before_ttl = match recv(&mut framed).await {
        Value::Integer(v) => v,
        other => panic!("expected TTL integer, got: {:?}", other),
    };
    assert!(
        (4..=8).contains(&before_ttl),
        "expected TTL ~8s after 2s wait, got {}",
        before_ttl
    );

    pm.shutdown().await.unwrap();
    handle.abort();

    // Restart — the key should be replayed via AOF with a fresh TTL of 10s.
    let (addr2, mut server2) = start_test_server_with_persistence(config).await;
    let handle2 = tokio::spawn(async move {
        server2.run().await.unwrap();
    });
    tokio::time::sleep(Duration::from_millis(100)).await;

    let stream2 = TcpStream::connect(addr2).await.unwrap();
    let mut framed2 = Framed::new(stream2, RespCodec);

    // Key must exist.
    send_cmd(&mut framed2, &["EXISTS", "ttl_key"]).await;
    assert_eq!(recv(&mut framed2).await, Value::Integer(1));

    // TTL must be close to original 10s (fresh replay).
    send_cmd(&mut framed2, &["TTL", "ttl_key"]).await;
    let after_ttl = match recv(&mut framed2).await {
        Value::Integer(v) => v,
        other => panic!("expected TTL integer, got: {:?}", other),
    };
    assert!(
        (7..=10).contains(&after_ttl),
        "expected fresh TTL ~10s after replay, got {}",
        after_ttl
    );

    handle2.abort();
    let _ = tokio::fs::remove_dir_all(&dir).await;
}

fn uuid_like() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}
