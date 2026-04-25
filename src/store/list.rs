use crate::store::{StoredValue, Store};
use bytes::Bytes;
use std::collections::VecDeque;

impl Store {
    pub fn l_push(&self, key: String, value: Bytes) -> usize {
        self.check_expiration(&key);

        let mut entry = self.inner.data.entry(key).or_insert_with(|| {
            StoredValue::List(VecDeque::new())
        });

        match entry.value_mut() {
            StoredValue::List(list) => {
                list.push_front(value);
                list.len()
            }
            _ => 0,
        }
    }

    pub fn r_push(&self, key: String, value: Bytes) -> usize {
        self.check_expiration(&key);

        let mut entry = self.inner.data.entry(key).or_insert_with(|| {
            StoredValue::List(VecDeque::new())
        });

        match entry.value_mut() {
            StoredValue::List(list) => {
                list.push_back(value);
                list.len()
            }
            _ => 0,
        }
    }

    pub fn l_pop(&self, key: &str) -> Option<Bytes> {
        if self.check_expiration(key) {
            return None;
        }
        let mut entry = self.inner.data.get_mut(key)?;
        match entry.value_mut() {
            StoredValue::List(list) => list.pop_front(),
            _ => None,
        }
    }

    pub fn r_pop(&self, key: &str) -> Option<Bytes> {
        if self.check_expiration(key) {
            return None;
        }
        let mut entry = self.inner.data.get_mut(key)?;
        match entry.value_mut() {
            StoredValue::List(list) => list.pop_back(),
            _ => None,
        }
    }

    pub fn l_range(&self, key: &str, start: i64, stop: i64) -> Vec<Bytes> {
        if self.check_expiration(key) {
            return Vec::new();
        }
        let entry = match self.inner.data.get(key) {
            Some(e) => e,
            None => return Vec::new(),
        };

        match entry.value() {
            StoredValue::List(list) => {
                let len = list.len() as i64;
                let start = if start < 0 { len + start } else { start }.max(0) as usize;
                let stop = if stop < 0 { len + stop } else { stop }.min(len - 1) as usize;

                if start > stop || start >= list.len() {
                    return Vec::new();
                }
                list.range(start..=stop).cloned().collect()
            }
            _ => Vec::new(),
        }
    }

    pub fn l_len(&self, key: &str) -> usize {
        if self.check_expiration(key) {
            return 0;
        }
        self.inner.data.get(key).map_or(0, |entry| {
            match entry.value() {
                StoredValue::List(list) => list.len(),
                _ => 0,
            }
        })
    }

    pub fn l_index(&self, key: &str, index: i64) -> Option<Bytes> {
        if self.check_expiration(key) {
            return None;
        }
        self.inner.data.get(key).and_then(|entry| {
            match entry.value() {
                StoredValue::List(list) => {
                    let idx = if index < 0 {
                        list.len() as i64 + index
                    } else {
                        index
                    } as usize;
                    list.get(idx).cloned()
                }
                _ => None,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_push_pop_order() {
        let store = Store::new();

        assert_eq!(store.r_push("l".to_string(), Bytes::from("a")), 1);
        assert_eq!(store.r_push("l".to_string(), Bytes::from("b")), 2);
        assert_eq!(store.l_push("l".to_string(), Bytes::from("0")), 3);

        assert_eq!(store.l_pop("l"), Some(Bytes::from("0")));
        assert_eq!(store.r_pop("l"), Some(Bytes::from("b")));
        assert_eq!(store.r_pop("l"), Some(Bytes::from("a")));
        assert_eq!(store.r_pop("l"), None);
    }

    #[test]
    fn test_lrange_and_lindex_with_negative_offsets() {
        let store = Store::new();
        store.r_push("l".to_string(), Bytes::from("a"));
        store.r_push("l".to_string(), Bytes::from("b"));
        store.r_push("l".to_string(), Bytes::from("c"));

        assert_eq!(store.l_index("l", -1), Some(Bytes::from("c")));
        assert_eq!(store.l_index("l", -2), Some(Bytes::from("b")));

        assert_eq!(
            store.l_range("l", -2, -1),
            vec![Bytes::from("b"), Bytes::from("c")]
        );
    }
}
