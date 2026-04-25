//! Persistence module for RDB and AOF
//!
//! RDB (Redis Database): Point-in-time snapshots
//! AOF (Append Only File): Log of write operations

pub mod aof;
pub mod rdb;
pub mod manager;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub use manager::PersistenceManager;

/// Persistence configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistenceConfig {
    /// Enable RDB persistence
    pub rdb_enabled: bool,
    /// RDB file path
    pub rdb_file: PathBuf,
    /// RDB save conditions: (changes, seconds)
    pub rdb_save_conditions: Vec<(u32, u32)>,

    /// Enable AOF persistence
    pub aof_enabled: bool,
    /// AOF file path
    pub aof_file: PathBuf,
    /// AOF fsync policy: always, everysec, no
    pub aof_fsync: AofFsyncPolicy,
    /// Rewrite AOF when growth exceeds percentage
    pub aof_rewrite_percentage: usize,
    /// Minimum size to trigger AOF rewrite (MB)
    pub aof_rewrite_min_size: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AofFsyncPolicy {
    /// Fsync after every write (safest, slowest)
    Always,
    /// Fsync every second (default, good balance)
    EverySec,
    /// Let OS handle fsync (fastest, least safe)
    No,
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self {
            rdb_enabled: true,
            rdb_file: PathBuf::from("dump.rdb"),
            rdb_save_conditions: vec![(1, 900), (10, 300), (1000, 60)], // Default Redis conditions
            aof_enabled: false,
            aof_file: PathBuf::from("appendonly.aof"),
            aof_fsync: AofFsyncPolicy::EverySec,
            aof_rewrite_percentage: 100,
            aof_rewrite_min_size: 64, // MB
        }
    }
}

impl PersistenceConfig {
    /// Create config with both RDB and AOF enabled
    pub fn with_aof() -> Self {
        Self {
            aof_enabled: true,
            ..Default::default()
        }
    }

    /// Disable all persistence
    pub fn disabled() -> Self {
        Self {
            rdb_enabled: false,
            aof_enabled: false,
            ..Default::default()
        }
    }
}

/// Persistence statistics
#[derive(Debug, Default)]
pub struct PersistenceStats {
    /// Last RDB save time (unix timestamp)
    pub rdb_last_save_time: Option<u64>,
    /// Last RDB save status
    pub rdb_last_save_status: Option<String>,
    /// RDB file size in bytes
    pub rdb_file_size: Option<u64>,
    /// AOF file size in bytes
    pub aof_file_size: Option<u64>,
    /// AOF rewrite in progress
    pub aof_rewrite_in_progress: bool,
    /// Total commands processed for AOF
    pub aof_total_commands: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum PersistenceError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Deserialization error: {0}")]
    Deserialization(String),

    #[error("Invalid file format: {0}")]
    InvalidFormat(String),
}

pub type Result<T> = std::result::Result<T, PersistenceError>;
