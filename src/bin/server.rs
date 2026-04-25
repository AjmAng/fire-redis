use fire_redis::{Server, ServerConfig};
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // init tracing subscriber for logging
    tracing_subscriber::fmt()
        .with_env_filter("info,fire_redis=debug")
        .init();

    // resolve configuration from environment variables
    let config = ServerConfig {
        bind_addr: std::env::var("REDIS_BIND").unwrap_or_else(|_| "127.0.0.1".to_string()),
        port: std::env::var("REDIS_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(6379),
        ..Default::default()
    };

    let mut server = Server::new(config).await?;
    server.bind().await?;

    info!("Press Ctrl+C to stop the server");
    server.run().await?;

    Ok(())
}