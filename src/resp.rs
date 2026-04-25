use bytes::{Buf, Bytes, BytesMut};
use std::io;
use thiserror::Error;
use tokio_util::codec::{Decoder, Encoder};

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    SimpleString(String),
    Error(String),
    Integer(i64),
    BulkString(Option<Bytes>),
    Array(Option<Vec<Value>>),
    Null,
}

#[derive(Error, Debug)]
pub enum RespError {
    #[error("Incomplete")]
    Incomplete,
    #[error("Invalid format: {0}")]
    Invalid(String),
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}

pub struct RespCodec;

impl Decoder for RespCodec {
    type Item = Value;
    type Error = RespError;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.is_empty() {
            return Ok(None);
        }

        // Parse from immutable bytes first; only consume on full-frame success.
        match parse_value(src.as_ref())? {
            Some((value, consumed)) => {
                src.advance(consumed);
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }
}

fn parse_value(buf: &[u8]) -> Result<Option<(Value, usize)>, RespError> {
    if buf.is_empty() {
        return Ok(None);
    }

    match buf[0] {
        b'+' => parse_simple_string(buf),
        b'-' => parse_error(buf),
        b':' => parse_integer(buf),
        b'$' => parse_bulk_string(buf),
        b'*' => parse_array(buf),
        t => Err(RespError::Invalid(format!("Unknown type: {}", t))),
    }
}

fn parse_simple_string(buf: &[u8]) -> Result<Option<(Value, usize)>, RespError> {
    let (line, consumed) = match parse_line(buf, 1) {
        Some(v) => v,
        None => return Ok(None),
    };
    let s = String::from_utf8_lossy(line).to_string();
    Ok(Some((Value::SimpleString(s), consumed)))
}

fn parse_error(buf: &[u8]) -> Result<Option<(Value, usize)>, RespError> {
    let (line, consumed) = match parse_line(buf, 1) {
        Some(v) => v,
        None => return Ok(None),
    };
    let s = String::from_utf8_lossy(line).to_string();
    Ok(Some((Value::Error(s), consumed)))
}

fn parse_integer(buf: &[u8]) -> Result<Option<(Value, usize)>, RespError> {
    let (line, consumed) = match parse_line(buf, 1) {
        Some(v) => v,
        None => return Ok(None),
    };
    let s = std::str::from_utf8(line)
        .map_err(|_| RespError::Invalid("Invalid integer bytes".to_string()))?;
    let num = s
        .parse::<i64>()
        .map_err(|_| RespError::Invalid(format!("Invalid integer: {}", s)))?;
    Ok(Some((Value::Integer(num), consumed)))
}

fn parse_bulk_string(buf: &[u8]) -> Result<Option<(Value, usize)>, RespError> {
    let (line, mut offset) = match parse_line(buf, 1) {
        Some(v) => v,
        None => return Ok(None),
    };

    let s = std::str::from_utf8(line)
        .map_err(|_| RespError::Invalid("Invalid bulk length bytes".to_string()))?;
    let len = s
        .parse::<i64>()
        .map_err(|_| RespError::Invalid(format!("Invalid bulk length: {}", s)))?;

    if len < 0 {
        return Ok(Some((Value::Null, offset)));
    }

    let len = len as usize;
    if buf.len() < offset + len + 2 {
        return Ok(None);
    }

    let data = &buf[offset..offset + len];
    offset += len;

    if &buf[offset..offset + 2] != b"\r\n" {
        return Err(RespError::Invalid("Bulk string missing CRLF terminator".to_string()));
    }
    offset += 2;

    Ok(Some((Value::BulkString(Some(Bytes::copy_from_slice(data))), offset)))
}

fn parse_array(buf: &[u8]) -> Result<Option<(Value, usize)>, RespError> {
    let (line, mut offset) = match parse_line(buf, 1) {
        Some(v) => v,
        None => return Ok(None),
    };

    let s = std::str::from_utf8(line)
        .map_err(|_| RespError::Invalid("Invalid array length bytes".to_string()))?;
    let len = s
        .parse::<i64>()
        .map_err(|_| RespError::Invalid(format!("Invalid array length: {}", s)))?;

    if len < 0 {
        return Ok(Some((Value::Null, offset)));
    }

    let mut items = Vec::with_capacity(len as usize);
    for _ in 0..len {
        match parse_value(&buf[offset..])? {
            Some((value, consumed)) => {
                items.push(value);
                offset += consumed;
            }
            None => return Ok(None),
        }
    }

    Ok(Some((Value::Array(Some(items)), offset)))
}

// Returns (line_without_crlf, total_consumed_from_buf_start).
fn parse_line(buf: &[u8], start: usize) -> Option<(&[u8], usize)> {
    if start > buf.len() {
        return None;
    }
    let rel = find_crlf(&buf[start..])?;
    let end = start + rel;
    let consumed = end + 2;
    Some((&buf[start..end], consumed))
}

fn find_crlf(buf: &[u8]) -> Option<usize> {
    buf.windows(2).position(|w| w == b"\r\n")
}

// Encoder: serialize value to RESP format
impl Encoder<Value> for RespCodec {
    type Error = RespError;

    fn encode(&mut self, item: Value, dst: &mut BytesMut) -> Result<(), Self::Error> {
        match item {
            Value::SimpleString(s) => dst.extend_from_slice(format!("+{}\r\n", s).as_bytes()),
            Value::Error(s) => dst.extend_from_slice(format!("-{}\r\n", s).as_bytes()),
            Value::Integer(i) => dst.extend_from_slice(format!(":{}\r\n", i).as_bytes()),
            Value::BulkString(None) | Value::Null => dst.extend_from_slice(b"$-1\r\n"),
            Value::BulkString(Some(b)) => {
                dst.extend_from_slice(format!("${}\r\n", b.len()).as_bytes());
                dst.extend_from_slice(&b);
                dst.extend_from_slice(b"\r\n");
            }
            Value::Array(None) => dst.extend_from_slice(b"*-1\r\n"),
            Value::Array(Some(arr)) => {
                dst.extend_from_slice(format!("*{}\r\n", arr.len()).as_bytes());
                for item in arr {
                    self.encode(item, dst)?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_string() {
        let mut codec = RespCodec;
        let mut buf = BytesMut::new();
        codec
            .encode(Value::SimpleString("OK".to_string()), &mut buf)
            .unwrap();
        assert_eq!(buf, b"+OK\r\n"[..]);
    }

    #[test]
    fn test_error() {
        let mut codec = RespCodec;
        let mut buf = BytesMut::new();
        codec
            .encode(Value::Error("ERR".to_string()), &mut buf)
            .unwrap();
        assert_eq!(buf, b"-ERR\r\n"[..]);
    }

    #[test]
    fn test_integer() {
        let mut codec = RespCodec;
        let mut buf = BytesMut::new();
        codec.encode(Value::Integer(123), &mut buf).unwrap();
        assert_eq!(buf, b":123\r\n"[..]);
    }

    #[test]
    fn test_bulk_string() {
        let mut codec = RespCodec;
        let mut buf = BytesMut::new();
        codec
            .encode(Value::BulkString(Some(Bytes::from("hello"))), &mut buf)
            .unwrap();
        assert_eq!(buf, b"$5\r\nhello\r\n"[..]);
    }

    #[test]
    fn test_array() {
        let mut codec = RespCodec;
        let mut buf = BytesMut::new();
        codec
            .encode(
                Value::Array(Some(vec![
                    Value::SimpleString("foo".to_string()),
                    Value::Integer(42),
                ])),
                &mut buf,
            )
            .unwrap();
        assert_eq!(buf, b"*2\r\n+foo\r\n:42\r\n"[..]);
    }

    #[test]
    fn test_decode_fragmented_ping_array_does_not_consume_partial() {
        let mut codec = RespCodec;
        let mut buf = BytesMut::from(&b"*1\r\n$4\r\nPI"[..]);
        let snap = buf.clone();

        let first = codec.decode(&mut buf).unwrap();
        assert!(first.is_none());
        assert_eq!(buf, snap);

        buf.extend_from_slice(b"NG\r\n");
        let second = codec.decode(&mut buf).unwrap();
        assert_eq!(
            second,
            Some(Value::Array(Some(vec![Value::BulkString(Some(Bytes::from("PING")))])))
        );
        assert!(buf.is_empty());
    }

    #[test]
    fn test_decode_multiple_frames_in_single_buffer() {
        let mut codec = RespCodec;
        let mut buf = BytesMut::from(&b"+OK\r\n:1\r\n"[..]);

        let v1 = codec.decode(&mut buf).unwrap();
        assert_eq!(v1, Some(Value::SimpleString("OK".to_string())));

        let v2 = codec.decode(&mut buf).unwrap();
        assert_eq!(v2, Some(Value::Integer(1)));

        assert!(buf.is_empty());
    }
}
