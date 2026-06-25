mod support;

use bytes::Bytes;
use fire_redis::Value;
use support::{recv, send_cmd, start_test_server};
use tokio::net::TcpStream;
use tokio_util::codec::Framed;

#[tokio::test]
async fn test_incr() {
    let (addr, mut server) = start_test_server().await;
    let server_handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr).await.unwrap();
    let mut framed = Framed::new(stream, fire_redis::RespCodec);

    // INCR on non-existent key starts at 0 and increments to 1
    send_cmd(&mut framed, &["INCR", "counter"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(1));

    // INCR again -> 2
    send_cmd(&mut framed, &["INCR", "counter"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(2));

    // INCR on a string key returns error
    send_cmd(&mut framed, &["SET", "astr", "hello"]).await;
    assert_eq!(recv(&mut framed).await, Value::SimpleString("OK".to_string()));

    send_cmd(&mut framed, &["INCR", "astr"]).await;
    // INCR on a non-parsable string returns an error
    let resp = recv(&mut framed).await;
    assert!(
        matches!(&resp, Value::Error(e) if e.contains("integer")),
        "expected integer error, got {resp:?}"
    );

    server_handle.abort();
}

#[tokio::test]
async fn test_lpush() {
    let (addr, mut server) = start_test_server().await;
    let server_handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr).await.unwrap();
    let mut framed = Framed::new(stream, fire_redis::RespCodec);

    // LPUSH returns the new length of the list
    send_cmd(&mut framed, &["LPUSH", "mylist", "world"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(1));

    send_cmd(&mut framed, &["LPUSH", "mylist", "hello"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(2));

    // Verify with LRANGE: hello is at front (index 0)
    send_cmd(&mut framed, &["LRANGE", "mylist", "0", "-1"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::Array(Some(vec![
            Value::BulkString(Some(Bytes::from("hello"))),
            Value::BulkString(Some(Bytes::from("world"))),
        ]))
    );

    // LPUSH with multiple values
    send_cmd(&mut framed, &["LPUSH", "mylist", "a", "b"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(4));

    // LPUSH on a non-list key returns WRONGTYPE
    send_cmd(&mut framed, &["SET", "astr", "val"]).await;
    assert_eq!(recv(&mut framed).await, Value::SimpleString("OK".to_string()));

    send_cmd(&mut framed, &["LPUSH", "astr", "x"]).await;
    assert!(
        matches!(recv(&mut framed).await, Value::Error(ref e) if e.contains("WRONGTYPE"))
    );

    server_handle.abort();
}

#[tokio::test]
async fn test_quit() {
    let (addr, mut server) = start_test_server().await;
    let server_handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr).await.unwrap();
    let mut framed = Framed::new(stream, fire_redis::RespCodec);

    // QUIT returns OK
    send_cmd(&mut framed, &["QUIT"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::SimpleString("OK".to_string())
    );

    server_handle.abort();
}

#[tokio::test]
async fn test_info() {
    let (addr, mut server) = start_test_server().await;
    let server_handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr).await.unwrap();
    let mut framed = Framed::new(stream, fire_redis::RespCodec);

    // INFO returns a bulk string with server info
    send_cmd(&mut framed, &["INFO"]).await;
    let resp = recv(&mut framed).await;
    match resp {
        Value::BulkString(Some(info)) => {
            let info_str = String::from_utf8_lossy(&info);
            assert!(info_str.contains("redis_version"), "missing redis_version");
            assert!(info_str.contains("active_connections"), "missing active_connections");
        }
        other => panic!("expected BulkString, got {other:?}"),
    }

    server_handle.abort();
}
