use crate::store::{StoredValue, Store};
use bytes::Bytes;

impl Store {
    pub fn get(&self, key: &str) -> Option<Bytes> {
        if self.check_expiration(key) {
            return None;
        }
        let entry = self.inner.data.get(key)?;
        match entry.value() {
            StoredValue::String(b) => Some(b.clone()),
            _ => None,
        }
    }

    pub fn set(&self, key: String, value: Bytes, expire_ms: Option<u64>) {
        self.inner.data.insert(key.clone(), StoredValue::String(value));
        self.set_expiration(&key, expire_ms);
        tracing::debug!("Set key, total keys: {}", self.inner.data.len());
    }

    pub fn append(&self, key: String, value: Bytes) -> usize {
        self.check_expiration(&key);

        let mut entry = self.inner.data.entry(key).or_insert_with(|| {
            StoredValue::String(Bytes::new())
        });

        match entry.value_mut() {
            StoredValue::String(existing) => {
                let mut new_val = existing.to_vec();
                new_val.extend_from_slice(&value);
                let len = new_val.len();
                *existing = Bytes::from(new_val);
                len
            }
            _ => 0,
        }
    }

    pub fn strlen(&self, key: &str) -> usize {
        if self.check_expiration(key) {
            return 0;
        }
        self.inner.data.get(key).map_or(0, |entry| {
            match entry.value() {
                StoredValue::String(b) => b.len(),
                _ => 0,
            }
        })
    }

    pub fn incr(&self, key: &String) -> Result<i64, String> {
        self.check_expiration(&key);

        let mut entry = self.inner.data.entry(key.into()).or_insert_with(|| {
            StoredValue::String(Bytes::from("0"))
        });

        match entry.value_mut() {
            StoredValue::String(b) => {
                let current_str = std::str::from_utf8(b).map_err(|_| "Value is not a valid UTF-8 string".to_string())?;
                let current_num: i64 = current_str.parse().map_err(|_| "Value is not an integer".to_string())?;
                let new_num = current_num.checked_add(1).ok_or("Integer overflow")?;
                *b = Bytes::from(new_num.to_string());
                Ok(new_num)
            }
            _ => Err("Key does not hold a string value".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_expiration() {
        let store = Store::new();
        store.set("key1".to_string(), Bytes::from("value1"), Some(100));
        assert_eq!(store.get("key1"), Some(Bytes::from("value1")));
        std::thread::sleep(std::time::Duration::from_millis(150));
        assert_eq!(store.get("key1"), None);
    }
}