use crate::{commands::SetCondition, resp::Value, store::Store};
use bytes::Bytes;

fn bulk_or_null(value: Option<Bytes>) -> Value {
    match value {
        Some(v) => Value::BulkString(Some(v)),
        None => Value::BulkString(None),
    }
}

fn bulk_array(items: Vec<Bytes>) -> Value {
    Value::Array(Some(
        items
            .into_iter()
            .map(|b| Value::BulkString(Some(b)))
            .collect(),
    ))
}

fn int_bool(value: bool) -> Value {
    Value::Integer(if value { 1 } else { 0 })
}

pub fn handle_l_push(store: &Store, key: String, vals: Vec<Bytes>) -> Value {
    if vals.is_empty() {
        return Value::Integer(0);
    }
    let mut len = 0;
    for val in vals.into_iter().rev() {
        len = store.l_push(key.clone(), val);
    }
    Value::Integer(len as i64)
}

pub fn handle_r_push(store: &Store, key: String, vals: Vec<Bytes>) -> Value {
    if vals.is_empty() {
        return Value::Integer(0);
    }
    let mut len = 0;
    for val in vals {
        len = store.r_push(key.clone(), val);
    }
    Value::Integer(len as i64)
}

pub fn handle_l_pop(store: &Store, key: String) -> Value {
    bulk_or_null(store.l_pop(&key))
}

pub fn handle_r_pop(store: &Store, key: String) -> Value {
    bulk_or_null(store.r_pop(&key))
}

pub fn handle_l_len(store: &Store, key: String) -> Value {
    Value::Integer(store.l_len(&key) as i64)
}

pub fn handle_l_index(store: &Store, key: String, index: i64) -> Value {
    bulk_or_null(store.l_index(&key, index))
}

pub fn handle_l_range(store: &Store, key: String, start: i64, stop: i64) -> Value {
    bulk_array(store.l_range(&key, start, stop))
}

pub(crate) fn handle_get(store: &Store, key: String) -> Value {
    bulk_or_null(store.get(&key))
}

pub(crate) fn handle_set(
    store: &Store,
    key: String,
    value: Bytes,
    expire: Option<u64>,
    condition: SetCondition,
) -> Value {
    let exists = store.exists(&key);
    match condition {
        SetCondition::Nx if exists => return Value::Null,
        SetCondition::Xx if !exists => return Value::Null,
        _ => {}
    }

    store.set(key, value, expire);
    Value::SimpleString("OK".to_string())
}

pub(crate) fn handle_append(store: &Store, key: String, value: Bytes) -> Value {
    Value::Integer(store.append(key, value) as i64)
}

pub(crate) fn handle_strlen(store: &Store, key: String) -> Value {
    Value::Integer(store.strlen(&key) as i64)
}

pub(crate) fn handle_type(store: &Store, key: String) -> Value {
    Value::SimpleString(store.type_of(&key).unwrap_or_else(|| "none".to_string()))
}

pub(crate) fn handle_keys(store: &Store, pattern: String) -> Value {
    if pattern != "*" {
        return Value::Error("ERR only '*' pattern is currently supported".to_string());
    }

    let mut keys = store.keys();
    keys.sort_unstable();

    Value::Array(Some(
        keys.into_iter()
            .map(|k| Value::BulkString(Some(Bytes::from(k))))
            .collect(),
    ))
}

pub(crate) fn handle_flushall(store: &Store) -> Value {
    store.flush_all();
    Value::SimpleString("OK".to_string())
}

pub(crate) fn handle_expire(store: &Store, key: String, expire: u64) -> Value {
    if store.expire(&key, expire) {
        Value::Integer(1)
    } else {
        Value::Integer(0)
    }
}

pub(crate) fn handle_ttl(store: &Store, key: String) -> Value {
    Value::Integer(store.ttl(&key))
}

pub(crate) fn handle_pttl(store: &Store, key: String) -> Value {
    Value::Integer(store.pttl(&key))
}

pub(crate) fn handle_incr(store: &Store, key: String) -> Value {
    match store.incr(&key) {
        Ok(v) => Value::Integer(v),
        Err(e) => Value::Error(e),
    }
}

pub(crate) fn handle_decr(store: &Store, key: String) -> Value {
    match store.decr(&key) {
        Ok(v) => Value::Integer(v),
        Err(e) => Value::Error(e),
    }
}

pub(crate) fn handle_mget(store: &Store, keys: Vec<String>) -> Value {
    Value::Array(Some(
        keys.into_iter()
            .map(|key| bulk_or_null(store.get(&key)))
            .collect(),
    ))
}

pub(crate) fn handle_mset(store: &Store, entries: Vec<(String, Bytes)>) -> Value {
    for (key, value) in entries {
        store.set(key, value, None);
    }
    Value::SimpleString("OK".to_string())
}

pub(crate) fn handle_exists(store: &Store, keys: Vec<String>) -> Value {
    let count = keys.iter().filter(|k| store.exists(k)).count() as i64;
    Value::Integer(count)
}

pub(crate) fn handle_del(store: &Store, keys: Vec<String>) -> Value {
    let count = store.del(&keys) as i64;
    Value::Integer(count)
}

pub(crate) fn handle_s_add(store: &Store, key: String, members: Vec<Bytes>) -> Value {
    Value::Integer(store.s_add(key, members) as i64)
}

pub(crate) fn handle_s_rem(store: &Store, key: String, members: Vec<Bytes>) -> Value {
    Value::Integer(store.s_rem(&key, &members) as i64)
}

pub(crate) fn handle_s_members(store: &Store, key: String) -> Value {
    bulk_array(store.s_members(&key))
}

pub(crate) fn handle_s_is_member(store: &Store, key: String, member: Bytes) -> Value {
    int_bool(store.s_is_member(&key, &member))
}

pub(crate) fn handle_s_card(store: &Store, key: String) -> Value {
    Value::Integer(store.s_card(&key) as i64)
}

pub(crate) fn handle_s_pop(store: &Store, key: String, count: Option<usize>) -> Value {
    match count {
        Some(c) => bulk_array(store.s_pop(&key, c)),
        None => {
            let mut values = store.s_pop(&key, 1);
            bulk_or_null(values.pop())
        }
    }
}

pub(crate) fn handle_h_set(store: &Store, key: String, fields: Vec<(String, Bytes)>) -> Value {
    let mut added = 0i64;
    for (field, value) in fields {
        if store.h_set(key.clone(), field, value) {
            added += 1;
        }
    }
    Value::Integer(added)
}

pub(crate) fn handle_h_get(store: &Store, key: String, field: String) -> Value {
    bulk_or_null(store.h_get(&key, &field))
}

pub(crate) fn handle_h_del(store: &Store, key: String, fields: Vec<String>) -> Value {
    Value::Integer(store.h_del(&key, &fields) as i64)
}

pub(crate) fn handle_h_len(store: &Store, key: String) -> Value {
    Value::Integer(store.h_len(&key) as i64)
}

pub(crate) fn handle_h_exists(store: &Store, key: String, field: String) -> Value {
    int_bool(store.h_exists(&key, &field))
}

pub(crate) fn handle_h_keys(store: &Store, key: String) -> Value {
    Value::Array(Some(
        store
            .h_keys(&key)
            .into_iter()
            .map(|field| Value::BulkString(Some(Bytes::from(field))))
            .collect(),
    ))
}

pub(crate) fn handle_h_vals(store: &Store, key: String) -> Value {
    bulk_array(store.h_vals(&key))
}

pub(crate) fn handle_h_get_all(store: &Store, key: String) -> Value {
    let mut pairs: Vec<_> = store.h_get_all(&key).into_iter().collect();
    pairs.sort_by(|a, b| a.0.cmp(&b.0));

    let mut response = Vec::with_capacity(pairs.len() * 2);
    for (field, value) in pairs {
        response.push(Value::BulkString(Some(Bytes::from(field))));
        response.push(Value::BulkString(Some(value)));
    }
    Value::Array(Some(response))
}

pub(crate) fn handle_z_add(store: &Store, key: String, entries: Vec<(f64, Bytes)>) -> Value {
    let mut changed = 0i64;
    for (score, member) in entries {
        if store.z_add(key.clone(), score, member) {
            changed += 1;
        }
    }
    Value::Integer(changed)
}

pub(crate) fn handle_z_range(store: &Store, key: String, start: i64, stop: i64) -> Value {
    Value::Array(Some(
        store
            .z_range(&key, start, stop)
            .into_iter()
            .map(|(member, _)| Value::BulkString(Some(member)))
            .collect(),
    ))
}

pub(crate) fn handle_z_rev_range(store: &Store, key: String, start: i64, stop: i64) -> Value {
    Value::Array(Some(
        store
            .z_rev_range(&key, start, stop)
            .into_iter()
            .map(|(member, _)| Value::BulkString(Some(member)))
            .collect(),
    ))
}

pub(crate) fn handle_z_score(store: &Store, key: String, member: Bytes) -> Value {
    match store.z_score(&key, &member) {
        Some(score) => Value::BulkString(Some(Bytes::from(score.to_string()))),
        None => Value::BulkString(None),
    }
}

pub(crate) fn handle_z_rem(store: &Store, key: String, members: Vec<Bytes>) -> Value {
    Value::Integer(store.z_rem(&key, &members) as i64)
}

pub(crate) fn handle_z_card(store: &Store, key: String) -> Value {
    Value::Integer(store.z_card(&key) as i64)
}

pub(crate) fn handle_z_count(store: &Store, key: String, min_score: f64, max_score: f64) -> Value {
    Value::Integer(store.z_count(&key, min_score, max_score) as i64)
}
