#![allow(dead_code)]

use bytes::Bytes;
use fire_redis::{RespCodec, Server, Value};
use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_util::codec::Framed;

pub async fn start_test_server() -> (std::net::SocketAddr, Server) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = Server::from_listener(listener).await.unwrap();
    (addr, server)
}

pub async fn start_test_server_with_persistence(
    persistence: fire_redis::persistence::PersistenceConfig,
) -> (std::net::SocketAddr, Server) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let server = Server::from_listener_with_config(listener, persistence)
        .await
        .unwrap();
    (addr, server)
}

pub async fn send_cmd(framed: &mut Framed<TcpStream, RespCodec>, args: &[&str]) {
    let command = Value::Array(Some(
        args.iter()
            .map(|arg| Value::BulkString(Some(Bytes::from((*arg).to_string()))))
            .collect(),
    ));
    framed.send(command).await.unwrap();
}

pub async fn recv(framed: &mut Framed<TcpStream, RespCodec>) -> Value {
    framed.next().await.unwrap().unwrap()
}

pub fn assert_wrongtype(response: Value) {
    match response {
        Value::Error(msg) => assert!(
            msg.starts_with("WRONGTYPE"),
            "expected WRONGTYPE error, got: {msg}"
        ),
        other => panic!("expected WRONGTYPE error, got: {:?}", other),
    }
}

pub fn assert_error_contains(response: Value, needle: &str) {
    match response {
        Value::Error(msg) => assert!(
            msg.contains(needle),
            "expected error containing '{needle}', got: {msg}"
        ),
        other => panic!("expected error response, got: {:?}", other),
    }
}

pub fn sorted_bulk_strings(response: Value) -> Vec<String> {
    let mut items = match response {
        Value::Array(Some(values)) => values
            .into_iter()
            .map(|value| match value {
                Value::BulkString(Some(bytes)) => String::from_utf8(bytes.to_vec()).unwrap(),
                other => panic!("expected bulk string array item, got: {:?}", other),
            })
            .collect::<Vec<_>>(),
        other => panic!("expected array response, got: {:?}", other),
    };
    items.sort();
    items
}
