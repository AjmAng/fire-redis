use fire_redis::{Server, RespCodec, Value};
use tokio::net::TcpStream;
use tokio_util::codec::Framed;
use futures::{SinkExt, StreamExt};
use bytes::Bytes;

async fn start_test_server() -> (std::net::SocketAddr, Server) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    
    let server = Server::from_listener(listener).await.unwrap();

    (addr, server)
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
    assert_eq!(response, Value::BulkString(Some(Bytes::from("Hello, Redis!"))));

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
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::BulkString(Some(Bytes::from("b"))));

    cmd!(framed, "SADD", "set1", "x", "y");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(2));
    cmd!(framed, "SISMEMBER", "set1", "x");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(1));

    cmd!(framed, "HSET", "h1", "f1", "v1", "f2", "v2");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(2));
    cmd!(framed, "HGET", "h1", "f1");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::BulkString(Some(Bytes::from("v1"))));

    cmd!(framed, "ZADD", "z1", "1", "m1", "2", "m2");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(2));
    cmd!(framed, "ZCARD", "z1");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Integer(2));

    cmd!(framed, "TYPE", "z1");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::SimpleString("zset".to_string()));
    cmd!(framed, "FLUSHALL");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::SimpleString("OK".to_string()));
    cmd!(framed, "KEYS", "*");
    assert_eq!(framed.next().await.unwrap().unwrap(), Value::Array(Some(vec![])));

    server_handle.abort();
}

