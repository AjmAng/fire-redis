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

fn uuid_like() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}
