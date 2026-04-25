use crate::store::{StoredValue, Store};
use bytes::Bytes;
use std::collections::HashSet;

impl Store {
    pub fn s_add(&self, key: String, members: Vec<Bytes>) -> usize {
        self.check_expiration(&key);

        let mut entry = self.inner.data.entry(key).or_insert_with(|| {
            StoredValue::Set(HashSet::new())
        });

        match entry.value_mut() {
            StoredValue::Set(set) => {
                let mut added = 0;
                for member in members {
                    if set.insert(member) {
                        added += 1;
                    }
                }
                added
            }
            _ => 0,
        }
    }

    pub fn s_rem(&self, key: &str, members: &[Bytes]) -> usize {
        if self.check_expiration(key) {
            return 0;
        }
        let mut entry = match self.inner.data.get_mut(key) {
            Some(e) => e,
            None => return 0,
        };

        match entry.value_mut() {
            StoredValue::Set(set) => {
                members.iter().filter(|m| set.remove(*m)).count()
            }
            _ => 0,
        }
    }

    pub fn s_members(&self, key: &str) -> Vec<Bytes> {
        if self.check_expiration(key) {
            return Vec::new();
        }
        self.inner.data.get(key).map_or_else(Vec::new, |entry| {
            match entry.value() {
                StoredValue::Set(set) => set.iter().cloned().collect(),
                _ => Vec::new(),
            }
        })
    }

    pub fn s_is_member(&self, key: &str, member: &Bytes) -> bool {
        if self.check_expiration(key) {
            return false;
        }
        self.inner.data.get(key).map_or(false, |entry| {
            match entry.value() {
                StoredValue::Set(set) => set.contains(member),
                _ => false,
            }
        })
    }

    pub fn s_card(&self, key: &str) -> usize {
        if self.check_expiration(key) {
            return 0;
        }
        self.inner.data.get(key).map_or(0, |entry| {
            match entry.value() {
                StoredValue::Set(set) => set.len(),
                _ => 0,
            }
        })
    }

    pub fn s_pop(&self, key: &str, count: usize) -> Vec<Bytes> {
        if self.check_expiration(key) {
            return Vec::new();
        }
        let mut entry = match self.inner.data.get_mut(key) {
            Some(e) => e,
            None => return Vec::new(),
        };

        match entry.value_mut() {
            StoredValue::Set(set) => {
                let mut result = Vec::with_capacity(count.min(set.len()));
                for _ in 0..count {
                    if let Some(member) = set.iter().next().cloned() {
                        set.remove(&member);
                        result.push(member);
                    } else {
                        break;
                    }
                }
                result
            }
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_add_rem_and_membership() {
        let store = Store::new();

        assert_eq!(
            store.s_add("s".to_string(), vec![Bytes::from("a"), Bytes::from("b")]),
            2
        );
        assert_eq!(
            store.s_add("s".to_string(), vec![Bytes::from("a"), Bytes::from("c")]),
            1
        );

        assert!(store.s_is_member("s", &Bytes::from("a")));
        assert_eq!(store.s_card("s"), 3);

        let removed = store.s_rem("s", &[Bytes::from("a"), Bytes::from("x")]);
        assert_eq!(removed, 1);
        assert!(!store.s_is_member("s", &Bytes::from("a")));
        assert_eq!(store.s_card("s"), 2);
    }

    #[test]
    fn test_s_pop_count_bounds() {
        let store = Store::new();
        store.s_add(
            "s".to_string(),
            vec![Bytes::from("a"), Bytes::from("b"), Bytes::from("c")],
        );

        let popped = store.s_pop("s", 10);
        assert_eq!(popped.len(), 3);
        assert_eq!(store.s_card("s"), 0);
    }
}
