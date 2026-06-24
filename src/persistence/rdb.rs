//! RDB (Redis Database) persistence implementation
//!
//! RDB creates point-in-time snapshots of the dataset.

use super::{PersistenceError, Result};
use crate::store::{Store, StoredValue};
use bytes::Bytes;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::Path;
use std::time::Instant;
use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// RDB file magic number: "REDIS"
const RDB_MAGIC: &[u8] = b"REDIS";
/// RDB version (0011 = version 11)
const RDB_VERSION: &[u8] = b"0011";
/// RDB opcode markers
const OP_EOF: u8 = 0xFF;
const OP_SELECT_DB: u8 = 0xFE;
const OP_EXPIRE_TIME_MS: u8 = 0xFC;
const OP_EXPIRE_TIME: u8 = 0xFD;
const OP_RESIZE_DB: u8 = 0xFB;
const OP_AUX: u8 = 0xFA;

/// Value type markers
const TYPE_STRING: u8 = 0;
const TYPE_LIST: u8 = 1;
const TYPE_SET: u8 = 2;
const TYPE_ZSET: u8 = 3;
const TYPE_HASH: u8 = 4;

/// RDB serializer
pub struct RdbSerializer;

impl RdbSerializer {
    /// Serialize the entire store to a file
    pub async fn save_to_file(store: &Store, path: &Path) -> Result<()> {
        let temp_path = path.with_extension("tmp");

        // Write to temp file first
        let mut file = File::create(&temp_path).await?;

        // Write header
        file.write_all(RDB_MAGIC).await?;
        file.write_all(RDB_VERSION).await?;

        // Write AUX fields (metadata)
        Self::write_aux(&mut file, "redis-ver", "7.0.0").await?;
        Self::write_aux(&mut file, "redis-bits", "64").await?;
        Self::write_aux(&mut file, "ctime", &Self::current_time().to_string()).await?;

        // Write database selector (db 0)
        file.write_all(&[OP_SELECT_DB]).await?;
        Self::write_length(&mut file, 0).await?;

        // Get data snapshot
        let (data, expirations) = store.snapshot().await;

        // Write resize info
        file.write_all(&[OP_RESIZE_DB]).await?;
        Self::write_length(&mut file, data.len() as u64).await?;
        Self::write_length(&mut file, expirations.len() as u64).await?;

        // Write key-value pairs
        for (key, value) in &data {
            // Check if key has expiration
            if let Some(expire_at) = expirations.get(key) {
                let now = Instant::now();
                if *expire_at <= now {
                    // Key expired, skip
                    continue;
                }
                let ttl_ms = expire_at.duration_since(now).as_millis() as u64;
                let expire_at_ms = Self::current_time_ms() + ttl_ms;
                file.write_all(&[OP_EXPIRE_TIME_MS]).await?;
                Self::write_u64_ms(&mut file, expire_at_ms).await?;
            }

            // Write value type
            Self::write_value_type(&mut file, value).await?;

            // Write key
            Self::write_string(&mut file, key).await?;

            // Write value
            Self::write_value(&mut file, value).await?;
        }

        // Write EOF marker
        file.write_all(&[OP_EOF]).await?;

        // Write checksum (simplified - just write zeros for now)
        file.write_all(&[0u8; 8]).await?;

        file.flush().await?;
        drop(file);

        // Atomic rename
        tokio::fs::rename(&temp_path, path).await?;

        Ok(())
    }

    async fn write_aux(file: &mut File, key: &str, value: &str) -> Result<()> {
        file.write_all(&[OP_AUX]).await?;
        Self::write_string(file, key).await?;
        Self::write_string(file, value).await?;
        Ok(())
    }

    async fn write_value_type(file: &mut File, value: &StoredValue) -> Result<()> {
        let type_byte = match value {
            StoredValue::String(_) => TYPE_STRING,
            StoredValue::List(_) => TYPE_LIST,
            StoredValue::Set(_) => TYPE_SET,
            StoredValue::Hash(_) => TYPE_HASH,
            StoredValue::SortedSet { .. } => TYPE_ZSET,
        };
        file.write_all(&[type_byte]).await?;
        Ok(())
    }

    async fn write_value(file: &mut File, value: &StoredValue) -> Result<()> {
        match value {
            StoredValue::String(s) => {
                Self::write_bytes(file, s).await?;
            }
            StoredValue::List(list) => {
                Self::write_length(file, list.len() as u64).await?;
                for item in list {
                    Self::write_bytes(file, item).await?;
                }
            }
            StoredValue::Set(set) => {
                Self::write_length(file, set.len() as u64).await?;
                for item in set {
                    Self::write_bytes(file, item).await?;
                }
            }
            StoredValue::Hash(map) => {
                Self::write_length(file, map.len() as u64).await?;
                for (k, v) in map {
                    Self::write_string(file, k).await?;
                    Self::write_bytes(file, v).await?;
                }
            }
            StoredValue::SortedSet { scores, .. } => {
                Self::write_length(file, scores.len() as u64).await?;
                for (member, score) in scores {
                    Self::write_double(file, score.0).await?;
                    Self::write_bytes(file, member).await?;
                }
            }
        }
        Ok(())
    }

    async fn write_length(file: &mut File, len: u64) -> Result<()> {
        if len < 64 {
            // 6 bits: 00xxxxxx
            file.write_all(&[len as u8]).await?;
        } else if len < 16384 {
            // 14 bits: 01xxxxxx xxxxxxxx
            let bytes = ((1 << 14) | len).to_be_bytes();
            file.write_all(&[bytes[6], bytes[7]]).await?;
        } else {
            // 64 bits: 10xxxxxx + 8 bytes
            file.write_all(&[0x80]).await?;
            file.write_all(&len.to_be_bytes()).await?;
        }
        Ok(())
    }

    async fn write_string(file: &mut File, s: &str) -> Result<()> {
        Self::write_length(file, s.len() as u64).await?;
        file.write_all(s.as_bytes()).await?;
        Ok(())
    }

    async fn write_bytes(file: &mut File, b: &Bytes) -> Result<()> {
        Self::write_length(file, b.len() as u64).await?;
        file.write_all(b).await?;
        Ok(())
    }

    async fn write_u64_ms(file: &mut File, val: u64) -> Result<()> {
        file.write_all(&val.to_le_bytes()).await?;
        Ok(())
    }

    async fn write_double(file: &mut File, val: f64) -> Result<()> {
        // Write as string for simplicity
        let s = val.to_string();
        Self::write_string(file, &s).await?;
        Ok(())
    }

    fn current_time() -> u64 {
        Self::current_time_ms() / 1000
    }

    fn current_time_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }
}

/// RDB deserializer
pub struct RdbDeserializer;

impl RdbDeserializer {
    /// Load data from RDB file into store
    pub async fn load_from_file(store: &Store, path: &Path) -> Result<()> {
        if !path.exists() {
            return Ok(()); // No RDB file yet, start fresh
        }

        let mut file = File::open(path).await?;
        let mut buf = Vec::new();
        file.read_to_end(&mut buf).await?;

        let mut pos = 0;

        // Check magic
        if buf.len() < 9 || &buf[0..5] != RDB_MAGIC {
            return Err(PersistenceError::InvalidFormat(
                "Invalid RDB magic number".to_string(),
            ));
        }
        pos += 9; // Skip magic (5) + version (4)

        let current_time = Self::current_time_ms();

        loop {
            if pos >= buf.len() {
                break;
            }

            let opcode = buf[pos];
            pos += 1;

            match opcode {
                OP_EOF => {
                    // End of file, skip checksum
                    break;
                }
                OP_SELECT_DB => {
                    // Read db number and skip
                    let (_, new_pos) = Self::read_length(&buf, pos)?;
                    pos = new_pos;
                }
                OP_RESIZE_DB => {
                    // Read db size and expires size, skip
                    let (_, new_pos) = Self::read_length(&buf, pos)?;
                    pos = new_pos;
                    let (_, new_pos) = Self::read_length(&buf, pos)?;
                    pos = new_pos;
                }
                OP_AUX => {
                    // Skip auxiliary fields
                    let (_, new_pos) = Self::read_string_bytes(&buf, pos)?;
                    pos = new_pos;
                    let (_, new_pos) = Self::read_string_bytes(&buf, pos)?;
                    pos = new_pos;
                }
                OP_EXPIRE_TIME_MS => {
                    // Read expiration time in ms
                    if pos + 8 > buf.len() {
                        return Err(PersistenceError::InvalidFormat(
                            "Truncated expire time".to_string(),
                        ));
                    }
                    let expire_ms = u64::from_le_bytes([
                        buf[pos],
                        buf[pos + 1],
                        buf[pos + 2],
                        buf[pos + 3],
                        buf[pos + 4],
                        buf[pos + 5],
                        buf[pos + 6],
                        buf[pos + 7],
                    ]);
                    pos += 8;

                    // Read value type
                    let value_type = buf[pos];
                    pos += 1;

                    // Read key
                    let (key_bytes, new_pos) = Self::read_string_bytes(&buf, pos)?;
                    pos = new_pos;
                    let key = String::from_utf8_lossy(&key_bytes).to_string();

                    // Read value
                    let (value, new_pos) = Self::read_value(&buf, pos, value_type)?;
                    pos = new_pos;

                    // Check if expired
                    if expire_ms > current_time {
                        let ttl_ms = expire_ms - current_time;
                        store.set_with_ttl(&key, value, ttl_ms);
                    }
                }
                OP_EXPIRE_TIME => {
                    // Read expiration time in seconds (legacy)
                    if pos + 4 > buf.len() {
                        return Err(PersistenceError::InvalidFormat(
                            "Truncated expire time".to_string(),
                        ));
                    }
                    let expire_s =
                        u32::from_le_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]])
                            as u64;
                    pos += 4;

                    let value_type = buf[pos];
                    pos += 1;

                    let (key_bytes, new_pos) = Self::read_string_bytes(&buf, pos)?;
                    pos = new_pos;
                    let key = String::from_utf8_lossy(&key_bytes).to_string();

                    let (value, new_pos) = Self::read_value(&buf, pos, value_type)?;
                    pos = new_pos;

                    let expire_ms = expire_s * 1000;
                    if expire_ms > current_time {
                        let ttl_ms = expire_ms - current_time;
                        store.set_with_ttl(&key, value, ttl_ms);
                    }
                }
                _ => {
                    // Value type (0-4)
                    let value_type = opcode;

                    // Read key
                    let (key_bytes, new_pos) = Self::read_string_bytes(&buf, pos)?;
                    pos = new_pos;
                    let key = String::from_utf8_lossy(&key_bytes).to_string();

                    // Read value
                    let (value, new_pos) = Self::read_value(&buf, pos, value_type)?;
                    pos = new_pos;

                    store.restore(&key, value);
                }
            }
        }

        Ok(())
    }

    fn read_length(buf: &[u8], pos: usize) -> Result<(u64, usize)> {
        if pos >= buf.len() {
            return Err(PersistenceError::InvalidFormat(
                "Unexpected end of file".to_string(),
            ));
        }

        let first = buf[pos];
        let enc_type = (first & 0xC0) >> 6;

        match enc_type {
            0 => {
                // 6 bits
                Ok((first as u64 & 0x3F, pos + 1))
            }
            1 => {
                // 14 bits
                if pos + 1 >= buf.len() {
                    return Err(PersistenceError::InvalidFormat(
                        "Truncated length".to_string(),
                    ));
                }
                let val = ((first as u64 & 0x3F) << 8) | (buf[pos + 1] as u64);
                Ok((val, pos + 2))
            }
            2 => {
                // 64 bits
                if pos + 8 >= buf.len() {
                    return Err(PersistenceError::InvalidFormat(
                        "Truncated length".to_string(),
                    ));
                }
                let val = u64::from_be_bytes([
                    buf[pos + 1],
                    buf[pos + 2],
                    buf[pos + 3],
                    buf[pos + 4],
                    buf[pos + 5],
                    buf[pos + 6],
                    buf[pos + 7],
                    buf[pos + 8],
                ]);
                Ok((val, pos + 9))
            }
            _ => Err(PersistenceError::InvalidFormat(
                "Invalid length encoding".to_string(),
            )),
        }
    }

    fn read_string_bytes(buf: &[u8], pos: usize) -> Result<(Vec<u8>, usize)> {
        let (len, pos) = Self::read_length(buf, pos)?;
        let len = len as usize;

        if pos + len > buf.len() {
            return Err(PersistenceError::InvalidFormat(
                "Truncated string".to_string(),
            ));
        }

        Ok((buf[pos..pos + len].to_vec(), pos + len))
    }

    fn read_value(buf: &[u8], pos: usize, value_type: u8) -> Result<(StoredValue, usize)> {
        match value_type {
            TYPE_STRING => {
                let (bytes, new_pos) = Self::read_string_bytes(buf, pos)?;
                Ok((StoredValue::String(Bytes::from(bytes)), new_pos))
            }
            TYPE_LIST => {
                let (len, mut pos) = Self::read_length(buf, pos)?;
                let mut list = VecDeque::with_capacity(len as usize);

                for _ in 0..len {
                    let (bytes, new_pos) = Self::read_string_bytes(buf, pos)?;
                    list.push_back(Bytes::from(bytes));
                    pos = new_pos;
                }

                Ok((StoredValue::List(list), pos))
            }
            TYPE_SET => {
                let (len, mut pos) = Self::read_length(buf, pos)?;
                let mut set = HashSet::with_capacity(len as usize);

                for _ in 0..len {
                    let (bytes, new_pos) = Self::read_string_bytes(buf, pos)?;
                    set.insert(Bytes::from(bytes));
                    pos = new_pos;
                }

                Ok((StoredValue::Set(set), pos))
            }
            TYPE_HASH => {
                let (len, mut pos) = Self::read_length(buf, pos)?;
                let mut map = HashMap::with_capacity(len as usize);

                for _ in 0..len {
                    let (k_bytes, new_pos) = Self::read_string_bytes(buf, pos)?;
                    pos = new_pos;
                    let (v_bytes, new_pos) = Self::read_string_bytes(buf, pos)?;
                    pos = new_pos;

                    map.insert(
                        String::from_utf8_lossy(&k_bytes).to_string(),
                        Bytes::from(v_bytes),
                    );
                }

                Ok((StoredValue::Hash(map), pos))
            }
            TYPE_ZSET => {
                let (len, mut pos) = Self::read_length(buf, pos)?;
                let mut scores = HashMap::with_capacity(len as usize);
                let mut tree: BTreeMap<ordered_float::OrderedFloat<f64>, HashSet<Bytes>> =
                    BTreeMap::new();

                for _ in 0..len {
                    let (score_bytes, new_pos) = Self::read_string_bytes(buf, pos)?;
                    pos = new_pos;
                    let score_str = String::from_utf8_lossy(&score_bytes);
                    let score: f64 = score_str.parse().map_err(|_| {
                        PersistenceError::InvalidFormat("Invalid score".to_string())
                    })?;
                    let ordered_score = ordered_float::OrderedFloat(score);

                    let (member_bytes, new_pos) = Self::read_string_bytes(buf, pos)?;
                    pos = new_pos;
                    let member = Bytes::from(member_bytes);

                    scores.insert(member.clone(), ordered_score);
                    tree.entry(ordered_score).or_default().insert(member);
                }

                Ok((StoredValue::SortedSet { scores, tree }, pos))
            }
            _ => Err(PersistenceError::InvalidFormat(format!(
                "Unknown value type: {}",
                value_type
            ))),
        }
    }

    fn current_time_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }
}

/// Trigger a background save
pub async fn bgsave(store: &Store, path: &Path) -> Result<()> {
    RdbSerializer::save_to_file(store, path).await
}

/// Load from RDB file
pub async fn load(store: &Store, path: &Path) -> Result<()> {
    RdbDeserializer::load_from_file(store, path).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rdb_save_load() {
        let store = Store::new();
        store.set("key1".to_string(), Bytes::from("value1"), None);
        store.set("key2".to_string(), Bytes::from("value2"), Some(10000));
        // add more type  hash/zset/list
        store.h_set(
            "hash1".to_string(),
            "field1".to_string(),
            Bytes::from("h_value1"),
        );
        store.z_add("zset1".to_string(), 1.0, Bytes::from("member1".to_string()));
        store.l_push("list1".to_string(), Bytes::from("l_value1"));

        let temp_path = std::env::temp_dir().join("test.rdb");

        // Save
        RdbSerializer::save_to_file(&store, &temp_path)
            .await
            .unwrap();

        // Load into new store
        let new_store = Store::new();
        RdbDeserializer::load_from_file(&new_store, &temp_path)
            .await
            .unwrap();

        // Verify
        assert_eq!(new_store.get("key1"), Some(Bytes::from("value1")));
        assert_eq!(new_store.get("key2"), Some(Bytes::from("value2")));
        let key2_pttl = new_store.pttl("key2");
        assert!(
            (5000..=10000).contains(&key2_pttl),
            "expected key2 pttl around 10000 ms, got {}",
            key2_pttl
        );
        assert_eq!(
            new_store.h_get("hash1", "field1"),
            Some(Bytes::from("h_value1"))
        );
        assert_eq!(
            new_store.z_score("zset1", &Bytes::from("member1")),
            Some(1.0)
        );
        assert_eq!(new_store.l_pop("list1"), Some(Bytes::from("l_value1")));

        // Cleanup
        let _ = tokio::fs::remove_file(&temp_path).await;
    }
}
