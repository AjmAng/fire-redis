mod support;

use bytes::Bytes;
use fire_redis::{RespCodec, Value};
use support::{assert_error_contains, assert_wrongtype, recv, send_cmd, start_test_server};
use tokio::net::TcpStream;
use tokio_util::codec::Framed;

#[tokio::test]
async fn test_server_ping() {
    let (addr, mut server) = start_test_server().await;
    let server_handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr).await.unwrap();
    let mut framed = Framed::new(stream, RespCodec);

    send_cmd(&mut framed, &["PING"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::SimpleString("PONG".to_string())
    );

    send_cmd(&mut framed, &["ECHO", "Hello, Redis!"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::BulkString(Some(Bytes::from("Hello, Redis!")))
    );

    server_handle.abort();
}

#[tokio::test]
async fn test_server_set_get() {
    let (addr, mut server) = start_test_server().await;
    let server_handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr).await.unwrap();
    let mut framed = Framed::new(stream, RespCodec);

    send_cmd(&mut framed, &["SET", "test_key", "test_value"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::SimpleString("OK".to_string())
    );

    send_cmd(&mut framed, &["GET", "test_key"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::BulkString(Some(Bytes::from("test_value")))
    );

    send_cmd(&mut framed, &["GET", "non_existent_key"]).await;
    assert_eq!(recv(&mut framed).await, Value::Null);

    send_cmd(&mut framed, &["DEL", "test_key"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(1));

    send_cmd(&mut framed, &["GET", "test_key"]).await;
    assert_eq!(recv(&mut framed).await, Value::Null);

    server_handle.abort();
}

#[tokio::test]
async fn test_set_nx_xx_conditions() {
    let (addr, mut server) = start_test_server().await;
    let server_handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr).await.unwrap();
    let mut framed = Framed::new(stream, RespCodec);

    send_cmd(&mut framed, &["SET", "k1", "v1", "NX"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::SimpleString("OK".to_string())
    );

    send_cmd(&mut framed, &["SET", "k1", "v2", "NX"]).await;
    assert_eq!(recv(&mut framed).await, Value::Null);

    send_cmd(&mut framed, &["GET", "k1"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::BulkString(Some(Bytes::from("v1")))
    );

    send_cmd(&mut framed, &["SET", "missing", "v0", "XX"]).await;
    assert_eq!(recv(&mut framed).await, Value::Null);

    send_cmd(&mut framed, &["SET", "k1", "v3", "XX"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::SimpleString("OK".to_string())
    );

    send_cmd(&mut framed, &["GET", "k1"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::BulkString(Some(Bytes::from("v3")))
    );

    server_handle.abort();
}

#[tokio::test]
async fn test_server_decr_and_mget_mset() {
    let (addr, mut server) = start_test_server().await;
    let server_handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr).await.unwrap();
    let mut framed = Framed::new(stream, RespCodec);

    send_cmd(&mut framed, &["DECR", "counter"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(-1));

    send_cmd(&mut framed, &["SET", "counter", "5"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::SimpleString("OK".to_string())
    );

    send_cmd(&mut framed, &["DECR", "counter"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(4));

    send_cmd(&mut framed, &["SET", "not_int", "hello"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::SimpleString("OK".to_string())
    );

    send_cmd(&mut framed, &["DECR", "not_int"]).await;
    assert!(matches!(recv(&mut framed).await, Value::Error(_)));

    send_cmd(&mut framed, &["MSET", "k1", "v1", "k2", "v2"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::SimpleString("OK".to_string())
    );

    send_cmd(&mut framed, &["MGET", "k1", "missing", "k2"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::Array(Some(vec![
            Value::BulkString(Some(Bytes::from("v1"))),
            Value::Null,
            Value::BulkString(Some(Bytes::from("v2"))),
        ]))
    );

    server_handle.abort();
}

#[tokio::test]
async fn test_server_wrongtype_errors_for_typed_commands() {
    let (addr, mut server) = start_test_server().await;
    let server_handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr).await.unwrap();
    let mut framed = Framed::new(stream, RespCodec);

    send_cmd(&mut framed, &["SET", "plain", "value"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::SimpleString("OK".to_string())
    );

    send_cmd(&mut framed, &["LLEN", "plain"]).await;
    assert_wrongtype(recv(&mut framed).await);

    send_cmd(&mut framed, &["SADD", "plain", "member"]).await;
    assert_wrongtype(recv(&mut framed).await);

    send_cmd(&mut framed, &["HGET", "plain", "field"]).await;
    assert_wrongtype(recv(&mut framed).await);

    send_cmd(&mut framed, &["ZCARD", "plain"]).await;
    assert_wrongtype(recv(&mut framed).await);

    send_cmd(&mut framed, &["GET", "plain"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::BulkString(Some(Bytes::from("value")))
    );

    server_handle.abort();
}

#[tokio::test]
async fn test_server_wrong_arity_errors() {
    let (addr, mut server) = start_test_server().await;
    let server_handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr).await.unwrap();
    let mut framed = Framed::new(stream, RespCodec);

    send_cmd(&mut framed, &["GET"]).await;
    assert_error_contains(recv(&mut framed).await, "wrong number of arguments");

    send_cmd(&mut framed, &["SET", "k"]).await;
    assert_error_contains(recv(&mut framed).await, "wrong number of arguments");

    send_cmd(&mut framed, &["LRANGE", "l1", "0"]).await;
    assert_error_contains(recv(&mut framed).await, "wrong number of arguments");

    send_cmd(&mut framed, &["HSET", "h1", "f1"]).await;
    assert_error_contains(recv(&mut framed).await, "wrong number of arguments");

    send_cmd(&mut framed, &["SPOP", "s1", "1", "2"]).await;
    assert_error_contains(recv(&mut framed).await, "wrong number of arguments");

    send_cmd(&mut framed, &["ZCOUNT", "z1", "1"]).await;
    assert_error_contains(recv(&mut framed).await, "wrong number of arguments");

    server_handle.abort();
}

#[tokio::test]
async fn test_server_invalid_numeric_argument_errors() {
    let (addr, mut server) = start_test_server().await;
    let server_handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr).await.unwrap();
    let mut framed = Framed::new(stream, RespCodec);

    send_cmd(&mut framed, &["EXPIRE", "k", "oops"]).await;
    assert_error_contains(recv(&mut framed).await, "value is not integer");

    send_cmd(&mut framed, &["SET", "k", "v", "PX", "oops"]).await;
    assert_error_contains(recv(&mut framed).await, "value is not integer");

    send_cmd(&mut framed, &["LINDEX", "l1", "oops"]).await;
    assert_error_contains(recv(&mut framed).await, "value is not integer");

    send_cmd(&mut framed, &["SPOP", "s1", "oops"]).await;
    assert_error_contains(recv(&mut framed).await, "value is not integer");

    send_cmd(&mut framed, &["ZADD", "z1", "oops", "m1"]).await;
    assert_error_contains(recv(&mut framed).await, "value is not float");

    send_cmd(&mut framed, &["ZCOUNT", "z1", "0", "oops"]).await;
    assert_error_contains(recv(&mut framed).await, "value is not float");

    server_handle.abort();
}
