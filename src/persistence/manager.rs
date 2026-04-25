//! Persistence Manager
//!
//! Handles background RDB saves and AOF fsync operations.

use super::aof::{AofChannel, AofWriter};
use super::rdb;
use super::{AofFsyncPolicy, PersistenceConfig, PersistenceStats};
use crate::resp::Value;
use crate::store::Store;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, RwLock};
use tokio::time::interval;
use tracing::{error, info};

/// Inner state of the persistence manager (shared across clones)
struct PersistenceManagerInner {
    config: PersistenceConfig,
    store: Store,
    stats: RwLock<PersistenceStats>,
    dirty_counter: AtomicU64,
    last_save: RwLock<Instant>,
    rdb_file: PathBuf,
    aof_writer: RwLock<Option<AofWriter>>,
    aof_file: PathBuf,
}

/// Persistence manager handles all background persistence tasks
#[derive(Clone)]
pub struct PersistenceManager {
    inner: Arc<PersistenceManagerInner>,
    aof_channel: Option<AofChannel>,
}

impl PersistenceManager {
    /// Create a new persistence manager
    pub async fn new(config: PersistenceConfig, store: Store) -> crate::Result<Self> {
        let rdb_file = config.rdb_file.clone();
        let aof_file = config.aof_file.clone();
        
        // Initialize AOF writer if enabled
        let aof_writer = if config.aof_enabled {
            let writer = AofWriter::new(&aof_file, config.aof_fsync).await
                .map_err(|e| crate::RedisError::Storage(e.to_string()))?;
            Some(writer)
        } else {
            None
        };
        
        // Create AOF channel
        let (aof_tx, aof_rx) = mpsc::unbounded_channel();
        let aof_channel = config.aof_enabled.then(|| AofChannel::new(aof_tx));
        
        let inner = Arc::new(PersistenceManagerInner {
            config,
            store,
            stats: RwLock::new(PersistenceStats::default()),
            dirty_counter: AtomicU64::new(0),
            last_save: RwLock::new(Instant::now()),
            rdb_file,
            aof_writer: RwLock::new(aof_writer),
            aof_file,
        });
        
        let manager = Self {
            inner,
            aof_channel,
        };
        
        // Start AOF processing task if enabled
        if manager.inner.config.aof_enabled {
            manager.clone().start_aof_processor(aof_rx);
        }
        
        Ok(manager)
    }
    
    /// Get the AOF channel for logging commands
    pub fn aof_channel(&self) -> Option<AofChannel> {
        self.aof_channel.clone()
    }
    
    /// Record a write operation (increments dirty counter)
    pub fn record_write(&self) {
        self.inner.dirty_counter.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Get current dirty counter value
    pub fn dirty_count(&self) -> u64 {
        self.inner.dirty_counter.load(Ordering::Relaxed)
    }
    
    /// Get persistence stats
    pub async fn stats(&self) -> PersistenceStats {
        self.inner.stats.read().await.clone()
    }
    
    /// Perform RDB save
    pub async fn save(&self) -> crate::Result<()> {
        info!("Starting RDB save...");
        
        let start = Instant::now();
        
        match rdb::bgsave(&self.inner.store, &self.inner.rdb_file).await {
            Ok(()) => {
                let elapsed = start.elapsed();
                let file_size = tokio::fs::metadata(&self.inner.rdb_file).await
                    .map(|m| m.len())
                    .ok();
                
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                
                {
                    let mut stats = self.inner.stats.write().await;
                    stats.rdb_last_save_time = Some(now);
                    stats.rdb_last_save_status = Some(format!("OK ({} ms)", elapsed.as_millis()));
                    stats.rdb_file_size = file_size;
                }
                
                // Reset dirty counter and last save time
                self.inner.dirty_counter.store(0, Ordering::Relaxed);
                *self.inner.last_save.write().await = Instant::now();
                
                info!("RDB save completed in {:?}", elapsed);
                Ok(())
            }
            Err(e) => {
                let mut stats = self.inner.stats.write().await;
                stats.rdb_last_save_status = Some(format!("ERR: {}", e));
                
                error!("RDB save failed: {}", e);
                Err(crate::RedisError::Storage(e.to_string()))
            }
        }
    }
    
    /// Start background persistence tasks
    pub async fn start_background_tasks(&self) {
        if self.inner.config.rdb_enabled && !self.inner.config.rdb_save_conditions.is_empty() {
            self.clone().start_rdb_autosave();
        }
        
        if self.inner.config.aof_enabled && self.inner.config.aof_fsync == AofFsyncPolicy::EverySec {
            self.clone().start_aof_fsync();
        }
    }
    
    /// Start RDB autosave background task
    fn start_rdb_autosave(self) {
        let conditions = self.inner.config.rdb_save_conditions.clone();
        
        tokio::spawn(async move {
            let mut check_interval = interval(Duration::from_secs(1));
            
            loop {
                check_interval.tick().await;
                
                let dirty_count = self.inner.dirty_counter.load(Ordering::Relaxed);
                let last_save_time = *self.inner.last_save.read().await;
                let elapsed = last_save_time.elapsed().as_secs() as u32;
                
                // Check each save condition
                for (changes, seconds) in &conditions {
                    if dirty_count >= *changes as u64 && elapsed >= *seconds {
                        if let Err(e) = self.save().await {
                            error!("Background RDB save failed: {}", e);
                        }
                        break;
                    }
                }
            }
        });
    }
    
    /// Start AOF fsync background task (for every sec policy)
    fn start_aof_fsync(self) {
        tokio::spawn(async move {
            let mut fsync_interval = interval(Duration::from_secs(1));
            
            loop {
                fsync_interval.tick().await;
                
                if let Some(ref mut w) = *self.inner.aof_writer.write().await {
                    if let Err(e) = w.fsync().await {
                        error!("AOF fsync failed: {}", e);
                    }
                }
            }
        });
    }
    
    /// Start AOF command processing task
    fn start_aof_processor(self, mut rx: mpsc::UnboundedReceiver<Value>) {
        tokio::spawn(async move {
            while let Some(command) = rx.recv().await {
                if let Some(ref mut w) = *self.inner.aof_writer.write().await {
                    if let Err(e) = w.append(&command).await {
                        error!("AOF append failed: {}", e);
                    } else {
                        self.inner.stats.write().await.aof_total_commands += 1;
                    }
                }
            }
        });
    }
    
    /// Load data from persistence files on startup
    pub async fn load_on_startup(&self) -> crate::Result<()> {
        // Load RDB first (older data)
        if self.inner.config.rdb_enabled && self.inner.rdb_file.exists() {
            info!("Loading RDB file: {:?}", self.inner.rdb_file);
            rdb::load(&self.inner.store, &self.inner.rdb_file).await
                .map_err(|e| crate::RedisError::Storage(e.to_string()))?;
            info!("RDB load complete");
        }
        
        // Then replay AOF (newer data)
        if self.inner.config.aof_enabled && self.inner.aof_file.exists() {
            info!("Replaying AOF file: {:?}", self.inner.aof_file);
            let count = super::aof::AofReplayer::replay(&self.inner.store, &self.inner.aof_file).await
                .map_err(|e| crate::RedisError::Storage(e.to_string()))?;
            info!("AOF replay complete: {} commands", count);
        }
        
        Ok(())
    }
    
    /// Trigger AOF rewrite
    pub async fn rewrite_aof(&self) -> crate::Result<u64> {
        let temp_path = self.inner.aof_file.with_extension("aof.tmp");
        
        info!("Starting AOF rewrite...");
        
        let count = super::aof::rewrite_aof(&self.inner.store, &self.inner.aof_file, &temp_path).await
            .map_err(|e| crate::RedisError::Storage(e.to_string()))?;
        
        // Update file size in stats
        let file_size = tokio::fs::metadata(&self.inner.aof_file).await
            .map(|m| m.len())
            .ok();
        
        self.inner.stats.write().await.aof_file_size = file_size;
        
        info!("AOF rewrite complete: {} commands", count);
        Ok(count)
    }
    
    /// Shutdown persistence manager
    pub async fn shutdown(&self) -> crate::Result<()> {
        // Final fsync for AOF
        if self.inner.config.aof_enabled {
            if let Some(ref mut w) = *self.inner.aof_writer.write().await {
                if let Err(e) = w.fsync().await {
                    error!("Final AOF fsync failed: {}", e);
                }
            }
        }
        
        // Save RDB on shutdown if enabled
        if self.inner.config.rdb_enabled {
            self.save().await?;
        }
        
        info!("Persistence manager shutdown complete");
        Ok(())
    }
}

// Implement Clone for PersistenceStats
impl Clone for PersistenceStats {
    fn clone(&self) -> Self {
        Self {
            rdb_last_save_time: self.rdb_last_save_time,
            rdb_last_save_status: self.rdb_last_save_status.clone(),
            rdb_file_size: self.rdb_file_size,
            aof_file_size: self.aof_file_size,
            aof_rewrite_in_progress: self.aof_rewrite_in_progress,
            aof_total_commands: self.aof_total_commands,
        }
    }
}
