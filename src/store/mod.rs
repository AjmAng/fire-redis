use bytes::Bytes;
use dashmap::DashMap;
use ordered_float::OrderedFloat;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::Instant;

pub mod hash;
pub mod list;
pub mod set;
pub mod sorted_set;
pub mod string;

#[derive(Clone, Debug)]
pub enum StoredValue {
    String(Bytes),
    List(VecDeque<Bytes>),
    Set(HashSet<Bytes>),
    Hash(HashMap<String, Bytes>),
    SortedSet {
        scores: HashMap<Bytes, OrderedFloat<f64>>,
        tree: BTreeMap<OrderedFloat<f64>, HashSet<Bytes>>,
    },
}

#[derive(Debug)]
struct StoreInner {
    data: DashMap<String, StoredValue>,
    expirations: DashMap<String, Instant>,
}

#[derive(Clone, Debug)]
pub struct Store {
    inner: Arc<StoreInner>,
}

impl Store {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(StoreInner {
                data: DashMap::new(),
                expirations: DashMap::new(),
            }),
        }
    }

    pub(crate) fn check_expiration(&self, key: &str) -> bool {
        let expire_at = self
            .inner
            .expirations
            .get(key)
            .map(|expire_entry| *expire_entry.value());

        if let Some(expire_at) = expire_at {
            if Instant::now() >= expire_at {
                self.inner.data.remove(key);
                self.inner.expirations.remove(key);
                return true;
            }
        }

        false
    }

    pub(crate) fn set_expiration(&self, key: &str, expire_ms: Option<u64>) {
        if let Some(ms) = expire_ms {
            let expire_at = Instant::now() + std::time::Duration::from_millis(ms);
            self.inner.expirations.insert(key.to_string(), expire_at);
        } else {
            self.inner.expirations.remove(key);
        }
    }

    /// Remove all expired keys and return how many data keys were evicted.
    pub fn evict_expired(&self) -> usize {
        let now = Instant::now();
        let expired_keys: Vec<String> = self
            .inner
            .expirations
            .iter()
            .filter_map(|entry| (now >= *entry.value()).then(|| entry.key().clone()))
            .collect();

        let mut evicted = 0;
        for key in expired_keys {
            if self.inner.data.remove(&key).is_some() {
                evicted += 1;
            }
            self.inner.expirations.remove(&key);
        }

        evicted
    }

    /// Get a snapshot of all data and expirations for persistence
    pub async fn snapshot(&self) -> (HashMap<String, StoredValue>, HashMap<String, Instant>) {
        self.evict_expired();

        let data: HashMap<String, StoredValue> = self
            .inner
            .data
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect();

        let expirations: HashMap<String, Instant> = self
            .inner
            .expirations
            .iter()
            .map(|entry| (entry.key().clone(), *entry.value()))
            .collect();

        (data, expirations)
    }

    /// Restore a key-value pair from persistence (without triggering expiration checks)
    pub fn restore(&self, key: &str, value: StoredValue) {
        self.inner.data.insert(key.to_string(), value);
    }

    /// Set a key with a specific TTL (used during RDB load)
    pub fn set_with_ttl(&self, key: &str, value: StoredValue, ttl_ms: u64) {
        self.inner.data.insert(key.to_string(), value);
        let expire_at = Instant::now() + std::time::Duration::from_millis(ttl_ms);
        self.inner.expirations.insert(key.to_string(), expire_at);
    }

    /// Get value for persistence restore (without expiration check)
    pub fn get_for_restore(&self, key: &str) -> Option<StoredValue> {
        self.inner.data.get(key).map(|entry| entry.value().clone())
    }

    /// Clear all data (used during FLUSHALL)
    pub fn clear_all(&self) {
        self.inner.data.clear();
        self.inner.expirations.clear();
    }
}

impl Store {
    pub fn del(&self, keys: &Vec<String>) -> usize {
        let mut count = 0;
        for key in keys {
            if self.inner.data.remove(key).is_some() {
                count += 1;
            }
            self.inner.expirations.remove(key);
        }
        count
    }

    pub fn exists(&self, key: &str) -> bool {
        !self.check_expiration(key) && self.inner.data.contains_key(key)
    }

    pub fn type_of(&self, key: &str) -> Option<String> {
        if self.check_expiration(key) {
            return None;
        }
        self.inner.data.get(key).map(|entry| match entry.value() {
            StoredValue::String(_) => "string".to_string(),
            StoredValue::List(_) => "list".to_string(),
            StoredValue::Set(_) => "set".to_string(),
            StoredValue::Hash(_) => "hash".to_string(),
            StoredValue::SortedSet { .. } => "zset".to_string(),
        })
    }

    pub fn keys(&self) -> Vec<String> {
        self.evict_expired();

        self.inner
            .data
            .iter()
            .map(|entry| entry.key().clone())
            .collect()
    }

    pub fn flush_all(&self) {
        self.inner.data.clear();
        self.inner.expirations.clear();
    }

    pub fn expire(&self, key: &str, expire_ms: u64) -> bool {
        if self.check_expiration(key) {
            return false;
        }

        if self.inner.data.contains_key(key) {
            self.set_expiration(key, Some(expire_ms));
            true
        } else {
            false
        }
    }

    pub fn pttl(&self, key: &str) -> i64 {
        if !self.exists(key) {
            return -2;
        }

        match self
            .inner
            .expirations
            .get(key)
            .map(|expire_entry| *expire_entry.value())
        {
            Some(expire_at) => expire_at
                .saturating_duration_since(Instant::now())
                .as_millis() as i64,
            None => -1,
        }
    }

    pub fn ttl(&self, key: &str) -> i64 {
        match self.pttl(key) {
            -2 => -2,
            -1 => -1,
            ms => ms / 1000,
        }
    }
}

impl Default for Store {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    #[test]
    fn clones_share_state() {
        let store = Store::new();
        let cloned = store.clone();

        store.restore("shared", StoredValue::String(Bytes::from("value")));

        match cloned.get_for_restore("shared") {
            Some(StoredValue::String(value)) => assert_eq!(value, Bytes::from("value")),
            other => panic!("unexpected stored value: {:?}", other),
        }
    }

    #[test]
    fn evict_expired_removes_data_and_ttl() {
        let store = Store::new();
        store.set("temp".to_string(), Bytes::from("value"), Some(10));

        std::thread::sleep(std::time::Duration::from_millis(30));

        assert_eq!(store.evict_expired(), 1);
        assert!(store.get_for_restore("temp").is_none());
        assert!(!store.exists("temp"));
    }
}
