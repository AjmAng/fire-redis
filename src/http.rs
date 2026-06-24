//! Lightweight HTTP endpoint for health checks and Prometheus-style metrics.
//!
//! Runs on a separate port (default 6380) so it never interferes with the
//! Redis protocol handler.  Exposes three routes:
//!
//! * `GET /`         – HTML overview page
//! * `GET /health`   – JSON health-check (always 200 when the server is up)
//! * `GET /metrics`  – Prometheus-compatible text dump

use crate::metrics::Metrics;
use std::sync::Arc;

// ── HTTP status / content-type helpers ──────────────────────────────

const OK: &[u8] = b"HTTP/1.1 200 OK\r\n";
const NOT_FOUND: &[u8] = b"HTTP/1.1 404 Not Found\r\n";

fn content_len(n: usize) -> String {
    format!("Content-Length: {}\r\n", n)
}

fn response_headers(content_type: &str) -> String {
    format!("Content-Type: {}\r\nConnection: close\r\n\r\n", content_type)
}

fn ok_response(content_type: &str, body: &str) -> Vec<u8> {
    let headers = response_headers(content_type);
    let cl = content_len(body.len());
    let mut buf = Vec::with_capacity(OK.len() + cl.len() + headers.len() + body.len());
    buf.extend_from_slice(OK);
    buf.extend_from_slice(cl.as_bytes());
    buf.extend_from_slice(headers.as_bytes());
    buf.extend_from_slice(body.as_bytes());
    buf
}

fn not_found(body: &str) -> Vec<u8> {
    let headers = response_headers("text/plain");
    let cl = content_len(body.len());
    let mut buf = Vec::with_capacity(NOT_FOUND.len() + cl.len() + headers.len() + body.len());
    buf.extend_from_slice(NOT_FOUND);
    buf.extend_from_slice(cl.as_bytes());
    buf.extend_from_slice(headers.as_bytes());
    buf.extend_from_slice(body.as_bytes());
    buf
}

// ── Route handlers ──────────────────────────────────────────────────

fn handle_root() -> Vec<u8> {
    let body = r#"<!DOCTYPE html>
<html>
<head><title>Fire-Redis Metrics</title></head>
<body>
<h1>Fire-Redis</h1>
<ul>
  <li><a href="/health">/health</a> – JSON health check</li>
  <li><a href="/metrics">/metrics</a> – Prometheus metrics</li>
</ul>
</body>
</html>"#;
    ok_response("text/html", body)
}

fn handle_health(metrics: &Metrics) -> Vec<u8> {
    let s = metrics.snapshot();
    // Simple JSON health body
    let body = format!(
        r#"{{"status":"ok","uptime_seconds":{},"active_connections":{},"total_commands":{},"total_keys":{}}}"#,
        s.uptime_seconds, s.active_connections, s.total_commands, s.total_keys,
    );
    ok_response("application/json", &body)
}

fn handle_metrics(metrics: &Metrics) -> Vec<u8> {
    let s = metrics.snapshot();
    let mut body = String::new();

    // ── Help & Type headers ──
    body.push_str("# HELP fire_redis_total_commands Total number of commands processed\n");
    body.push_str("# TYPE fire_redis_total_commands counter\n");
    body.push_str(&format!("fire_redis_total_commands {}\n", s.total_commands));

    body.push_str("# HELP fire_redis_error_commands Number of errored commands\n");
    body.push_str("# TYPE fire_redis_error_commands counter\n");
    body.push_str(&format!("fire_redis_error_commands {}\n", s.error_commands));

    body.push_str("# HELP fire_redis_connections_received Total connections received\n");
    body.push_str("# TYPE fire_redis_connections_received counter\n");
    body.push_str(&format!("fire_redis_connections_received {}\n", s.total_connections));

    body.push_str("# HELP fire_redis_active_connections Currently active connections\n");
    body.push_str("# TYPE fire_redis_active_connections gauge\n");
    body.push_str(&format!("fire_redis_active_connections {}\n", s.active_connections));

    body.push_str("# HELP fire_redis_total_keys Total keys in database\n");
    body.push_str("# TYPE fire_redis_total_keys gauge\n");
    body.push_str(&format!("fire_redis_total_keys {}\n", s.total_keys));

    body.push_str("# HELP fire_redis_evicted_keys Keys evicted due to expiration\n");
    body.push_str("# TYPE fire_redis_evicted_keys counter\n");
    body.push_str(&format!("fire_redis_evicted_keys {}\n", s.evicted_keys));

    body.push_str("# HELP fire_redis_expired_keys Keys expired\n");
    body.push_str("# TYPE fire_redis_expired_keys counter\n");
    body.push_str(&format!("fire_redis_expired_keys {}\n", s.expired_keys));

    body.push_str("# HELP fire_redis_keyspace_hits Keyspace hits\n");
    body.push_str("# TYPE fire_redis_keyspace_hits counter\n");
    body.push_str(&format!("fire_redis_keyspace_hits {}\n", s.keyspace_hits));

    body.push_str("# HELP fire_redis_keyspace_misses Keyspace misses\n");
    body.push_str("# TYPE fire_redis_keyspace_misses counter\n");
    body.push_str(&format!("fire_redis_keyspace_misses {}\n", s.keyspace_misses));

    body.push_str("# HELP fire_redis_net_input_bytes Total network input bytes\n");
    body.push_str("# TYPE fire_redis_net_input_bytes counter\n");
    body.push_str(&format!("fire_redis_net_input_bytes {}\n", s.total_net_input_bytes));

    body.push_str("# HELP fire_redis_net_output_bytes Total network output bytes\n");
    body.push_str("# TYPE fire_redis_net_output_bytes counter\n");
    body.push_str(&format!("fire_redis_net_output_bytes {}\n", s.total_net_output_bytes));

    body.push_str("# HELP fire_redis_rdb_saves RDB saves\n");
    body.push_str("# TYPE fire_redis_rdb_saves counter\n");
    body.push_str(&format!("fire_redis_rdb_saves {}\n", s.rdb_saves));

    body.push_str("# HELP fire_redis_aof_writes AOF writes\n");
    body.push_str("# TYPE fire_redis_aof_writes counter\n");
    body.push_str(&format!("fire_redis_aof_writes {}\n", s.aof_writes));

    body.push_str("# HELP fire_redis_aof_rewrites AOF rewrites\n");
    body.push_str("# TYPE fire_redis_aof_rewrites counter\n");
    body.push_str(&format!("fire_redis_aof_rewrites {}\n", s.aof_rewrites));

    body.push_str("# HELP fire_redis_uptime_seconds Server uptime\n");
    body.push_str("# TYPE fire_redis_uptime_seconds counter\n");
    body.push_str(&format!("fire_redis_uptime_seconds {}\n", s.uptime_seconds));

    // Per-command counters
    for (cmd, count) in &s.command_counts {
        let safe_name = cmd.to_ascii_lowercase().replace('.', "_");
        body.push_str(&format!("# HELP fire_redis_cmd_{} Command count for {}\n", safe_name, cmd));
        body.push_str(&format!("# TYPE fire_redis_cmd_{} counter\n", safe_name));
        body.push_str(&format!("fire_redis_cmd_{} {}\n", safe_name, count));
    }

    // Latency histogram
    body.push_str("# HELP fire_redis_latency_us Command latency histogram buckets\n");
    body.push_str("# TYPE fire_redis_latency_us histogram\n");
    for (threshold, count) in &s.latency_buckets {
        body.push_str(&format!(
            "fire_redis_latency_us_bucket{{le=\"{}\"}} {}\n",
            threshold, count
        ));
    }
    body.push_str(&format!(
        "fire_redis_latency_us_bucket{{le=\"+Inf\"}} {}\n",
        s.latency_count
    ));
    body.push_str(&format!("fire_redis_latency_us_count {}\n", s.latency_count));
    body.push_str(&format!("fire_redis_latency_us_sum {}\n", s.total_latency_us));

    ok_response("text/plain; version=0.0.4", &body)
}

// ── Request routing ─────────────────────────────────────────────────

/// Parse the HTTP request line from a raw byte buffer and return the path.
fn parse_path(buf: &[u8]) -> Option<&str> {
    let s = std::str::from_utf8(buf).ok()?;
    let line = s.lines().next()?; // e.g. "GET /health HTTP/1.1"
    let path = line.split_whitespace().nth(1)?;
    Some(path)
}

/// Run the HTTP metrics server on the given port.
///
/// This is intended to be spawned as a background task from the main
/// server (e.g. `tokio::spawn(http::serve(6380, metrics))`).
pub async fn serve(port: u16, metrics: Arc<Metrics>) -> Result<(), crate::RedisError> {
    let addr = format!("127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("HTTP metrics endpoint listening on {}", addr);

    loop {
        let (mut socket, _) = listener.accept().await?;
        let metrics = metrics.clone();

        tokio::spawn(async move {
            use tokio::io::AsyncReadExt;

            let mut buf = vec![0u8; 4096];
            let n = match socket.read(&mut buf).await {
                Ok(n) if n > 0 => n,
                _ => return,
            };

            let path = parse_path(&buf[..n]).unwrap_or("");
            let response = match path {
                "/" => handle_root(),
                "/health" => handle_health(&metrics),
                "/metrics" => handle_metrics(&metrics),
                _ => not_found("Not Found"),
            };

            let _ = tokio::io::AsyncWriteExt::write_all(&mut socket, &response).await;
        });
    }
}
