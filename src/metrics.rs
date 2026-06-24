//! Lightweight runtime metrics for the server.
//!
//! Metrics are stored as atomic counters so they can be updated from multiple
//! async tasks without locking. A `MetricsSnapshot` provides a consistent
//! point-in-time view for reporting.  Per-command counters and latency
//! histograms provide richer observability for operations teams and the
//! built-in `INFO` command.

use dashmap::DashMap;
use std::fmt::Write;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

// ── Histogram bucket thresholds (microseconds) ──────────────────────
/// Bucket boundaries for command-latency histograms.
const LATENCY_BUCKETS_US: &[u64] = &[10, 50, 100, 500, 1_000, 5_000, 10_000, 50_000];

// ── Snapshot ────────────────────────────────────────────────────────

/// Runtime metrics snapshot, returned by `Metrics::snapshot`.
#[derive(Debug, Clone, Default)]
pub struct MetricsSnapshot {
    // Existing counters
    pub total_commands: u64,
    pub error_commands: u64,
    pub total_connections: u64,
    pub active_connections: i64,
    pub evicted_keys: u64,
    pub rdb_saves: u64,
    pub aof_writes: u64,
    pub aof_rewrites: u64,

    // New counters
    pub total_net_input_bytes: u64,
    pub total_net_output_bytes: u64,
    pub expired_keys: u64,
    pub total_keys: i64,
    pub keyspace_hits: u64,
    pub keyspace_misses: u64,

    // Server info
    pub uptime_seconds: u64,

    // Per-command counters (top-N snapshot)
    pub command_counts: Vec<(String, u64)>,

    // Latency
    pub total_latency_us: u64,
    pub latency_count: u64,
    pub latency_buckets: Vec<(u64, u64)>, // (threshold_us, count)
}

impl MetricsSnapshot {
    /// Render the snapshot in Redis `INFO` style.
    pub fn to_info_string(&self) -> String {
        let avg_latency = if self.latency_count > 0 {
            self.total_latency_us / self.latency_count
        } else {
            0
        };

        let hit_rate = if self.keyspace_hits + self.keyspace_misses > 0 {
            (self.keyspace_hits as f64 / (self.keyspace_hits + self.keyspace_misses) as f64)
                * 100.0
        } else {
            100.0
        };

        let mut out = String::new();

        // ── Server ──
        let _ = writeln!(out, "# Server\r\nredis_version:{}", crate::VERSION);
        let _ = writeln!(out, "uptime_in_seconds:{}", self.uptime_seconds);
        let _ = writeln!(out, "tcp_port:6379\r\n");

        // ── Stats ──
        let _ = writeln!(out, "# Stats");
        let _ = writeln!(out, "total_commands:{}", self.total_commands);
        let _ = writeln!(out, "error_commands:{}", self.error_commands);
        let _ = writeln!(out, "total_connections_received:{}", self.total_connections);
        let _ = writeln!(out, "active_connections:{}", self.active_connections);
        let _ = writeln!(out, "total_keys:{}", self.total_keys);
        let _ = writeln!(out, "evicted_keys:{}", self.evicted_keys);
        let _ = writeln!(out, "expired_keys:{}", self.expired_keys);
        let _ = writeln!(out, "keyspace_hits:{}", self.keyspace_hits);
        let _ = writeln!(out, "keyspace_misses:{}", self.keyspace_misses);
        let _ = writeln!(out, "keyspace_hit_rate:{:.1}%", hit_rate);
        let _ = writeln!(
            out,
            "total_net_input_bytes:{}",
            self.total_net_input_bytes
        );
        let _ = writeln!(
            out,
            "total_net_output_bytes:{}",
            self.total_net_output_bytes
        );
        let _ = writeln!(out, "rdb_saves:{}", self.rdb_saves);
        let _ = writeln!(out, "aof_writes:{}", self.aof_writes);
        let _ = writeln!(out, "aof_rewrites:{}", self.aof_rewrites);

        let _ = writeln!(out, "avg_latency_us:{}", avg_latency);
        let _ = writeln!(
            out,
            "instantaneous_ops_per_sec:{}",
            self.total_commands.saturating_div(self.uptime_seconds.max(1))
        );

        // ── Latency histogram ──
        let _ = writeln!(out, "\r\n# Latency");
        for (threshold, count) in &self.latency_buckets {
            let _ = writeln!(out, "latency_bucket>{}us:{}", threshold, count);
        }

        // ── Command stats ──
        let _ = writeln!(out, "\r\n# Commandstats");
        for (cmd, count) in &self.command_counts {
            let _ = writeln!(
                out,
                "cmd_{}:{}",
                cmd.to_ascii_lowercase(),
                count
            );
        }

        // ── Keyspace ──
        let _ = writeln!(out, "\r\n# Keyspace");
        let _ = writeln!(
            out,
            "db0:keys={},expires={}",
            self.total_keys,
            // We don't have a separate expire count in the snapshot yet,
            // so we approximate with evicted+expired (best-effort).
            self.evicted_keys + self.expired_keys
        );

        let _ = writeln!(out, "\r\n# Replication\r\nrole:master\r\nconnected_slaves:0");

        out
    }
}

// ── Metrics ─────────────────────────────────────────────────────────

/// Shared, lock-free runtime metrics.
#[derive(Debug)]
pub struct Metrics {
    // Existing counters
    total_commands: AtomicU64,
    error_commands: AtomicU64,
    total_connections: AtomicU64,
    active_connections: AtomicI64,
    evicted_keys: AtomicU64,
    rdb_saves: AtomicU64,
    aof_writes: AtomicU64,
    aof_rewrites: AtomicU64,

    // New counters
    total_net_input_bytes: AtomicU64,
    total_net_output_bytes: AtomicU64,
    expired_keys: AtomicU64,
    total_keys: AtomicI64,
    keyspace_hits: AtomicU64,
    keyspace_misses: AtomicU64,

    // Per-command counters
    command_counts: DashMap<String, AtomicU64>,

    // Latency tracking
    total_latency_us: AtomicU64,
    latency_count: AtomicU64,
    latency_buckets: [AtomicU64; LATENCY_BUCKETS_US.len()],

    // Server start time
    start_time: Instant,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            total_commands: AtomicU64::new(0),
            error_commands: AtomicU64::new(0),
            total_connections: AtomicU64::new(0),
            active_connections: AtomicI64::new(0),
            evicted_keys: AtomicU64::new(0),
            rdb_saves: AtomicU64::new(0),
            aof_writes: AtomicU64::new(0),
            aof_rewrites: AtomicU64::new(0),
            total_net_input_bytes: AtomicU64::new(0),
            total_net_output_bytes: AtomicU64::new(0),
            expired_keys: AtomicU64::new(0),
            total_keys: AtomicI64::new(0),
            keyspace_hits: AtomicU64::new(0),
            keyspace_misses: AtomicU64::new(0),
            command_counts: DashMap::new(),
            total_latency_us: AtomicU64::new(0),
            latency_count: AtomicU64::new(0),
            latency_buckets: Default::default(),
            start_time: Instant::now(),
        }
    }
}

impl Metrics {
    pub fn new() -> Self {
        Self::default()
    }

    // ── Existing accessors ──

    pub fn record_command(&self) {
        self.total_commands.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_command_named(&self, cmd_name: &str) {
        self.total_commands.fetch_add(1, Ordering::Relaxed);
        let entry = self
            .command_counts
            .entry(cmd_name.to_string())
            .or_insert_with(|| AtomicU64::new(0));
        entry.value().fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_error(&self) {
        self.error_commands.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_connection_opened(&self) {
        self.total_connections.fetch_add(1, Ordering::Relaxed);
        self.active_connections.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_connection_closed(&self) {
        self.active_connections.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn record_evicted(&self, count: u64) {
        if count > 0 {
            self.evicted_keys.fetch_add(count, Ordering::Relaxed);
        }
    }

    pub fn record_rdb_save(&self) {
        self.rdb_saves.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_aof_write(&self) {
        self.aof_writes.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_aof_rewrite(&self) {
        self.aof_rewrites.fetch_add(1, Ordering::Relaxed);
    }

    // ── New accessors ──

    pub fn record_net_input(&self, bytes: u64) {
        self.total_net_input_bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn record_net_output(&self, bytes: u64) {
        self.total_net_output_bytes.fetch_add(bytes, Ordering::Relaxed);
    }

    pub fn record_expired_key(&self) {
        self.expired_keys.fetch_add(1, Ordering::Relaxed);
    }

    pub fn set_total_keys(&self, count: i64) {
        self.total_keys.store(count, Ordering::Relaxed);
    }

    pub fn record_keyspace_hit(&self) {
        self.keyspace_hits.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_keyspace_miss(&self) {
        self.keyspace_misses.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_latency(&self, elapsed_us: u64) {
        self.total_latency_us.fetch_add(elapsed_us, Ordering::Relaxed);
        self.latency_count.fetch_add(1, Ordering::Relaxed);
        // Record into histogram buckets
        for (i, threshold) in LATENCY_BUCKETS_US.iter().enumerate() {
            if elapsed_us <= *threshold {
                self.latency_buckets[i].fetch_add(1, Ordering::Relaxed);
                break;
            }
        }
    }

    // ── Snapshot ──

    pub fn snapshot(&self) -> MetricsSnapshot {
        // Collect per-command counters
        let mut command_counts: Vec<(String, u64)> = self
            .command_counts
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().load(Ordering::Relaxed)))
            .collect();
        command_counts.sort_by(|a, b| b.1.cmp(&a.1)); // descending by count

        // Collect latency buckets
        let latency_buckets: Vec<(u64, u64)> = LATENCY_BUCKETS_US
            .iter()
            .copied()
            .zip(self.latency_buckets.iter().map(|a| a.load(Ordering::Relaxed)))
            .collect();

        MetricsSnapshot {
            total_commands: self.total_commands.load(Ordering::Relaxed),
            error_commands: self.error_commands.load(Ordering::Relaxed),
            total_connections: self.total_connections.load(Ordering::Relaxed),
            active_connections: self.active_connections.load(Ordering::Relaxed),
            evicted_keys: self.evicted_keys.load(Ordering::Relaxed),
            rdb_saves: self.rdb_saves.load(Ordering::Relaxed),
            aof_writes: self.aof_writes.load(Ordering::Relaxed),
            aof_rewrites: self.aof_rewrites.load(Ordering::Relaxed),
            total_net_input_bytes: self.total_net_input_bytes.load(Ordering::Relaxed),
            total_net_output_bytes: self.total_net_output_bytes.load(Ordering::Relaxed),
            expired_keys: self.expired_keys.load(Ordering::Relaxed),
            total_keys: self.total_keys.load(Ordering::Relaxed),
            keyspace_hits: self.keyspace_hits.load(Ordering::Relaxed),
            keyspace_misses: self.keyspace_misses.load(Ordering::Relaxed),
            uptime_seconds: self.start_time.elapsed().as_secs(),
            command_counts,
            total_latency_us: self.total_latency_us.load(Ordering::Relaxed),
            latency_count: self.latency_count.load(Ordering::Relaxed),
            latency_buckets,
        }
    }

    /// Generate a full INFO-style string directly (most common usage).
    pub fn info_string(&self) -> String {
        self.snapshot().to_info_string()
    }
}

impl Clone for Metrics {
    fn clone(&self) -> Self {
        let mut latency_buckets: [AtomicU64; LATENCY_BUCKETS_US.len()] = Default::default();
        for (i, bucket) in self.latency_buckets.iter().enumerate() {
            latency_buckets[i] = AtomicU64::new(bucket.load(Ordering::Relaxed));
        }

        Self {
            total_commands: AtomicU64::new(self.total_commands.load(Ordering::Relaxed)),
            error_commands: AtomicU64::new(self.error_commands.load(Ordering::Relaxed)),
            total_connections: AtomicU64::new(self.total_connections.load(Ordering::Relaxed)),
            active_connections: AtomicI64::new(self.active_connections.load(Ordering::Relaxed)),
            evicted_keys: AtomicU64::new(self.evicted_keys.load(Ordering::Relaxed)),
            rdb_saves: AtomicU64::new(self.rdb_saves.load(Ordering::Relaxed)),
            aof_writes: AtomicU64::new(self.aof_writes.load(Ordering::Relaxed)),
            aof_rewrites: AtomicU64::new(self.aof_rewrites.load(Ordering::Relaxed)),
            total_net_input_bytes: AtomicU64::new(self.total_net_input_bytes.load(Ordering::Relaxed)),
            total_net_output_bytes: AtomicU64::new(self.total_net_output_bytes.load(Ordering::Relaxed)),
            expired_keys: AtomicU64::new(self.expired_keys.load(Ordering::Relaxed)),
            total_keys: AtomicI64::new(self.total_keys.load(Ordering::Relaxed)),
            keyspace_hits: AtomicU64::new(self.keyspace_hits.load(Ordering::Relaxed)),
            keyspace_misses: AtomicU64::new(self.keyspace_misses.load(Ordering::Relaxed)),
            command_counts: DashMap::new(),
            total_latency_us: AtomicU64::new(self.total_latency_us.load(Ordering::Relaxed)),
            latency_count: AtomicU64::new(self.latency_count.load(Ordering::Relaxed)),
            latency_buckets,
            start_time: Instant::now(),
        }
    }
}

impl From<Arc<Metrics>> for Metrics {
    fn from(arc: Arc<Metrics>) -> Self {
        (*arc).clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_increment_and_snapshot() {
        let m = Metrics::new();
        m.record_command();
        m.record_command();
        m.record_error();
        m.record_connection_opened();
        m.record_connection_opened();
        m.record_connection_closed();
        m.record_evicted(3);
        m.record_rdb_save();
        m.record_aof_write();
        m.record_aof_rewrite();
        m.record_keyspace_hit();
        m.record_keyspace_miss();
        m.record_expired_key();
        m.record_net_input(1024);
        m.record_net_output(512);

        let s = m.snapshot();
        assert_eq!(s.total_commands, 2);
        assert_eq!(s.error_commands, 1);
        assert_eq!(s.total_connections, 2);
        assert_eq!(s.active_connections, 1);
        assert_eq!(s.evicted_keys, 3);
        assert_eq!(s.rdb_saves, 1);
        assert_eq!(s.aof_writes, 1);
        assert_eq!(s.aof_rewrites, 1);
        assert_eq!(s.keyspace_hits, 1);
        assert_eq!(s.keyspace_misses, 1);
        assert_eq!(s.expired_keys, 1);
        assert_eq!(s.total_net_input_bytes, 1024);
        assert_eq!(s.total_net_output_bytes, 512);
    }

    #[test]
    fn clone_preserves_values() {
        let m = Metrics::new();
        m.record_command();
        let cloned = m.clone();
        assert_eq!(cloned.snapshot().total_commands, 1);
    }

    #[test]
    fn per_command_counts() {
        let m = Metrics::new();
        m.record_command_named("GET");
        m.record_command_named("GET");
        m.record_command_named("SET");
        m.record_command_named("DEL");

        let s = m.snapshot();
        assert_eq!(s.total_commands, 4);
        let cmds: Vec<_> = s.command_counts;
        assert!(cmds.contains(&("GET".into(), 2)));
        assert!(cmds.contains(&("SET".into(), 1)));
        assert!(cmds.contains(&("DEL".into(), 1)));
    }

    #[test]
    fn latency_buckets_recorded() {
        let m = Metrics::new();
        m.record_latency(5);   // falls into <= 10 bucket
        m.record_latency(75);  // falls into <= 100 bucket
        m.record_latency(5000); // falls into <= 5000 bucket

        let s = m.snapshot();
        assert_eq!(s.latency_count, 3);
        assert_eq!(s.total_latency_us, 5080);
        assert!(s.latency_buckets.iter().any(|(t, c)| *t == 10 && *c == 1));
        assert!(s.latency_buckets.iter().any(|(t, c)| *t == 100 && *c == 1));
        assert!(s.latency_buckets.iter().any(|(t, c)| *t == 5000 && *c == 1));
    }

    #[test]
    fn info_string_contains_sections() {
        let m = Metrics::new();
        m.record_command_named("GET");
        m.record_command_named("SET");
        m.record_keyspace_hit();

        let info = m.info_string();
        assert!(info.contains("# Server"));
        assert!(info.contains("# Stats"));
        assert!(info.contains("# Commandstats"));
        assert!(info.contains("# Keyspace"));
        assert!(info.contains("# Replication"));
        assert!(info.contains("cmd_get:1"));
        assert!(info.contains("cmd_set:1"));
    }
}
