//! AOF (Append Only File) persistence implementation
//!
//! AOF logs every write operation to a file for durability.

use super::{AofFsyncPolicy, PersistenceError, Result};
use crate::metrics::Metrics;
use crate::resp::{RespCodec, Value};
use crate::store::Store;
use bytes::BytesMut;
use std::path::Path;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::sync::mpsc;
use tokio_util::codec::{Decoder, Encoder};
use tracing::{error, info, warn};

/// AOF Writer handles appending commands to the AOF file
pub struct AofWriter {
    file: File,
    fsync_policy: AofFsyncPolicy,
    bytes_written: u64,
}

impl AofWriter {
    /// Create a new AOF writer, opening or creating the AOF file
    pub async fn new(path: &Path, fsync_policy: AofFsyncPolicy) -> Result<Self> {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await?;

        let metadata = file.metadata().await?;
        let bytes_written = metadata.len();

        Ok(Self {
            file,
            fsync_policy,
            bytes_written,
        })
    }

    /// Append a command to the AOF file
    pub async fn append(&mut self, command: &Value) -> Result<()> {
        let mut buf = BytesMut::new();
        let mut codec = RespCodec;

        // Serialize the command to RESP format
        codec
            .encode(command.clone(), &mut buf)
            .map_err(|e| PersistenceError::Serialization(e.to_string()))?;

        // Write to file
        self.file.write_all(&buf).await?;
        self.bytes_written += buf.len() as u64;

        // Handle fsync based on policy
        match self.fsync_policy {
            AofFsyncPolicy::Always => {
                self.file.sync_all().await?;
            }
            AofFsyncPolicy::EverySec => {
                // Fsync will be handled by a background task
            }
            AofFsyncPolicy::No => {
                // Let OS handle it
            }
        }

        Ok(())
    }

    /// Force fsync to disk
    pub async fn fsync(&mut self) -> Result<()> {
        self.file.sync_all().await?;
        Ok(())
    }

    /// Get total bytes written
    pub fn bytes_written(&self) -> u64 {
        self.bytes_written
    }

    /// Truncate and reset the file (used for rewrite)
    pub async fn truncate(&mut self) -> Result<()> {
        self.file.set_len(0).await?;
        self.bytes_written = 0;
        Ok(())
    }
}

/// AOF Replay functionality
pub struct AofReplayer;

impl AofReplayer {
    /// Replay AOF file to restore state
    pub async fn replay(store: &Store, path: &Path) -> Result<u64> {
        Self::replay_with_metrics(store, path, &Metrics::new()).await
    }

    /// Replay AOF file with optional metrics tracking.
    pub async fn replay_with_metrics(
        store: &Store,
        path: &Path,
        metrics: &Metrics,
    ) -> Result<u64> {
        if !path.exists() {
            return Ok(0);
        }

        let file = File::open(path).await?;
        let reader = BufReader::new(file);
        let lines = reader.lines();

        let mut command_count = 0u64;

        // AOF is in RESP format, so we need to parse it properly
        // For simplicity, we'll read the raw bytes and decode RESP
        drop(lines);

        let file = File::open(path).await?;
        let mut reader = BufReader::new(file);
        let mut raw_buffer = Vec::new();
        reader.read_to_end(&mut raw_buffer).await?;

        let mut bytes = BytesMut::from(&raw_buffer[..]);
        let mut codec = RespCodec;

        loop {
            if bytes.is_empty() {
                break;
            }

            match codec.decode(&mut bytes) {
                Ok(Some(value)) => {
                    // Execute the command
                    if let Err(e) = Self::execute_command(store, value, metrics).await {
                        warn!("Failed to execute AOF command: {}", e);
                    }
                    command_count += 1;
                }
                Ok(None) => {
                    // Incomplete data, break
                    break;
                }
                Err(e) => {
                    error!("AOF decode error: {}", e);
                    break;
                }
            }
        }

        info!("AOF replay complete: {} commands executed", command_count);
        Ok(command_count)
    }

    async fn execute_command(store: &Store, value: Value, metrics: &Metrics) -> crate::Result<()> {
        use crate::commands::Command;

        if let Value::Array(Some(args)) = value {
            if let Ok(cmd) = Command::try_from(args) {
                cmd.execute(store, metrics);
            }
        }
        Ok(())
    }
}

/// AOF Rewrite - compact AOF by writing current state
pub async fn rewrite_aof(store: &Store, aof_path: &Path, temp_path: &Path) -> Result<u64> {
    // Create temporary AOF file with current state
    let mut temp_writer = AofWriter::new(temp_path, AofFsyncPolicy::No).await?;
    let mut codec = RespCodec;

    // Get all keys and their values
    let keys = store.keys();
    let mut commands_written = 0u64;

    for key in keys {
        if let Some(value) = store.get_for_restore(&key) {
            let ttl_ms = store.ttl_ms_for_persistence(&key);
            // Generate appropriate command based on value type
            let commands = value_to_commands(&key, &value, ttl_ms);

            for cmd in commands {
                let mut buf = BytesMut::new();
                codec
                    .encode(cmd, &mut buf)
                    .map_err(|e| PersistenceError::Serialization(e.to_string()))?;
                temp_writer.file.write_all(&buf).await?;
                commands_written += 1;
            }
        }
    }

    // Sync temp file
    temp_writer.fsync().await?;
    drop(temp_writer);

    // Atomically replace old AOF with new one
    tokio::fs::rename(temp_path, aof_path).await?;

    info!(
        "AOF rewrite complete: {} commands written",
        commands_written
    );
    Ok(commands_written)
}

/// Convert a StoredValue back to RESP commands
fn value_to_commands(
    key: &str,
    value: &crate::store::StoredValue,
    ttl_ms: Option<u64>,
) -> Vec<Value> {
    use crate::resp::Value;
    use bytes::Bytes;

    let mut commands = Vec::new();

    match value {
        crate::store::StoredValue::String(bytes) => {
            commands.push(Value::Array(Some(vec![
                Value::BulkString(Some(Bytes::from_static(b"SET"))),
                Value::BulkString(Some(Bytes::from(key.to_string()))),
                Value::BulkString(Some(bytes.clone())),
            ])));
        }
        crate::store::StoredValue::List(list) => {
            if !list.is_empty() {
                let mut args = vec![
                    Value::BulkString(Some(Bytes::from_static(b"RPUSH"))),
                    Value::BulkString(Some(Bytes::from(key.to_string()))),
                ];
                for item in list {
                    args.push(Value::BulkString(Some(item.clone())));
                }
                commands.push(Value::Array(Some(args)));
            }
        }
        crate::store::StoredValue::Set(set) => {
            if !set.is_empty() {
                let mut args = vec![
                    Value::BulkString(Some(Bytes::from_static(b"SADD"))),
                    Value::BulkString(Some(Bytes::from(key.to_string()))),
                ];
                for item in set {
                    args.push(Value::BulkString(Some(item.clone())));
                }
                commands.push(Value::Array(Some(args)));
            }
        }
        crate::store::StoredValue::Hash(map) => {
            if !map.is_empty() {
                let mut args = vec![
                    Value::BulkString(Some(Bytes::from_static(b"HSET"))),
                    Value::BulkString(Some(Bytes::from(key.to_string()))),
                ];
                for (field, value) in map {
                    args.push(Value::BulkString(Some(Bytes::from(field.clone()))));
                    args.push(Value::BulkString(Some(value.clone())));
                }
                commands.push(Value::Array(Some(args)));
            }
        }
        crate::store::StoredValue::SortedSet { scores, .. } => {
            if !scores.is_empty() {
                let mut args = vec![
                    Value::BulkString(Some(Bytes::from_static(b"ZADD"))),
                    Value::BulkString(Some(Bytes::from(key.to_string()))),
                ];
                for (member, score) in scores {
                    args.push(Value::BulkString(Some(Bytes::from(score.to_string()))));
                    args.push(Value::BulkString(Some(member.clone())));
                }
                commands.push(Value::Array(Some(args)));
            }
        }
    }

    // Preserve TTL by appending an EXPIRE command. Use a minimum of 1 second
    // because our EXPIRE command takes seconds and we do not yet support PEXPIRE.
    if let Some(ms) = ttl_ms {
        let seconds = (ms.div_ceil(1000)).max(1);
        commands.push(Value::Array(Some(vec![
            Value::BulkString(Some(Bytes::from_static(b"EXPIRE"))),
            Value::BulkString(Some(Bytes::from(key.to_string()))),
            Value::BulkString(Some(Bytes::from(seconds.to_string()))),
        ])));
    }

    commands
}

/// AOF Channel for sending commands to be logged
#[derive(Clone)]
pub struct AofChannel {
    sender: mpsc::UnboundedSender<Value>,
}

impl AofChannel {
    pub fn new(sender: mpsc::UnboundedSender<Value>) -> Self {
        Self { sender }
    }

    pub fn log(&self, command: Value) {
        let _ = self.sender.send(command);
    }
}

/// Check if a command should be logged to AOF
pub fn should_log_to_aof(command_name: &str) -> bool {
    // Don't log read-only commands
    let read_commands = [
        "GET",
        "MGET",
        "EXISTS",
        "TTL",
        "PTTL",
        "TYPE",
        "KEYS",
        "SCAN",
        "LRANGE",
        "LLEN",
        "LINDEX",
        "SMEMBERS",
        "SCARD",
        "SISMEMBER",
        "SRANDMEMBER",
        "HGET",
        "HMGET",
        "HGETALL",
        "HKEYS",
        "HVALS",
        "HLEN",
        "HEXISTS",
        "ZRANGE",
        "ZREVRANGE",
        "ZRANGEBYSCORE",
        "ZREVRANGEBYSCORE",
        "ZCARD",
        "ZSCORE",
        "ZRANK",
        "ZREVRANK",
        "ZCOUNT",
        "PING",
        "ECHO",
        "INFO",
        "QUIT",
        "SELECT",
        "DBSIZE",
    ];

    !read_commands.contains(&command_name.to_ascii_uppercase().as_str())
}

/// Format a command and its arguments as a RESP array for AOF logging
pub fn format_command_for_aof(cmd: &str, args: &[String]) -> Value {
    use crate::resp::Value;
    use bytes::Bytes;

    let mut array = vec![Value::BulkString(Some(Bytes::from(cmd.to_string())))];
    for arg in args {
        array.push(Value::BulkString(Some(Bytes::from(arg.clone()))));
    }

    Value::Array(Some(array))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;

    #[tokio::test]
    async fn test_aof_append() {
        let temp_path = std::env::temp_dir().join("test.aof");

        let mut writer = AofWriter::new(&temp_path, AofFsyncPolicy::No)
            .await
            .unwrap();

        let cmd = Value::Array(Some(vec![
            Value::BulkString(Some(Bytes::from_static(b"SET"))),
            Value::BulkString(Some(Bytes::from_static(b"key1"))),
            Value::BulkString(Some(Bytes::from_static(b"value1"))),
        ]));

        writer.append(&cmd).await.unwrap();
        writer.fsync().await.unwrap();

        let content = tokio::fs::read_to_string(&temp_path).await.unwrap();
        assert!(content.contains("SET"));
        assert!(content.contains("key1"));
        assert!(content.contains("value1"));

        // Cleanup
        let _ = tokio::fs::remove_file(&temp_path).await;
    }

    #[test]
    fn test_should_log_to_aof() {
        assert!(!should_log_to_aof("GET"));
        assert!(!should_log_to_aof("EXISTS"));
        assert!(should_log_to_aof("SET"));
        assert!(should_log_to_aof("DEL"));
    }
}
