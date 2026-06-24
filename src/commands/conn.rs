use crate::{metrics::Metrics, resp::Value};
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

pub fn handle_info(metrics: &Metrics) -> Value {
    let info = metrics.info_string();
    Value::BulkString(Some(info.into()))
}
