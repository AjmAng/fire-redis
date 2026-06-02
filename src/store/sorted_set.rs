use crate::store::{StoredValue, Store};
use bytes::Bytes;
use ordered_float::OrderedFloat;
use std::collections::{BTreeMap, HashMap, HashSet};

impl Store {
    pub fn z_add(&self, key: String, score: f64, member: Bytes) -> bool {
        self.check_expiration(&key);

        let mut entry = self.inner.data.entry(key).or_insert_with(|| {
            StoredValue::SortedSet {
                scores: HashMap::new(),
                tree: BTreeMap::new(),
            }
        });

        match entry.value_mut() {
            StoredValue::SortedSet { scores, tree } => {
                let ordered_score = OrderedFloat(score);
                let is_new_member = !scores.contains_key(&member);

                if let Some(old_score) = scores.remove(&member) {
                    if let Some(members) = tree.get_mut(&old_score.into()) {
                        members.remove(&member);
                        if members.is_empty() {
                            tree.remove(&old_score.into());
                        }
                    }
                }

                scores.insert(member.clone(), ordered_score.into());
                tree.entry(ordered_score)
                    .or_insert_with(HashSet::new)
                    .insert(member);
                is_new_member
            }
            _ => false,
        }
    }

    pub fn z_range(&self, key: &str, start: i64, stop: i64) -> Vec<(Bytes, f64)> {
        if self.check_expiration(key) {
            return Vec::new();
        }
        self.inner.data.get(key).map_or_else(Vec::new, |entry| {
            match entry.value() {
                StoredValue::SortedSet { tree, .. } => {
                    let all_members: Vec<_> = tree
                        .iter()
                        .flat_map(|(score, members)| {
                            members.iter().map(move |m| (m.clone(), score.0))
                        })
                        .collect();

                    let len = all_members.len() as i64;
                    let start = if start < 0 { len + start } else { start }.max(0) as usize;
                    let stop = if stop < 0 { len + stop } else { stop }.min(len - 1) as usize;

                    if start > stop || start >= all_members.len() {
                        return Vec::new();
                    }
                    all_members[start..=stop.min(all_members.len() - 1)].to_vec()
                }
                _ => Vec::new(),
            }
        })
    }

    pub fn z_rev_range(&self, key: &str, start: i64, stop: i64) -> Vec<(Bytes, f64)> {
        if self.check_expiration(key) {
            return Vec::new();
        }
        self.inner.data.get(key).map_or_else(Vec::new, |entry| {
            match entry.value() {
                StoredValue::SortedSet { tree, .. } => {
                    let all_members: Vec<_> = tree
                        .iter()
                        .rev()
                        .flat_map(|(score, members)| {
                            members.iter().map(move |m| (m.clone(), score.0))
                        })
                        .collect();

                    let len = all_members.len() as i64;
                    let start = if start < 0 { len + start } else { start }.max(0) as usize;
                    let stop = if stop < 0 { len + stop } else { stop }.min(len - 1) as usize;

                    if start > stop || start >= all_members.len() {
                        return Vec::new();
                    }
                    all_members[start..=stop.min(all_members.len() - 1)].to_vec()
                }
                _ => Vec::new(),
            }
        })
    }

    pub fn z_score(&self, key: &str, member: &Bytes) -> Option<f64> {
        if self.check_expiration(key) {
            return None;
        }
        self.inner.data.get(key).and_then(|entry| {
            match entry.value() {
                StoredValue::SortedSet { scores, .. } => scores.get(member).map(|s| s.0),
                _ => None,
            }
        })
    }

    pub fn z_rem(&self, key: &str, members: &[Bytes]) -> usize {
        if self.check_expiration(key) {
            return 0;
        }
        let mut entry = match self.inner.data.get_mut(key) {
            Some(e) => e,
            None => return 0,
        };

        match entry.value_mut() {
            StoredValue::SortedSet { scores, tree } => {
                let mut removed = 0;
                for member in members {
                    if let Some(score) = scores.remove(member) {
                        if let Some(set) = tree.get_mut(&score.into()) {
                            set.remove(member);
                            if set.is_empty() {
                                tree.remove(&score.into());
                            }
                        }
                        removed += 1;
                    }
                }
                let should_remove = scores.is_empty();
                drop(entry);

                if should_remove {
                    self.inner.data.remove(key);
                    self.inner.expirations.remove(key);
                }

                removed
            }
            _ => 0,
        }
    }

    pub fn z_card(&self, key: &str) -> usize {
        if self.check_expiration(key) {
            return 0;
        }
        self.inner.data.get(key).map_or(0, |entry| {
            match entry.value() {
                StoredValue::SortedSet { scores, .. } => scores.len(),
                _ => 0,
            }
        })
    }

    pub fn z_count(&self, key: &str, min_score: f64, max_score: f64) -> usize {
        if self.check_expiration(key) {
            return 0;
        }
        self.inner.data.get(key).map_or(0, |entry| {
            match entry.value() {
                StoredValue::SortedSet { tree, .. } => {
                    tree.range(OrderedFloat(min_score)..=OrderedFloat(max_score))
                        .map(|(_, members)| members.len())
                        .sum()
                }
                _ => 0,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sorted_set_order_and_score_queries() {
        let store = Store::new();

        assert!(store.z_add("z".to_string(), 2.0, Bytes::from("b")));
        assert!(store.z_add("z".to_string(), 1.0, Bytes::from("a")));
        assert!(store.z_add("z".to_string(), 3.0, Bytes::from("c")));

        let range = store.z_range("z", 0, -1);
        assert_eq!(range.len(), 3);
        assert_eq!(range[0].0, Bytes::from("a"));
        assert_eq!(range[1].0, Bytes::from("b"));
        assert_eq!(range[2].0, Bytes::from("c"));

        assert_eq!(store.z_score("z", &Bytes::from("b")), Some(2.0));
        assert_eq!(store.z_count("z", 1.5, 3.0), 2);
    }

    #[test]
    fn test_sorted_set_update_and_remove() {
        let store = Store::new();
        store.z_add("z".to_string(), 1.0, Bytes::from("a"));
        store.z_add("z".to_string(), 2.0, Bytes::from("b"));

        // Updating score should keep cardinality stable.
        assert!(store.z_add("z".to_string(), 3.0, Bytes::from("a")));
        assert_eq!(store.z_card("z"), 2);
        assert_eq!(store.z_score("z", &Bytes::from("a")), Some(3.0));

        assert_eq!(store.z_rem("z", &[Bytes::from("b")]), 1);
        assert_eq!(store.z_card("z"), 1);
    }

    #[test]
    fn test_sorted_set_update_does_not_duplicate_in_range() {
        let store = Store::new();

        assert!(store.z_add("z".to_string(), 1.0, Bytes::from("a")));
        assert!(!store.z_add("z".to_string(), 3.0, Bytes::from("a")));

        let range = store.z_range("z", 0, -1);
        assert_eq!(range, vec![(Bytes::from("a"), 3.0)]);
    }

    #[test]
    fn test_zrem_removes_empty_sorted_set_key() {
        let store = Store::new();
        store.z_add("z".to_string(), 1.0, Bytes::from("a"));

        assert_eq!(store.z_rem("z", &[Bytes::from("a")]), 1);
        assert!(!store.exists("z"));
    }
}
