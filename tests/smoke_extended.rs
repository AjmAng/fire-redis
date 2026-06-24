mod support;

use bytes::Bytes;
use fire_redis::{RespCodec, Value};
use support::{recv, send_cmd, start_test_server};
use tokio::net::TcpStream;
use tokio_util::codec::Framed;

#[tokio::test]
async fn test_extended_commands_parse_and_execute() {
    let (addr, mut server) = start_test_server().await;
    let server_handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr).await.unwrap();
    let mut framed = Framed::new(stream, RespCodec);

    send_cmd(&mut framed, &["APPEND", "s1", "foo"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(3));

    send_cmd(&mut framed, &["STRLEN", "s1"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(3));

    send_cmd(&mut framed, &["RPUSH", "l1", "a", "b"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(2));

    send_cmd(&mut framed, &["LINDEX", "l1", "-1"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::BulkString(Some(Bytes::from("b")))
    );

    send_cmd(&mut framed, &["SADD", "set1", "x", "y"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(2));

    send_cmd(&mut framed, &["SISMEMBER", "set1", "x"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(1));

    send_cmd(&mut framed, &["SMEMBERS", "set1"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::Array(Some(vec![
            Value::BulkString(Some(Bytes::from("x"))),
            Value::BulkString(Some(Bytes::from("y"))),
        ]))
    );

    send_cmd(&mut framed, &["HSET", "h1", "f1", "v1", "f2", "v2"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(2));

    send_cmd(&mut framed, &["HGET", "h1", "f1"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::BulkString(Some(Bytes::from("v1")))
    );

    send_cmd(&mut framed, &["ZADD", "z1", "1", "m1", "2", "m2"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(2));

    send_cmd(&mut framed, &["ZCARD", "z1"]).await;
    assert_eq!(recv(&mut framed).await, Value::Integer(2));

    send_cmd(&mut framed, &["TYPE", "z1"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::SimpleString("zset".to_string())
    );

    send_cmd(&mut framed, &["FLUSHALL"]).await;
    assert_eq!(
        recv(&mut framed).await,
        Value::SimpleString("OK".to_string())
    );

    send_cmd(&mut framed, &["KEYS", "*"]).await;
    assert_eq!(recv(&mut framed).await, Value::Array(Some(vec![])));

    server_handle.abort();
}
