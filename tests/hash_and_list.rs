mod support;

use bytes::Bytes;
use fire_redis::{RespCodec, Value};
use support::{recv, send_cmd, sorted_bulk_strings, start_test_server};
use tokio::net::TcpStream;
use tokio_util::codec::Framed;

#[tokio::test]
async fn test_server_hash_command_coverage() {
    let (addr, mut server) = start_test_server().await;
    let server_handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr).await.unwrap();
    let mut framed = Framed::new(stream, RespCodec);

    send_cmd(&mut framed, &["HSET", "h1", "f1", "v1", "f2", "v2"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(2));

    send_cmd(&mut framed, &["HEXISTS", "h1", "f1"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(1));

    send_cmd(&mut framed, &["HLEN", "h1"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(2));

    send_cmd(&mut framed, &["HKEYS", "h1"]).await;
    assert_eq!(
        sorted_bulk_strings(recv(&mut framed).await),
        vec!["f1".to_string(), "f2".to_string()]
    );

    send_cmd(&mut framed, &["HVALS", "h1"]).await;
    assert_eq!(
        sorted_bulk_strings(recv(&mut framed).await),
        vec!["v1".to_string(), "v2".to_string()]
    );

    send_cmd(&mut framed, &["HGETALL", "h1"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::Array(Some(vec![
            Value::BulkString(Some(Bytes::from("f1"))),
            Value::BulkString(Some(Bytes::from("v1"))),
            Value::BulkString(Some(Bytes::from("f2"))),
            Value::BulkString(Some(Bytes::from("v2"))),
        ]))
    );

    send_cmd(&mut framed, &["HDEL", "h1", "f1", "missing"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(1));

    send_cmd(&mut framed, &["HEXISTS", "h1", "f1"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(0));

    send_cmd(&mut framed, &["HDEL", "h1", "f2"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(1));

    send_cmd(&mut framed, &["EXISTS", "h1"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(0));

    send_cmd(&mut framed, &["TYPE", "h1"]).await;
    assert_eq!(recv(&mut framed).await, Value::SimpleString("none".to_string()));

    server_handle.abort();
}

#[tokio::test]
async fn test_server_list_command_coverage_and_cleanup() {
    let (addr, mut server) = start_test_server().await;
    let server_handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr).await.unwrap();
    let mut framed = Framed::new(stream, RespCodec);

    send_cmd(&mut framed, &["RPUSH", "l2", "a", "b", "c"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(3));

    send_cmd(&mut framed, &["LLEN", "l2"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(3));

    send_cmd(&mut framed, &["LRANGE", "l2", "0", "-1"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::Array(Some(vec![
            Value::BulkString(Some(Bytes::from("a"))),
            Value::BulkString(Some(Bytes::from("b"))),
            Value::BulkString(Some(Bytes::from("c"))),
        ]))
    );

    send_cmd(&mut framed, &["LINDEX", "l2", "-1"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::BulkString(Some(Bytes::from("c")))
    );

    send_cmd(&mut framed, &["LPOP", "l2"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::BulkString(Some(Bytes::from("a")))
    );

    send_cmd(&mut framed, &["RPOP", "l2"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::BulkString(Some(Bytes::from("c")))
    );

    send_cmd(&mut framed, &["LPOP", "l2"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::BulkString(Some(Bytes::from("b")))
    );

    send_cmd(&mut framed, &["EXISTS", "l2"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(0));

    send_cmd(&mut framed, &["TYPE", "l2"]).await;
    assert_eq!(recv(&mut framed).await, Value::SimpleString("none".to_string()));

    send_cmd(&mut framed, &["LRANGE", "l2", "0", "-1"]).await;
    assert_eq!(recv(&mut framed).await, Value::Array(Some(vec![])));

    server_handle.abort();
}

#[tokio::test]
async fn test_server_lrange_and_set_edge_cases() {
    let (addr, mut server) = start_test_server().await;
    let server_handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr).await.unwrap();
    let mut framed = Framed::new(stream, RespCodec);

    send_cmd(&mut framed, &["RPUSH", "l1", "a", "b", "c"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(3));

    send_cmd(&mut framed, &["LRANGE", "l1", "-10", "1"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::Array(Some(vec![
            Value::BulkString(Some(Bytes::from("a"))),
            Value::BulkString(Some(Bytes::from("b"))),
        ]))
    );

    send_cmd(&mut framed, &["LRANGE", "l1", "5", "10"]).await;
    assert_eq!(recv(&mut framed).await, Value::Array(Some(vec![])));

    send_cmd(&mut framed, &["SADD", "set1", "b", "a", "c", "a"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(3));

    send_cmd(&mut framed, &["SMEMBERS", "set1"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::Array(Some(vec![
            Value::BulkString(Some(Bytes::from("a"))),
            Value::BulkString(Some(Bytes::from("b"))),
            Value::BulkString(Some(Bytes::from("c"))),
        ]))
    );

    send_cmd(&mut framed, &["SREM", "set1", "a", "b", "c"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(3));

    send_cmd(&mut framed, &["EXISTS", "set1"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(0));

    server_handle.abort();
}

