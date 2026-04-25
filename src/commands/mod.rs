use crate::{resp::Value, store::Store};
use bytes::Bytes;

pub mod conn;
pub mod data;

#[derive(Debug)]
pub enum Command {
    Ping,
    Echo(Bytes),
    Quit,
    Info,

    // String
    Get(String),
    Set(String, Bytes, Option<u64>), // key, value, px/ms
    Del(Vec<String>),
    Exists(Vec<String>),
    Expire(String, u64),
    Incr(String),
    Append(String, Bytes),
    StrLen(String),
    Type(String),
    Keys(String),
    FlushAll,

    // List
    LPush(String, Vec<Bytes>),
    RPush(String, Vec<Bytes>),
    LPop(String),
    RPop(String),
    LLen(String),
    LIndex(String, i64),
    LRange(String, i64, i64),

    // Set
    SAdd(String, Vec<Bytes>),
    SRem(String, Vec<Bytes>),
    SMembers(String),
    SIsMember(String, Bytes),
    SCard(String),
    SPop(String, Option<usize>),

    // Hash
    HSet(String, Vec<(String, Bytes)>),
    HGet(String, String),
    HDel(String, Vec<String>),
    HLen(String),
    HExists(String, String),
    HKeys(String),
    HVals(String),
    HGetAll(String),

    // Sorted set
    ZAdd(String, Vec<(f64, Bytes)>),
    ZRange(String, i64, i64),
    ZRevRange(String, i64, i64),
    ZScore(String, Bytes),
    ZRem(String, Vec<Bytes>),
    ZCard(String),
    ZCount(String, f64, f64),

}

impl TryFrom<Vec<Value>> for Command {
    type Error = Value;

    fn try_from(args: Vec<Value>) -> Result<Self, Self::Error> {
        if args.is_empty() {
            return Err(Value::Error("ERR empty command".into()));
        }


        let cmd = match &args[0] {
            Value::BulkString(Some(b)) => b.to_ascii_uppercase(),
            _ => return Err(Value::Error("ERR invalid command".into())),
        };

        match cmd.as_slice() {
            b"PING" => Ok(Command::Ping),
            b"ECHO" => parse_echo(&args),
            b"GET" => parse_get(&args),
            b"SET" => parse_set(&args),
            b"DEL" => parse_del(&args),
            b"EXISTS" => parse_exists(&args),
            b"EXPIRE" => parse_expire(&args),
            b"INCR" => parse_incr(&args),
            b"APPEND" => parse_append(&args),
            b"STRLEN" => parse_strlen(&args),
            b"TYPE" => parse_type(&args),
            b"KEYS" => parse_keys(&args),
            b"FLUSHALL" => parse_flushall(&args),
            b"LPUSH" => parse_l_push(&args),
            b"RPUSH" => parse_r_push(&args),
            b"LPOP" => parse_l_pop(&args),
            b"RPOP" => parse_r_pop(&args),
            b"LLEN" => parse_l_len(&args),
            b"LINDEX" => parse_l_index(&args),
            b"LRANGE" => parse_l_range(&args),
            b"SADD" => parse_s_add(&args),
            b"SREM" => parse_s_rem(&args),
            b"SMEMBERS" => parse_s_members(&args),
            b"SISMEMBER" => parse_s_is_member(&args),
            b"SCARD" => parse_s_card(&args),
            b"SPOP" => parse_s_pop(&args),
            b"HSET" => parse_h_set(&args),
            b"HGET" => parse_h_get(&args),
            b"HDEL" => parse_h_del(&args),
            b"HLEN" => parse_h_len(&args),
            b"HEXISTS" => parse_h_exists(&args),
            b"HKEYS" => parse_h_keys(&args),
            b"HVALS" => parse_h_vals(&args),
            b"HGETALL" => parse_h_get_all(&args),
            b"ZADD" => parse_z_add(&args),
            b"ZRANGE" => parse_z_range(&args),
            b"ZREVRANGE" => parse_z_rev_range(&args),
            b"ZSCORE" => parse_z_score(&args),
            b"ZREM" => parse_z_rem(&args),
            b"ZCARD" => parse_z_card(&args),
            b"ZCOUNT" => parse_z_count(&args),
            _ => Err(Value::Error(format!(
                "ERR unknown command '{}'",
                String::from_utf8_lossy(&cmd)
            ))),
        }
    }
}

impl Command {

    pub fn execute(self, store: &Store) -> Value {
        match self {
            Command::Ping => conn::handle_ping(),
            Command::Echo(msg) => conn::handle_echo(msg),
            Command::Quit => conn::handle_quit(),
            Command::Info => conn::handle_info(),

            Command::Get(k) => data::handle_get(store, k),
            Command::Set(k, v, px) => data::handle_set(store, k, v, px),
            Command::Del(keys) => data::handle_del(store, keys),
            Command::Exists(keys) => data::handle_exists(store, keys),
            Command::Expire(k, secs) => data::handle_expire(store, k, secs),
            Command::Incr(k) => data::handle_incr(store, k),
            Command::Append(k, v) => data::handle_append(store, k, v),
            Command::StrLen(k) => data::handle_strlen(store, k),
            Command::Type(k) => data::handle_type(store, k),
            Command::Keys(pattern) => data::handle_keys(store, pattern),
            Command::FlushAll => data::handle_flushall(store),

            Command::LPush(k, vals) => data::handle_l_push(store, k, vals),
            Command::RPush(k, vals) => data::handle_r_push(store, k, vals),
            Command::LPop(k) => data::handle_l_pop(store, k),
            Command::RPop(k) => data::handle_r_pop(store, k),
            Command::LLen(k) => data::handle_l_len(store, k),
            Command::LIndex(k, idx) => data::handle_l_index(store, k, idx),
            Command::LRange(k, start, stop) => data::handle_l_range(store, k, start, stop),

            Command::SAdd(k, members) => data::handle_s_add(store, k, members),
            Command::SRem(k, members) => data::handle_s_rem(store, k, members),
            Command::SMembers(k) => data::handle_s_members(store, k),
            Command::SIsMember(k, member) => data::handle_s_is_member(store, k, member),
            Command::SCard(k) => data::handle_s_card(store, k),
            Command::SPop(k, count) => data::handle_s_pop(store, k, count),

            Command::HSet(k, fields) => data::handle_h_set(store, k, fields),
            Command::HGet(k, field) => data::handle_h_get(store, k, field),
            Command::HDel(k, fields) => data::handle_h_del(store, k, fields),
            Command::HLen(k) => data::handle_h_len(store, k),
            Command::HExists(k, field) => data::handle_h_exists(store, k, field),
            Command::HKeys(k) => data::handle_h_keys(store, k),
            Command::HVals(k) => data::handle_h_vals(store, k),
            Command::HGetAll(k) => data::handle_h_get_all(store, k),

            Command::ZAdd(k, entries) => data::handle_z_add(store, k, entries),
            Command::ZRange(k, start, stop) => data::handle_z_range(store, k, start, stop),
            Command::ZRevRange(k, start, stop) => data::handle_z_rev_range(store, k, start, stop),
            Command::ZScore(k, member) => data::handle_z_score(store, k, member),
            Command::ZRem(k, members) => data::handle_z_rem(store, k, members),
            Command::ZCard(k) => data::handle_z_card(store, k),
            Command::ZCount(k, min_score, max_score) => data::handle_z_count(store, k, min_score, max_score),
        }
    }
}

fn parse_echo(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 2 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    match &args[1] {
        Value::BulkString(Some(b)) => Ok(Command::Echo(b.clone())),
        _ => Err(Value::Error("ERR value is not string".into())),
    }
}

fn parse_get(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 2 {
        return Err(Value::Error("ERR wrong number of arguments for 'get'".into()));
    }
    let key = bulk_to_string(&args[1])?;
    Ok(Command::Get(key))
}

fn parse_set(args: &[Value]) -> Result<Command, Value> {
    if args.len() < 3 {
        return Err(Value::Error("ERR wrong number of arguments for 'set'".into()));
    }
    let key = bulk_to_string(&args[1])?;
    let val = bulk_to_bytes(&args[2])?;

    let mut px = None;
    if args.len() >= 5 {
        if let Value::BulkString(Some(opt)) = &args[3] {
            let opt_str = String::from_utf8_lossy(opt).to_ascii_uppercase();
            if opt_str == "PX" {
                px = Some(bulk_to_u64(&args[4])?);
            } else if opt_str == "EX" {
                px = Some(bulk_to_u64(&args[4])? * 1000);
            }
        }
    }

    Ok(Command::Set(key, val, px))
}

fn parse_del(args: &[Value]) -> Result<Command, Value> {
    if args.len() < 2 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    let keys: Result<Vec<_>, _> = args[1..].iter().map(bulk_to_string).collect();
    Ok(Command::Del(keys?))
}

fn parse_exists(args: &[Value]) -> Result<Command, Value> {
    if args.len() < 2 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    let keys: Result<Vec<_>, _> = args[1..].iter().map(bulk_to_string).collect();
    Ok(Command::Exists(keys?))
}

fn parse_expire(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 3 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    let key = bulk_to_string(&args[1])?;
    let ttl_ms = bulk_to_u64(&args[2])? * 1000;
    Ok(Command::Expire(key, ttl_ms))
}

fn parse_incr(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 2 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    let key = bulk_to_string(&args[1])?;
    Ok(Command::Incr(key))
}

fn parse_append(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 3 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    Ok(Command::Append(bulk_to_string(&args[1])?, bulk_to_bytes(&args[2])?))
}

fn parse_strlen(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 2 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    Ok(Command::StrLen(bulk_to_string(&args[1])?))
}

fn parse_type(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 2 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    Ok(Command::Type(bulk_to_string(&args[1])?))
}

fn parse_keys(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 2 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    Ok(Command::Keys(bulk_to_string(&args[1])?))
}

fn parse_flushall(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 1 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    Ok(Command::FlushAll)
}

fn parse_l_push(args: &[Value]) -> Result<Command, Value> {
    if args.len() < 3 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    let key = bulk_to_string(&args[1])?;
    let vals: Result<Vec<_>, _> = args[2..].iter().map(bulk_to_bytes).collect();
    Ok(Command::LPush(key, vals?))
}

fn parse_r_push(args: &[Value]) -> Result<Command, Value> {
    if args.len() < 3 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    let key = bulk_to_string(&args[1])?;
    let vals: Result<Vec<_>, _> = args[2..].iter().map(bulk_to_bytes).collect();
    Ok(Command::RPush(key, vals?))
}

fn parse_l_pop(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 2 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    Ok(Command::LPop(bulk_to_string(&args[1])?))
}


fn parse_r_pop(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 2 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    let key = bulk_to_string(&args[1])?;
    Ok(Command::RPop(key))
}

fn parse_l_len(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 2 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    Ok(Command::LLen(bulk_to_string(&args[1])?))
}

fn parse_l_index(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 3 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    Ok(Command::LIndex(bulk_to_string(&args[1])?, bulk_to_i64(&args[2])?))
}

fn parse_l_range(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 4 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    let key = bulk_to_string(&args[1])?;
    let start = bulk_to_i64(&args[2])?;
    let stop = bulk_to_i64(&args[3])?;
    Ok(Command::LRange(key, start, stop))
}

fn parse_s_add(args: &[Value]) -> Result<Command, Value> {
    if args.len() < 3 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    let key = bulk_to_string(&args[1])?;
    let members: Result<Vec<_>, _> = args[2..].iter().map(bulk_to_bytes).collect();
    Ok(Command::SAdd(key, members?))
}

fn parse_s_rem(args: &[Value]) -> Result<Command, Value> {
    if args.len() < 3 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    let key = bulk_to_string(&args[1])?;
    let members: Result<Vec<_>, _> = args[2..].iter().map(bulk_to_bytes).collect();
    Ok(Command::SRem(key, members?))
}

fn parse_s_members(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 2 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    Ok(Command::SMembers(bulk_to_string(&args[1])?))
}

fn parse_s_is_member(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 3 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    Ok(Command::SIsMember(
        bulk_to_string(&args[1])?,
        bulk_to_bytes(&args[2])?,
    ))
}

fn parse_s_card(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 2 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    Ok(Command::SCard(bulk_to_string(&args[1])?))
}

fn parse_s_pop(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 2 && args.len() != 3 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    let key = bulk_to_string(&args[1])?;
    let count = if args.len() == 3 {
        Some(bulk_to_u64(&args[2])? as usize)
    } else {
        None
    };
    Ok(Command::SPop(key, count))
}

fn parse_h_set(args: &[Value]) -> Result<Command, Value> {
    if args.len() < 4 || args.len() % 2 != 0 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    let key = bulk_to_string(&args[1])?;
    let mut fields = Vec::with_capacity((args.len() - 2) / 2);
    for pair in args[2..].chunks(2) {
        fields.push((bulk_to_string(&pair[0])?, bulk_to_bytes(&pair[1])?));
    }
    Ok(Command::HSet(key, fields))
}

fn parse_h_get(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 3 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    Ok(Command::HGet(
        bulk_to_string(&args[1])?,
        bulk_to_string(&args[2])?,
    ))
}

fn parse_h_del(args: &[Value]) -> Result<Command, Value> {
    if args.len() < 3 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    let key = bulk_to_string(&args[1])?;
    let fields: Result<Vec<_>, _> = args[2..].iter().map(bulk_to_string).collect();
    Ok(Command::HDel(key, fields?))
}

fn parse_h_len(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 2 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    Ok(Command::HLen(bulk_to_string(&args[1])?))
}

fn parse_h_exists(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 3 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    Ok(Command::HExists(
        bulk_to_string(&args[1])?,
        bulk_to_string(&args[2])?,
    ))
}

fn parse_h_keys(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 2 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    Ok(Command::HKeys(bulk_to_string(&args[1])?))
}

fn parse_h_vals(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 2 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    Ok(Command::HVals(bulk_to_string(&args[1])?))
}

fn parse_h_get_all(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 2 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    Ok(Command::HGetAll(bulk_to_string(&args[1])?))
}

fn parse_z_add(args: &[Value]) -> Result<Command, Value> {
    if args.len() < 4 || args.len() % 2 != 0 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    let key = bulk_to_string(&args[1])?;
    let mut entries = Vec::with_capacity((args.len() - 2) / 2);
    for pair in args[2..].chunks(2) {
        entries.push((bulk_to_f64(&pair[0])?, bulk_to_bytes(&pair[1])?));
    }
    Ok(Command::ZAdd(key, entries))
}

fn parse_z_range(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 4 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    Ok(Command::ZRange(
        bulk_to_string(&args[1])?,
        bulk_to_i64(&args[2])?,
        bulk_to_i64(&args[3])?,
    ))
}

fn parse_z_rev_range(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 4 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    Ok(Command::ZRevRange(
        bulk_to_string(&args[1])?,
        bulk_to_i64(&args[2])?,
        bulk_to_i64(&args[3])?,
    ))
}

fn parse_z_score(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 3 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    Ok(Command::ZScore(
        bulk_to_string(&args[1])?,
        bulk_to_bytes(&args[2])?,
    ))
}

fn parse_z_rem(args: &[Value]) -> Result<Command, Value> {
    if args.len() < 3 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    let key = bulk_to_string(&args[1])?;
    let members: Result<Vec<_>, _> = args[2..].iter().map(bulk_to_bytes).collect();
    Ok(Command::ZRem(key, members?))
}

fn parse_z_card(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 2 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    Ok(Command::ZCard(bulk_to_string(&args[1])?))
}

fn parse_z_count(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 4 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    Ok(Command::ZCount(
        bulk_to_string(&args[1])?,
        bulk_to_f64(&args[2])?,
        bulk_to_f64(&args[3])?,
    ))
}

fn bulk_to_string(v: &Value) -> Result<String, Value> {
    match v {
        Value::BulkString(Some(b)) => Ok(String::from_utf8_lossy(b).to_string()),
        _ => Err(Value::Error("ERR value is not string".into())),
    }
}

fn bulk_to_bytes(v: &Value) -> Result<Bytes, Value> {
    match v {
        Value::BulkString(Some(b)) => Ok(b.clone()),
        _ => Err(Value::Error("ERR value is not string".into())),
    }
}

fn bulk_to_u64(v: &Value) -> Result<u64, Value> {
    match v {
        Value::BulkString(Some(b)) => {
            String::from_utf8_lossy(b).parse()
                .map_err(|_| Value::Error("ERR value is not integer".into()))
        }
        Value::Integer(i) if *i >= 0 => Ok(*i as u64),
        Value::Integer(_) => Err(Value::Error("ERR value is not integer".into())),
        _ => Err(Value::Error("ERR value is not integer".into())),
    }
}

fn bulk_to_i64(v: &Value) -> Result<i64, Value> {
    match v {
        Value::BulkString(Some(b)) => String::from_utf8_lossy(b)
            .parse()
            .map_err(|_| Value::Error("ERR value is not integer".into())),
        Value::Integer(i) => Ok(*i),
        _ => Err(Value::Error("ERR value is not integer".into())),
    }
}

fn bulk_to_f64(v: &Value) -> Result<f64, Value> {
    match v {
        Value::BulkString(Some(b)) => String::from_utf8_lossy(b)
            .parse()
            .map_err(|_| Value::Error("ERR value is not float".into())),
        Value::Integer(i) => Ok(*i as f64),
        _ => Err(Value::Error("ERR value is not float".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(input: &str) -> Value {
        Value::BulkString(Some(Bytes::from(input.to_string())))
    }

    #[test]
    fn test_parse_set_with_ex_converts_to_ms() {
        let cmd = Command::try_from(vec![b("SET"), b("k"), b("v"), b("EX"), b("2")]).unwrap();
        match cmd {
            Command::Set(key, value, Some(ttl_ms)) => {
                assert_eq!(key, "k");
                assert_eq!(value, Bytes::from("v"));
                assert_eq!(ttl_ms, 2000);
            }
            other => panic!("unexpected command: {:?}", other),
        }
    }

    #[test]
    fn test_parse_lrange_supports_negative_offsets() {
        let cmd = Command::try_from(vec![b("LRANGE"), b("k"), b("-2"), b("-1")]).unwrap();
        match cmd {
            Command::LRange(key, start, stop) => {
                assert_eq!(key, "k");
                assert_eq!(start, -2);
                assert_eq!(stop, -1);
            }
            other => panic!("unexpected command: {:?}", other),
        }
    }

    #[test]
    fn test_parse_hset_rejects_odd_field_value_pairs() {
        let result = Command::try_from(vec![b("HSET"), b("h"), b("f1")]);
        assert!(matches!(result, Err(Value::Error(_))));
    }

    #[test]
    fn test_parse_zadd_rejects_invalid_float_score() {
        let result = Command::try_from(vec![b("ZADD"), b("z"), b("not-float"), b("m1")]);
        assert!(matches!(result, Err(Value::Error(_))));
    }

    #[test]
    fn test_parse_spop_with_count() {
        let cmd = Command::try_from(vec![b("SPOP"), b("s"), b("3")]).unwrap();
        match cmd {
            Command::SPop(key, Some(count)) => {
                assert_eq!(key, "s");
                assert_eq!(count, 3);
            }
            other => panic!("unexpected command: {:?}", other),
        }
    }
}

