//! OpenTelemetry integration for observability.
//!
//! Provides:
//! - OTLP trace export (spans sent via HTTP/protobuf to an OTel collector)
//! - Structured logging via `tracing-subscriber`
//! - Graceful degradation when no OTel collector is available
//!
//! # Environment variables
//!
//! | Variable | Default | Description |
//! |---|---|---|
//! | `OTEL_EXPORTER_OTLP_ENDPOINT` | `http://localhost:4318` | Base URL for the OTLP HTTP exporter |
//! | `RUST_LOG` | `info,fire_redis=debug` | Log filter (standard `tracing`/env-filter syntax) |

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_sdk::trace::TracerProvider;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// Initialize tracing, optionally exporting spans via OTLP.
///
/// If an OTel collector is reachable (default: `http://localhost:4318`), spans
/// are exported via HTTP/protobuf.  Otherwise the process logs to stdout only.
///
/// The returned [`OtelGuard`] **must** be kept alive for the lifetime of the
/// application — on drop it flushes and shuts down the OTel exporter.
pub fn init() -> OtelGuard {
    let provider = init_tracer_provider();

    // ── tracing-subscriber layers ────────────────────────────────────

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(true)
        .with_thread_ids(false);

    let filter_layer = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "info,fire_redis=debug".into());

    let registry = tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer);

    if let Some(ref provider) = provider {
        let tracer = provider.tracer("fire-redis");
        let otel_layer = tracing_opentelemetry::layer()
            .with_tracer(tracer);
        registry.with(otel_layer).init();
    } else {
        registry.init();
    }

    OtelGuard { provider }
}

// ── internal helpers ─────────────────────────────────────────────────

fn init_tracer_provider() -> Option<TracerProvider> {
    let endpoint =
        std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").unwrap_or_else(|_| String::new());

    let exporter = match opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .build()
    {
        Ok(e) => e,
        Err(e) => {
            eprintln!(
                "[otel] Failed to create OTLP span exporter: {e}\n\
                 [otel] Span export disabled. Set OTEL_EXPORTER_OTLP_ENDPOINT to enable."
            );
            return None;
        }
    };

    let provider = TracerProvider::builder()
        .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
        .build();

    eprintln!("[otel] OTel tracing initialised (endpoint: `{endpoint}`)");
    Some(provider)
}

// ── Guard ────────────────────────────────────────────────────────────

/// Keeps the OTel tracer provider alive and flushes spans on shutdown.
///
/// Drop this **after** the main server loop completes so that all in-flight
/// spans are exported before the process exits.
pub struct OtelGuard {
    provider: Option<TracerProvider>,
}

impl Drop for OtelGuard {
    fn drop(&mut self) {
        if let Some(ref provider) = self.provider {
            if let Err(e) = provider.shutdown() {
                eprintln!("[otel] Error shutting down tracer provider: {e}");
            }
        }
        opentelemetry::global::shutdown_tracer_provider();
    }
}
