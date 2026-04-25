use crate::store::{StoredValue, Store};
use bytes::Bytes;
use std::collections::HashMap;

impl Store {
    pub fn h_set(&self, key: String, field: String, value: Bytes) -> bool {
        self.check_expiration(&key);

        let mut entry = self.inner.data.entry(key).or_insert_with(|| {
            StoredValue::Hash(HashMap::new())
        });

        match entry.value_mut() {
            StoredValue::Hash(map) => map.insert(field, value).is_none(),
            _ => false,
        }
    }

    pub fn h_get(&self, key: &str, field: &str) -> Option<Bytes> {
        if self.check_expiration(key) {
            return None;
        }
        self.inner.data.get(key).and_then(|entry| {
            match entry.value() {
                StoredValue::Hash(map) => map.get(field).cloned(),
                _ => None,
            }
        })
    }

    pub fn h_get_all(&self, key: &str) -> HashMap<String, Bytes> {
        if self.check_expiration(key) {
            return HashMap::new();
        }
        self.inner.data.get(key).map_or_else(HashMap::new, |entry| {
            match entry.value() {
                StoredValue::Hash(map) => map.clone(),
                _ => HashMap::new(),
            }
        })
    }

    pub fn h_del(&self, key: &str, fields: &[String]) -> usize {
        if self.check_expiration(key) {
            return 0;
        }
        let mut entry = match self.inner.data.get_mut(key) {
            Some(e) => e,
            None => return 0,
        };

        match entry.value_mut() {
            StoredValue::Hash(map) => {
                fields.iter().filter(|f| map.remove(*f).is_some()).count()
            }
            _ => 0,
        }
    }

    pub fn h_len(&self, key: &str) -> usize {
        if self.check_expiration(key) {
            return 0;
        }
        self.inner.data.get(key).map_or(0, |entry| {
            match entry.value() {
                StoredValue::Hash(map) => map.len(),
                _ => 0,
            }
        })
    }

    pub fn h_exists(&self, key: &str, field: &str) -> bool {
        if self.check_expiration(key) {
            return false;
        }
        self.inner.data.get(key).map_or(false, |entry| {
            match entry.value() {
                StoredValue::Hash(map) => map.contains_key(field),
                _ => false,
            }
        })
    }

    pub fn h_keys(&self, key: &str) -> Vec<String> {
        if self.check_expiration(key) {
            return Vec::new();
        }
        self.inner.data.get(key).map_or_else(Vec::new, |entry| {
            match entry.value() {
                StoredValue::Hash(map) => map.keys().cloned().collect(),
                _ => Vec::new(),
            }
        })
    }

    pub fn h_vals(&self, key: &str) -> Vec<Bytes> {
        if self.check_expiration(key) {
            return Vec::new();
        }
        self.inner.data.get(key).map_or_else(Vec::new, |entry| {
            match entry.value() {
                StoredValue::Hash(map) => map.values().cloned().collect(),
                _ => Vec::new(),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash() {
        let store = Store::new();

        assert!(!store.exists("hash"));

        assert!(store.h_set("hash".to_string(), "field1".to_string(), Bytes::from("value1")));
        assert!(store.h_set("hash".to_string(), "field2".to_string(), Bytes::from("value2")));

        assert_eq!(store.h_get("hash", "field1"), Some(Bytes::from("value1")));
        assert_eq!(store.h_get("hash", "field2"), Some(Bytes::from("value2")));

        store.h_set("hash".to_string(), "field1".to_string(), Bytes::from("value3"));
        assert_eq!(store.h_get("hash", "field1"), Some(Bytes::from("value3")));

        assert_eq!(store.h_len("hash"), 2);
        assert_eq!(store.h_exists("hash", "field1"), true);

        let keys = store.h_keys("hash");
        assert_eq!(keys.len(), 2);
        assert!(keys.contains(&"field1".to_string()));
        assert!(keys.contains(&"field2".to_string()));

        let vals = store.h_vals("hash");
        assert_eq!(vals.len(), 2);
        assert!(vals.contains(&Bytes::from("value3")));

        let kv = store.h_get_all("hash");
        assert_eq!(kv.len(), 2);
        assert!(kv.contains_key("field1"));
        assert_eq!(kv.get("field1"), Some(&Bytes::from("value3")));
        assert!(kv.contains_key("field2"));

        store.h_del("hash", &["field1".to_string()]);
        assert_eq!(store.h_get("hash", "field1"), None);

    }
}