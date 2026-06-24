mod support;

use fire_redis::{RespCodec, Value};
use support::{recv, send_cmd, start_test_server};
use tokio::net::TcpStream;
use tokio_util::codec::Framed;

#[tokio::test]
async fn test_server_expire() {
    let (addr, mut server) = start_test_server().await;
    let server_handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr).await.unwrap();
    let mut framed = Framed::new(stream, RespCodec);

    send_cmd(&mut framed, &["SET", "temp_key", "temp_value"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::SimpleString("OK".to_string())
    );

    send_cmd(&mut framed, &["EXPIRE", "temp_key", "1"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(1));

    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    send_cmd(&mut framed, &["GET", "temp_key"]).await;
    assert_eq!(recv(&mut framed).await, Value::Null);

    server_handle.abort();
}

#[tokio::test]
async fn test_server_ttl_and_pttl() {
    let (addr, mut server) = start_test_server().await;
    let server_handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr).await.unwrap();
    let mut framed = Framed::new(stream, RespCodec);

    send_cmd(&mut framed, &["TTL", "missing_key"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(-2));

    send_cmd(&mut framed, &["PTTL", "missing_key"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(-2));

    send_cmd(&mut framed, &["SET", "plain_key", "value"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::SimpleString("OK".to_string())
    );

    send_cmd(&mut framed, &["TTL", "plain_key"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(-1));

    send_cmd(&mut framed, &["PTTL", "plain_key"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(-1));

    send_cmd(&mut framed, &["EXPIRE", "plain_key", "2"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(1));

    send_cmd(&mut framed, &["PTTL", "plain_key"]).await;
    let pttl = match recv(&mut framed).await {
        Value::Integer(v) => v,
        other => panic!("unexpected PTTL response: {:?}", other),
    };
    assert!((1..=2000).contains(&pttl), "unexpected PTTL value: {pttl}");

    send_cmd(&mut framed, &["TTL", "plain_key"]).await;
    let ttl = match recv(&mut framed).await {
        Value::Integer(v) => v,
        other => panic!("unexpected TTL response: {:?}", other),
    };
    assert!((0..=2).contains(&ttl), "unexpected TTL value: {ttl}");

    tokio::time::sleep(tokio::time::Duration::from_millis(2200)).await;

    send_cmd(&mut framed, &["TTL", "plain_key"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(-2));

    server_handle.abort();
}

#[tokio::test]
async fn test_background_evict_removes_expired_key_from_store() {
    let (addr, mut server) = start_test_server().await;
    let store = server.store().clone();

    let server_handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr).await.unwrap();
    let mut framed = Framed::new(stream, RespCodec);

    send_cmd(&mut framed, &["SET", "evict_me", "value"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::SimpleString("OK".to_string())
    );

    send_cmd(&mut framed, &["EXPIRE", "evict_me", "1"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(1));

    assert!(store.get_for_restore("evict_me").is_some());

    tokio::time::sleep(tokio::time::Duration::from_millis(2200)).await;

    assert!(store.get_for_restore("evict_me").is_none());

    server_handle.abort();
}
