use crate::resp::Value;
use bytes::Bytes;

pub fn handle_ping() -> Value {
    Value::SimpleString("PONG".into())
}

pub fn handle_echo(msg: Bytes) -> Value {
    Value::BulkString(Some(msg))
}

pub fn handle_quit() -> Value {
    Value::SimpleString("OK".into())
}

pub fn handle_info() -> Value {
    let info = "# Server\r\n\
         redis_version:0.1.0\r\n\
         redis_mode:standalone\r\n\
         os:Rust\r\n".to_string();
    Value::BulkString(Some(info.into()))
}