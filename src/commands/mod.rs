use crate::{metrics::Metrics, resp::Value, store::Store};
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
    Set(String, Bytes, Option<u64>, SetCondition), // key, value, px/ms, condition
    Del(Vec<String>),
    Exists(Vec<String>),
    Expire(String, u64),
    Ttl(String),
    Pttl(String),
    Incr(String),
    Decr(String),
    MGet(Vec<String>),
    MSet(Vec<(String, Bytes)>),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SetCondition {
    None,
    Nx,
    Xx,
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
            b"INFO" => Ok(Command::Info),
            b"GET" => parse_get(&args),
            b"SET" => parse_set(&args),
            b"DEL" => parse_del(&args),
            b"EXISTS" => parse_exists(&args),
            b"EXPIRE" => parse_expire(&args),
            b"TTL" => parse_ttl(&args),
            b"PTTL" => parse_pttl(&args),
            b"INCR" => parse_incr(&args),
            b"DECR" => parse_decr(&args),
            b"MGET" => parse_mget(&args),
            b"MSET" => parse_mset(&args),
            b"APPEND" => parse_append(&args),
            b"STRLEN" => parse_strlen(&args),
            b"TYPE" => parse_type(&args),
            b"KEYS" => parse_keys(&args),
            b"QUIT" => Ok(Command::Quit),
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
    pub fn execute(self, store: &Store, metrics: &Metrics) -> Value {
        match self {
            Command::Ping => conn::handle_ping(),
            Command::Echo(msg) => conn::handle_echo(msg),
            Command::Quit => conn::handle_quit(),
            Command::Info => conn::handle_info(metrics),

            Command::Get(k) => data::handle_get(store, k),
            Command::Set(k, v, px, condition) => data::handle_set(store, k, v, px, condition),
            Command::Del(keys) => data::handle_del(store, keys),
            Command::Exists(keys) => data::handle_exists(store, keys),
            Command::Expire(k, secs) => data::handle_expire(store, k, secs),
            Command::Ttl(k) => data::handle_ttl(store, k),
            Command::Pttl(k) => data::handle_pttl(store, k),
            Command::Incr(k) => data::handle_incr(store, k),
            Command::Decr(k) => data::handle_decr(store, k),
            Command::MGet(keys) => data::handle_mget(store, keys),
            Command::MSet(entries) => data::handle_mset(store, entries),
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
            Command::ZCount(k, min_score, max_score) => {
                data::handle_z_count(store, k, min_score, max_score)
            }
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
        return Err(Value::Error(
            "ERR wrong number of arguments for 'get'".into(),
        ));
    }
    let key = bulk_to_string(&args[1])?;
    Ok(Command::Get(key))
}

fn parse_set(args: &[Value]) -> Result<Command, Value> {
    if args.len() < 3 {
        return Err(Value::Error(
            "ERR wrong number of arguments for 'set'".into(),
        ));
    }
    let key = bulk_to_string(&args[1])?;
    let val = bulk_to_bytes(&args[2])?;

    let mut px = None;
    let mut condition = SetCondition::None;
    let mut idx = 3;

    while idx < args.len() {
        let opt = bulk_to_string(&args[idx])?.to_ascii_uppercase();
        match opt.as_str() {
            "EX" | "PX" => {
                if px.is_some() || idx + 1 >= args.len() {
                    return Err(Value::Error("ERR syntax error".into()));
                }

                let ttl = bulk_to_u64(&args[idx + 1])?;
                px = if opt == "EX" {
                    Some(ttl.checked_mul(1000).ok_or_else(|| {
                        Value::Error("ERR invalid expire time in 'set' command".into())
                    })?)
                } else {
                    Some(ttl)
                };

                idx += 2;
            }
            "NX" => {
                if condition != SetCondition::None {
                    return Err(Value::Error("ERR syntax error".into()));
                }
                condition = SetCondition::Nx;
                idx += 1;
            }
            "XX" => {
                if condition != SetCondition::None {
                    return Err(Value::Error("ERR syntax error".into()));
                }
                condition = SetCondition::Xx;
                idx += 1;
            }
            _ => return Err(Value::Error("ERR syntax error".into())),
        }
    }

    Ok(Command::Set(key, val, px, condition))
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

fn parse_ttl(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 2 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    Ok(Command::Ttl(bulk_to_string(&args[1])?))
}

fn parse_pttl(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 2 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    Ok(Command::Pttl(bulk_to_string(&args[1])?))
}

fn parse_incr(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 2 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    let key = bulk_to_string(&args[1])?;
    Ok(Command::Incr(key))
}

fn parse_decr(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 2 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    let key = bulk_to_string(&args[1])?;
    Ok(Command::Decr(key))
}

fn parse_mget(args: &[Value]) -> Result<Command, Value> {
    if args.len() < 2 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    let keys: Result<Vec<_>, _> = args[1..].iter().map(bulk_to_string).collect();
    Ok(Command::MGet(keys?))
}

fn parse_mset(args: &[Value]) -> Result<Command, Value> {
    if args.len() < 3 || args.len() % 2 == 0 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }

    let mut entries = Vec::with_capacity((args.len() - 1) / 2);
    for pair in args[1..].chunks(2) {
        entries.push((bulk_to_string(&pair[0])?, bulk_to_bytes(&pair[1])?));
    }

    Ok(Command::MSet(entries))
}

fn parse_append(args: &[Value]) -> Result<Command, Value> {
    if args.len() != 3 {
        return Err(Value::Error("ERR wrong number of arguments".into()));
    }
    Ok(Command::Append(
        bulk_to_string(&args[1])?,
        bulk_to_bytes(&args[2])?,
    ))
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
    Ok(Command::LIndex(
        bulk_to_string(&args[1])?,
        bulk_to_i64(&args[2])?,
    ))
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
        Value::BulkString(Some(b)) => String::from_utf8_lossy(b)
            .parse()
            .map_err(|_| Value::Error("ERR value is not integer".into())),
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
            Command::Set(key, value, Some(ttl_ms), condition) => {
                assert_eq!(key, "k");
                assert_eq!(value, Bytes::from("v"));
                assert_eq!(ttl_ms, 2000);
                assert_eq!(condition, SetCondition::None);
            }
            other => panic!("unexpected command: {:?}", other),
        }
    }

    #[test]
    fn test_parse_set_with_nx_and_px() {
        let cmd =
            Command::try_from(vec![b("SET"), b("k"), b("v"), b("NX"), b("PX"), b("100")]).unwrap();
        match cmd {
            Command::Set(key, value, Some(ttl_ms), condition) => {
                assert_eq!(key, "k");
                assert_eq!(value, Bytes::from("v"));
                assert_eq!(ttl_ms, 100);
                assert_eq!(condition, SetCondition::Nx);
            }
            other => panic!("unexpected command: {:?}", other),
        }
    }

    #[test]
    fn test_parse_set_rejects_nx_xx_combination() {
        let result = Command::try_from(vec![b("SET"), b("k"), b("v"), b("NX"), b("XX")]);
        assert!(matches!(result, Err(Value::Error(_))));
    }

    #[test]
    fn test_parse_set_rejects_unknown_option() {
        let result = Command::try_from(vec![b("SET"), b("k"), b("v"), b("KEEPTTL")]);
        assert!(matches!(result, Err(Value::Error(_))));
    }

    #[test]
    fn test_parse_ttl() {
        let cmd = Command::try_from(vec![b("TTL"), b("k")]).unwrap();
        match cmd {
            Command::Ttl(key) => assert_eq!(key, "k"),
            other => panic!("unexpected command: {:?}", other),
        }
    }

    #[test]
    fn test_parse_pttl() {
        let cmd = Command::try_from(vec![b("PTTL"), b("k")]).unwrap();
        match cmd {
            Command::Pttl(key) => assert_eq!(key, "k"),
            other => panic!("unexpected command: {:?}", other),
        }
    }

    #[test]
    fn test_parse_mget() {
        let cmd = Command::try_from(vec![b("MGET"), b("k1"), b("k2")]).unwrap();
        match cmd {
            Command::MGet(keys) => assert_eq!(keys, vec!["k1".to_string(), "k2".to_string()]),
            other => panic!("unexpected command: {:?}", other),
        }
    }

    #[test]
    fn test_parse_mset() {
        let cmd = Command::try_from(vec![b("MSET"), b("k1"), b("v1"), b("k2"), b("v2")]).unwrap();
        match cmd {
            Command::MSet(entries) => assert_eq!(
                entries,
                vec![
                    ("k1".to_string(), Bytes::from("v1")),
                    ("k2".to_string(), Bytes::from("v2")),
                ]
            ),
            other => panic!("unexpected command: {:?}", other),
        }
    }

    #[test]
    fn test_parse_mset_rejects_odd_pairs() {
        let result = Command::try_from(vec![b("MSET"), b("k1"), b("v1"), b("k2")]);
        assert!(matches!(result, Err(Value::Error(_))));
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

    #[test]
    fn test_parse_spop_rejects_too_many_args() {
        let result = Command::try_from(vec![b("SPOP"), b("s"), b("1"), b("2")]);
        assert!(matches!(result, Err(Value::Error(_))));
    }

    #[test]
    fn test_parse_zrange_rejects_non_integer_index() {
        let result = Command::try_from(vec![b("ZRANGE"), b("z"), b("0"), b("oops")]);
        assert!(matches!(result, Err(Value::Error(_))));
    }

    #[test]
    fn test_parse_zcount_rejects_wrong_arity() {
        let result = Command::try_from(vec![b("ZCOUNT"), b("z"), b("1")]);
        assert!(matches!(result, Err(Value::Error(_))));
    }

    // ── Connection commands ────────────────────────────────────────

    #[test]
    fn test_parse_ping() {
        let cmd = Command::try_from(vec![b("PING")]).unwrap();
        assert!(matches!(cmd, Command::Ping));
    }

    #[test]
    fn test_parse_echo() {
        let cmd = Command::try_from(vec![b("ECHO"), b("hello")]).unwrap();
        match cmd {
            Command::Echo(msg) => assert_eq!(msg, Bytes::from("hello")),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_parse_echo_rejects_extra_args() {
        let r = Command::try_from(vec![b("ECHO"), b("a"), b("b")]);
        assert!(matches!(r, Err(Value::Error(_))));
    }

    #[test]
    fn test_parse_quit() {
        let cmd = Command::try_from(vec![b("QUIT")]).unwrap();
        assert!(matches!(cmd, Command::Quit));
    }

    #[test]
    fn test_parse_info() {
        let cmd = Command::try_from(vec![b("INFO")]).unwrap();
        assert!(matches!(cmd, Command::Info));
    }

    // ── Key commands ───────────────────────────────────────────────

    #[test]
    fn test_parse_del() {
        let cmd = Command::try_from(vec![b("DEL"), b("k1"), b("k2")]).unwrap();
        match cmd {
            Command::Del(keys) => assert_eq!(keys, vec!["k1", "k2"]),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_parse_del_minimal() {
        let cmd = Command::try_from(vec![b("DEL"), b("k")]).unwrap();
        match cmd {
            Command::Del(keys) => assert_eq!(keys, vec!["k"]),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_parse_exists() {
        let cmd = Command::try_from(vec![b("EXISTS"), b("k1"), b("k2")]).unwrap();
        match cmd {
            Command::Exists(keys) => assert_eq!(keys, vec!["k1", "k2"]),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_parse_expire() {
        let cmd = Command::try_from(vec![b("EXPIRE"), b("k"), b("10")]).unwrap();
        match cmd {
            Command::Expire(key, ms) => {
                assert_eq!(key, "k");
                assert_eq!(ms, 10000); // EXPIRE converts seconds to milliseconds
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_parse_expire_rejects_non_integer() {
        let r = Command::try_from(vec![b("EXPIRE"), b("k"), b("oops")]);
        assert!(matches!(r, Err(Value::Error(_))));
    }

    #[test]
    fn test_parse_incr() {
        let cmd = Command::try_from(vec![b("INCR"), b("c")]).unwrap();
        match cmd {
            Command::Incr(key) => assert_eq!(key, "c"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_parse_decr() {
        let cmd = Command::try_from(vec![b("DECR"), b("c")]).unwrap();
        match cmd {
            Command::Decr(key) => assert_eq!(key, "c"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_parse_append() {
        let cmd = Command::try_from(vec![b("APPEND"), b("k"), b("val")]).unwrap();
        match cmd {
            Command::Append(key, val) => {
                assert_eq!(key, "k");
                assert_eq!(val, Bytes::from("val"));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_parse_strlen() {
        let cmd = Command::try_from(vec![b("STRLEN"), b("k")]).unwrap();
        match cmd {
            Command::StrLen(key) => assert_eq!(key, "k"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_parse_type_cmd() {
        let cmd = Command::try_from(vec![b("TYPE"), b("k")]).unwrap();
        match cmd {
            Command::Type(key) => assert_eq!(key, "k"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_parse_keys() {
        let cmd = Command::try_from(vec![b("KEYS"), b("*")]).unwrap();
        match cmd {
            Command::Keys(pattern) => assert_eq!(pattern, "*"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_parse_flushall() {
        let cmd = Command::try_from(vec![b("FLUSHALL")]).unwrap();
        assert!(matches!(cmd, Command::FlushAll));
    }

    // ── List commands ──────────────────────────────────────────────

    #[test]
    fn test_parse_lpush() {
        let cmd =
            Command::try_from(vec![b("LPUSH"), b("l"), b("a"), b("b")]).unwrap();
        match cmd {
            Command::LPush(key, vals) => {
                assert_eq!(key, "l");
                assert_eq!(vals, vec![Bytes::from("a"), Bytes::from("b")]);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_parse_rpush() {
        let cmd = Command::try_from(vec![b("RPUSH"), b("l"), b("x")]).unwrap();
        match cmd {
            Command::RPush(key, vals) => {
                assert_eq!(key, "l");
                assert_eq!(vals, vec![Bytes::from("x")]);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_parse_lpop() {
        let cmd = Command::try_from(vec![b("LPOP"), b("l")]).unwrap();
        match cmd {
            Command::LPop(key) => assert_eq!(key, "l"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_parse_rpop() {
        let cmd = Command::try_from(vec![b("RPOP"), b("l")]).unwrap();
        match cmd {
            Command::RPop(key) => assert_eq!(key, "l"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_parse_llen() {
        let cmd = Command::try_from(vec![b("LLEN"), b("l")]).unwrap();
        match cmd {
            Command::LLen(key) => assert_eq!(key, "l"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_parse_lindex() {
        let cmd = Command::try_from(vec![b("LINDEX"), b("l"), b("0")]).unwrap();
        match cmd {
            Command::LIndex(key, idx) => {
                assert_eq!(key, "l");
                assert_eq!(idx, 0);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    // ── Set commands ───────────────────────────────────────────────

    #[test]
    fn test_parse_srem() {
        let cmd = Command::try_from(vec![b("SREM"), b("s"), b("a"), b("b")]).unwrap();
        match cmd {
            Command::SRem(key, members) => {
                assert_eq!(key, "s");
                assert_eq!(members, vec![Bytes::from("a"), Bytes::from("b")]);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_parse_smembers() {
        let cmd = Command::try_from(vec![b("SMEMBERS"), b("s")]).unwrap();
        match cmd {
            Command::SMembers(key) => assert_eq!(key, "s"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_parse_sismember() {
        let cmd = Command::try_from(vec![b("SISMEMBER"), b("s"), b("m")]).unwrap();
        match cmd {
            Command::SIsMember(key, m) => {
                assert_eq!(key, "s");
                assert_eq!(m, Bytes::from("m"));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_parse_scard() {
        let cmd = Command::try_from(vec![b("SCARD"), b("s")]).unwrap();
        match cmd {
            Command::SCard(key) => assert_eq!(key, "s"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    // ── Hash commands ──────────────────────────────────────────────

    #[test]
    fn test_parse_hget() {
        let cmd = Command::try_from(vec![b("HGET"), b("h"), b("f1")]).unwrap();
        match cmd {
            Command::HGet(key, field) => {
                assert_eq!(key, "h");
                assert_eq!(field, "f1");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_parse_hdel() {
        let cmd = Command::try_from(vec![b("HDEL"), b("h"), b("f1"), b("f2")]).unwrap();
        match cmd {
            Command::HDel(key, fields) => {
                assert_eq!(key, "h");
                assert_eq!(fields, vec!["f1", "f2"]);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_parse_hlen() {
        let cmd = Command::try_from(vec![b("HLEN"), b("h")]).unwrap();
        match cmd {
            Command::HLen(key) => assert_eq!(key, "h"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_parse_hexists() {
        let cmd = Command::try_from(vec![b("HEXISTS"), b("h"), b("f1")]).unwrap();
        match cmd {
            Command::HExists(key, field) => {
                assert_eq!(key, "h");
                assert_eq!(field, "f1");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_parse_hkeys() {
        let cmd = Command::try_from(vec![b("HKEYS"), b("h")]).unwrap();
        match cmd {
            Command::HKeys(key) => assert_eq!(key, "h"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_parse_hvals() {
        let cmd = Command::try_from(vec![b("HVALS"), b("h")]).unwrap();
        match cmd {
            Command::HVals(key) => assert_eq!(key, "h"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_parse_hgetall() {
        let cmd = Command::try_from(vec![b("HGETALL"), b("h")]).unwrap();
        match cmd {
            Command::HGetAll(key) => assert_eq!(key, "h"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    // ── Sorted set commands ────────────────────────────────────────

    #[test]
    fn test_parse_zscore() {
        let cmd = Command::try_from(vec![b("ZSCORE"), b("z"), b("m1")]).unwrap();
        match cmd {
            Command::ZScore(key, member) => {
                assert_eq!(key, "z");
                assert_eq!(member, Bytes::from("m1"));
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_parse_zrem() {
        let cmd = Command::try_from(vec![b("ZREM"), b("z"), b("m1"), b("m2")]).unwrap();
        match cmd {
            Command::ZRem(key, members) => {
                assert_eq!(key, "z");
                assert_eq!(members, vec![Bytes::from("m1"), Bytes::from("m2")]);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    // ── Arity / error helpers ──────────────────────────────────────

    #[test]
    fn test_parse_unknown_command() {
        let r = Command::try_from(vec![b("BOGUS")]);
        assert!(matches!(r, Err(Value::Error(e)) if e.contains("unknown command")));
    }

    #[test]
    fn test_parse_empty_args() {
        let r = Command::try_from(vec![]);
        assert!(matches!(r, Err(Value::Error(e)) if e.contains("empty command")));
    }

    #[test]
    fn test_parse_invalid_cmd_type() {
        let r = Command::try_from(vec![Value::SimpleString("PING".into())]);
        assert!(matches!(r, Err(Value::Error(e)) if e.contains("invalid command")));
    }
}
