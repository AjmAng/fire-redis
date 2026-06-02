use bytes::Bytes;
use fire_redis::{RespCodec, Server, Value};
use futures::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_util::codec::Framed;

async fn start_test_server() -> (std::net::SocketAddr, Server) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let server = Server::from_listener(listener).await.unwrap();

    (addr, server)
}

fn assert_wrongtype(response: Value) {
    match response {
        Value::Error(msg) => assert!(
            msg.starts_with("WRONGTYPE"),
            "expected WRONGTYPE error, got: {msg}"
        ),
        other => panic!("expected WRONGTYPE error, got: {:?}", other),
    }
}

fn sorted_bulk_strings(response: Value) -> Vec<String> {
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

#[macro_export]
macro_rules! cmd {
    ($framed:expr, $($arg:expr),* $(,)?) => {{
        let args = vec![
            $(Value::BulkString(Some(Bytes::from($arg.to_string())))),*
        ];
        $framed.send(Value::Array(Some(args))).await.unwrap()
    }};
}

#[tokio::test]
async fn test_server_ping() {
    let (addr, mut server) = start_test_server().await;

    let server_handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr).await.unwrap();
    let mut framed = Framed::new(stream, RespCodec);

    cmd!(framed, "PING");

    let response = framed.next().await.unwrap().unwrap();
    assert_eq!(response, Value::SimpleString("PONG".to_string()));

    // test echo
    cmd!(framed, "ECHO", "Hello, Redis!");
    let response = framed.next().await.unwrap().unwrap();
    assert_eq!(
        response,
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

    // SET key value
    cmd!(framed, "SET", "test_key", "test_value");

    let response = framed.next().await.unwrap().unwrap();
    assert_eq!(response, Value::SimpleString("OK".to_string()));

    // GET key
    cmd!(framed, "GET", "test_key");

    let response = framed.next().await.unwrap().unwrap();
    assert_eq!(response, Value::BulkString(Some(Bytes::from("test_value"))));

    // GET non-existent key
    cmd!(framed, "GET", "non_existent_key");

    let response = framed.next().await.unwrap().unwrap();
    assert_eq!(response, Value::Null);

    // DeL key
    cmd!(framed, "DEL", "test_key");

    let response = framed.next().await.unwrap().unwrap();
    assert_eq!(response, Value::Integer(1));

    // GET deleted key
    cmd!(framed, "GET", "test_key");

    let response = framed.next().await.unwrap().unwrap();
    assert_eq!(response, Value::Null);

    server_handle.abort()
}

#[tokio::test]
async fn test_server_hash_command_coverage() {
    let (addr, mut server) = start_test_server().await;
    let server_handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr).await.unwrap();
    let mut framed = Framed::new(stream, RespCodec);

    cmd!(framed, "HSET", "h1", "f1", "v1", "f2", "v2");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(2));

    cmd!(framed, "HEXISTS", "h1", "f1");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(1));

    cmd!(framed, "HLEN", "h1");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(2));

    cmd!(framed, "HKEYS", "h1");
    assert_eq!(
        sorted_bulk_strings(framed.next().await.unwrap().unwrap()),
        vec!["f1".to_string(), "f2".to_string()]
    );

    cmd!(framed, "HVALS", "h1");
    assert_eq!(
        sorted_bulk_strings(framed.next().await.unwrap().unwrap()),
        vec!["v1".to_string(), "v2".to_string()]
    );

    cmd!(framed, "HGETALL", "h1");
    assert_eq!(
        framed.next().await.unwrap().unwrap(),
        Value::Array(Some(vec![
            Value::BulkString(Some(Bytes::from("f1"))),
            Value::BulkString(Some(Bytes::from("v1"))),
            Value::BulkString(Some(Bytes::from("f2"))),
            Value::BulkString(Some(Bytes::from("v2"))),
        ]))
    );

    cmd!(framed, "HDEL", "h1", "f1", "missing");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(1));

    cmd!(framed, "HEXISTS", "h1", "f1");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(0));

    cmd!(framed, "HDEL", "h1", "f2");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(1));

    cmd!(framed, "EXISTS", "h1");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(0));

    cmd!(framed, "TYPE", "h1");
    assert_eq!(
        framed.next().await.unwrap().unwrap(),
        Value::SimpleString("none".to_string())
    );

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

    cmd!(framed, "RPUSH", "l2", "a", "b", "c");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(3));

    cmd!(framed, "LLEN", "l2");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(3));

    cmd!(framed, "LRANGE", "l2", "0", "-1");
    assert_eq!(
        framed.next().await.unwrap().unwrap(),
        Value::Array(Some(vec![
            Value::BulkString(Some(Bytes::from("a"))),
            Value::BulkString(Some(Bytes::from("b"))),
            Value::BulkString(Some(Bytes::from("c"))),
        ]))
    );

    cmd!(framed, "LINDEX", "l2", "-1");
    assert_eq!(
        framed.next().await.unwrap().unwrap(),
        Value::BulkString(Some(Bytes::from("c")))
    );

    cmd!(framed, "LPOP", "l2");
    assert_eq!(
        framed.next().await.unwrap().unwrap(),
        Value::BulkString(Some(Bytes::from("a")))
    );

    cmd!(framed, "RPOP", "l2");
    assert_eq!(
        framed.next().await.unwrap().unwrap(),
        Value::BulkString(Some(Bytes::from("c")))
    );

    cmd!(framed, "LPOP", "l2");
    assert_eq!(
        framed.next().await.unwrap().unwrap(),
        Value::BulkString(Some(Bytes::from("b")))
    );

    cmd!(framed, "EXISTS", "l2");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(0));

    cmd!(framed, "TYPE", "l2");
    assert_eq!(
        framed.next().await.unwrap().unwrap(),
        Value::SimpleString("none".to_string())
    );

    cmd!(framed, "LRANGE", "l2", "0", "-1");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Array(Some(vec![])));

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

    cmd!(framed, "ZADD", "z1", "1", "m1");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(1));

    cmd!(framed, "ZADD", "z1", "2", "m1");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(0));

    cmd!(framed, "ZCARD", "z1");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(1));

    cmd!(framed, "ZRANGE", "z1", "0", "-1");
    assert_eq!(
        framed.next().await.unwrap().unwrap(),
        Value::Array(Some(vec![Value::BulkString(Some(Bytes::from("m1")))]))
    );

    cmd!(framed, "ZSCORE", "z1", "m1");
    assert_eq!(
        framed.next().await.unwrap().unwrap(),
        Value::BulkString(Some(Bytes::from("2")))
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

    cmd!(framed, "SET", "plain", "value");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::SimpleString("OK".to_string()));

    cmd!(framed, "LLEN", "plain");
    assert_wrongtype(framed.next().await.unwrap().unwrap());

    cmd!(framed, "SADD", "plain", "member");
    assert_wrongtype(framed.next().await.unwrap().unwrap());

    cmd!(framed, "HGET", "plain", "field");
    assert_wrongtype(framed.next().await.unwrap().unwrap());

    cmd!(framed, "ZCARD", "plain");
    assert_wrongtype(framed.next().await.unwrap().unwrap());

    cmd!(framed, "GET", "plain");
    assert_eq!(
        framed.next().await.unwrap().unwrap(),
        Value::BulkString(Some(Bytes::from("value")))
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

    cmd!(framed, "DECR", "counter");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(-1));

    cmd!(framed, "SET", "counter", "5");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::SimpleString("OK".to_string()));

    cmd!(framed, "DECR", "counter");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(4));

    cmd!(framed, "SET", "not_int", "hello");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::SimpleString("OK".to_string()));

    cmd!(framed, "DECR", "not_int");
    let response = framed.next().await.unwrap().unwrap();
    assert!(matches!(response, Value::Error(_)), "unexpected response: {:?}", response);

    cmd!(framed, "MSET", "k1", "v1", "k2", "v2");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::SimpleString("OK".to_string()));

    cmd!(framed, "MGET", "k1", "missing", "k2");
    assert_eq!(
        framed.next().await.unwrap().unwrap(),
        Value::Array(Some(vec![
            Value::BulkString(Some(Bytes::from("v1"))),
            Value::Null,
            Value::BulkString(Some(Bytes::from("v2"))),
        ]))
    );

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

    cmd!(framed, "SET", "k1", "v1", "NX");
    assert_eq!(
        framed.next().await.unwrap().unwrap(),
        Value::SimpleString("OK".to_string())
    );

    cmd!(framed, "SET", "k1", "v2", "NX");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Null);

    cmd!(framed, "GET", "k1");
    assert_eq!(
        framed.next().await.unwrap().unwrap(),
        Value::BulkString(Some(Bytes::from("v1")))
    );

    cmd!(framed, "SET", "missing", "v0", "XX");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Null);

    cmd!(framed, "SET", "k1", "v3", "XX");
    assert_eq!(
        framed.next().await.unwrap().unwrap(),
        Value::SimpleString("OK".to_string())
    );

    cmd!(framed, "GET", "k1");
    assert_eq!(
        framed.next().await.unwrap().unwrap(),
        Value::BulkString(Some(Bytes::from("v3")))
    );

    server_handle.abort();
}

#[tokio::test]
async fn test_server_expire() {
    // test expire command
    let (addr, mut server) = start_test_server().await;
    let server_handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr).await.unwrap();
    let mut framed = Framed::new(stream, RespCodec);
    // SET key value
    cmd!(framed, "SET", "temp_key", "temp_value");
    let response = framed.next().await.unwrap().unwrap();
    assert_eq!(response, Value::SimpleString("OK".to_string()));

    // EXPIRE key 1 (expire in 1 second)
    cmd!(framed, "EXPIRE", "temp_key", "1");
    let response = framed.next().await.unwrap().unwrap();
    assert_eq!(response, Value::Integer(1));

    // Wait for 2 seconds
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
    // GET expired key
    cmd!(framed, "GET", "temp_key");
    let response = framed.next().await.unwrap().unwrap();
    assert_eq!(response, Value::Null);

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

    cmd!(framed, "TTL", "missing_key");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(-2));

    cmd!(framed, "PTTL", "missing_key");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(-2));

    cmd!(framed, "SET", "plain_key", "value");
    assert_eq!(
        framed.next().await.unwrap().unwrap(),
        Value::SimpleString("OK".to_string())
    );

    cmd!(framed, "TTL", "plain_key");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(-1));

    cmd!(framed, "PTTL", "plain_key");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(-1));

    cmd!(framed, "EXPIRE", "plain_key", "2");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(1));

    cmd!(framed, "PTTL", "plain_key");
    let pttl = match framed.next().await.unwrap().unwrap() {
        Value::Integer(v) => v,
        other => panic!("unexpected PTTL response: {:?}", other),
    };
    assert!((1..=2000).contains(&pttl), "unexpected PTTL value: {pttl}");

    cmd!(framed, "TTL", "plain_key");
    let ttl = match framed.next().await.unwrap().unwrap() {
        Value::Integer(v) => v,
        other => panic!("unexpected TTL response: {:?}", other),
    };
    assert!((0..=2).contains(&ttl), "unexpected TTL value: {ttl}");

    tokio::time::sleep(tokio::time::Duration::from_millis(2200)).await;

    cmd!(framed, "TTL", "plain_key");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(-2));

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

    cmd!(framed, "RPUSH", "l1", "a", "b", "c");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(3));

    cmd!(framed, "LRANGE", "l1", "-10", "1");
    assert_eq!(
        framed.next().await.unwrap().unwrap(),
        Value::Array(Some(vec![
            Value::BulkString(Some(Bytes::from("a"))),
            Value::BulkString(Some(Bytes::from("b"))),
        ]))
    );

    cmd!(framed, "LRANGE", "l1", "5", "10");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Array(Some(vec![])));

    cmd!(framed, "SADD", "set1", "b", "a", "c", "a");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(3));

    cmd!(framed, "SMEMBERS", "set1");
    assert_eq!(
        framed.next().await.unwrap().unwrap(),
        Value::Array(Some(vec![
            Value::BulkString(Some(Bytes::from("a"))),
            Value::BulkString(Some(Bytes::from("b"))),
            Value::BulkString(Some(Bytes::from("c"))),
        ]))
    );

    cmd!(framed, "SREM", "set1", "a", "b", "c");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(3));

    cmd!(framed, "EXISTS", "set1");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(0));

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

    cmd!(framed, "SET", "evict_me", "value");
    let response = framed.next().await.unwrap().unwrap();
    assert_eq!(response, Value::SimpleString("OK".to_string()));

    cmd!(framed, "EXPIRE", "evict_me", "1");
    let response = framed.next().await.unwrap().unwrap();
    assert_eq!(response, Value::Integer(1));

    assert!(store.get_for_restore("evict_me").is_some());

    tokio::time::sleep(tokio::time::Duration::from_millis(2200)).await;

    assert!(store.get_for_restore("evict_me").is_none());

    server_handle.abort();
}

#[tokio::test]
async fn test_extended_commands_parse_and_execute() {
    let (addr, mut server) = start_test_server().await;
    let server_handle = tokio::spawn(async move {
        server.run().await.unwrap();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let stream = TcpStream::connect(addr).await.unwrap();
    let mut framed = Framed::new(stream, RespCodec);

    cmd!(framed, "APPEND", "s1", "foo");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(3));
    cmd!(framed, "STRLEN", "s1");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(3));

    cmd!(framed, "RPUSH", "l1", "a", "b");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(2));
    cmd!(framed, "LINDEX", "l1", "-1");
    assert_eq!(
        framed.next().await.unwrap().unwrap(),
        Value::BulkString(Some(Bytes::from("b")))
    );

    cmd!(framed, "SADD", "set1", "x", "y");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(2));
    cmd!(framed, "SISMEMBER", "set1", "x");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(1));
    cmd!(framed, "SMEMBERS", "set1");
    assert_eq!(
        framed.next().await.unwrap().unwrap(),
        Value::Array(Some(vec![
            Value::BulkString(Some(Bytes::from("x"))),
            Value::BulkString(Some(Bytes::from("y"))),
        ]))
    );

    cmd!(framed, "HSET", "h1", "f1", "v1", "f2", "v2");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(2));
    cmd!(framed, "HGET", "h1", "f1");
    assert_eq!(
        framed.next().await.unwrap().unwrap(),
        Value::BulkString(Some(Bytes::from("v1")))
    );

    cmd!(framed, "ZADD", "z1", "1", "m1", "2", "m2");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(2));
    cmd!(framed, "ZCARD", "z1");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(2));

    cmd!(framed, "TYPE", "z1");
    assert_eq!(
        framed.next().await.unwrap().unwrap(),
        Value::SimpleString("zset".to_string())
    );
    cmd!(framed, "FLUSHALL");
    assert_eq!(
        framed.next().await.unwrap().unwrap(),
        Value::SimpleString("OK".to_string())
    );
    cmd!(framed, "KEYS", "*");
    assert_eq!(
        framed.next().await.unwrap().unwrap(),
        Value::Array(Some(vec![]))
    );

    server_handle.abort();
}
