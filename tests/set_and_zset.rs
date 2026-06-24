mod support;

use bytes::Bytes;
use fire_redis::{RespCodec, Value};
use support::{recv, send_cmd, start_test_server};
use tokio::net::TcpStream;
use tokio_util::codec::Framed;

#[tokio::test]
async fn test_server_set_spop_and_scard_coverage() {
    let (addr, mut server) = start_test_server().await;
    let server_handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr).await.unwrap();
    let mut framed = Framed::new(stream, RespCodec);

    send_cmd(&mut framed, &["SADD", "s2", "a", "b", "c"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(3));

    send_cmd(&mut framed, &["SCARD", "s2"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(3));

    send_cmd(&mut framed, &["SPOP", "s2", "2"]).await;
    let popped = match recv(&mut framed).await {
        Value::Array(Some(values)) => values,
        other => panic!("unexpected SPOP array response: {:?}", other),
    };
    assert_eq!(popped.len(), 2);

    send_cmd(&mut framed, &["SCARD", "s2"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(1));

    send_cmd(&mut framed, &["SPOP", "s2"]).await;
    assert!(matches!(
        recv(&mut framed).await,
        Value::BulkString(Some(_))
    ));

    send_cmd(&mut framed, &["EXISTS", "s2"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(0));

    send_cmd(&mut framed, &["SPOP", "s2"]).await;
    assert_eq!(recv(&mut framed).await, Value::Null);

    server_handle.abort();
}

#[tokio::test]
async fn test_server_zadd_update_semantics() {
    let (addr, mut server) = start_test_server().await;
    let server_handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr).await.unwrap();
    let mut framed = Framed::new(stream, RespCodec);

    send_cmd(&mut framed, &["ZADD", "z1", "1", "m1"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(1));

    send_cmd(&mut framed, &["ZADD", "z1", "2", "m1"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(0));

    send_cmd(&mut framed, &["ZCARD", "z1"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(1));

    send_cmd(&mut framed, &["ZRANGE", "z1", "0", "-1"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::Array(Some(vec![Value::BulkString(Some(Bytes::from("m1")))]))
    );

    send_cmd(&mut framed, &["ZSCORE", "z1", "m1"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::BulkString(Some(Bytes::from("2")))
    );

    server_handle.abort();
}

#[tokio::test]
async fn test_server_zset_range_count_and_removal_coverage() {
    let (addr, mut server) = start_test_server().await;
    let server_handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr).await.unwrap();
    let mut framed = Framed::new(stream, RespCodec);

    send_cmd(&mut framed, &["ZADD", "z2", "1", "a", "2", "b", "3", "c"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(3));

    send_cmd(&mut framed, &["ZRANGE", "z2", "0", "-1"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::Array(Some(vec![
            Value::BulkString(Some(Bytes::from("a"))),
            Value::BulkString(Some(Bytes::from("b"))),
            Value::BulkString(Some(Bytes::from("c"))),
        ]))
    );

    send_cmd(&mut framed, &["ZREVRANGE", "z2", "0", "1"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::Array(Some(vec![
            Value::BulkString(Some(Bytes::from("c"))),
            Value::BulkString(Some(Bytes::from("b"))),
        ]))
    );

    send_cmd(&mut framed, &["ZCOUNT", "z2", "2", "3"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(2));

    send_cmd(&mut framed, &["ZREM", "z2", "b", "missing"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(1));

    send_cmd(&mut framed, &["ZCARD", "z2"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(2));

    send_cmd(&mut framed, &["ZREM", "z2", "a", "c"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(2));

    send_cmd(&mut framed, &["EXISTS", "z2"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(0));

    send_cmd(&mut framed, &["ZCOUNT", "z2", "0", "10"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(0));

    server_handle.abort();
}
