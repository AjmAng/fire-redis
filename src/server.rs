//! TCP Server implementation with graceful shutdown support

use crate::{
    commands::Command,
    persistence::{PersistenceConfig, PersistenceManager},
    resp::{RespCodec, Value},
    store::Store,
    RedisError, Result,
};
use std::net::SocketAddr;
use std::time::Duration;
use tokio::{
    net::{TcpListener, TcpStream},
    signal,
    sync::{broadcast, mpsc},
};
use tokio_util::codec::Framed;
use futures::{SinkExt, StreamExt};
use tracing::{debug, error, info};

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub bind_addr: String,
    pub port: u16,
    pub max_connections: usize,
    pub timeout_secs: u64,
    pub eviction_interval_ms: u64,
    pub persistence: PersistenceConfig,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1".to_string(),
            port: 6379,
            max_connections: 10000,
            timeout_secs: 30,
            eviction_interval_ms: 1000,
            persistence: PersistenceConfig::default(),
        }
    }
}

impl ServerConfig {
    pub fn new(bind_addr: &str, port: u16) -> Self {
        Self {
            bind_addr: bind_addr.to_string(),
            port,
            ..Default::default()
        }
    }

    pub fn with_persistence(bind_addr: &str, port: u16, persistence: PersistenceConfig) -> Self {
        Self {
            bind_addr: bind_addr.to_string(),
            port,
            persistence,
            ..Default::default()
        }
    }

    pub fn with_eviction_interval_ms(mut self, eviction_interval_ms: u64) -> Self {
        self.eviction_interval_ms = eviction_interval_ms;
        self
    }

    pub fn socket_addr(&self) -> Result<SocketAddr> {
        format!("{}:{}", self.bind_addr, self.port)
            .parse()
            .map_err(|e| RedisError::Command(format!("Invalid address: {}", e)))
    }
}

pub struct Server {
    config: ServerConfig,
    store: Store,
    listener: Option<TcpListener>,
    shutdown_tx: broadcast::Sender<()>,
    persistence_manager: Option<PersistenceManager>,
}

impl Server {
    pub async fn new(config: ServerConfig) -> Result<Self> {
        let store = Store::new();
        let (shutdown_tx, _) = broadcast::channel(1);

        // Initialize persistence manager
        let persistence_manager = if config.persistence.rdb_enabled || config.persistence.aof_enabled {
            Some(PersistenceManager::new(config.persistence.clone(), store.clone()).await?)
        } else {
            None
        };

        Ok(Self {
            config,
            store,
            listener: None,
            shutdown_tx,
            persistence_manager,
        })
    }

    pub async fn from_listener(listener: TcpListener) -> Result<Self> {
        let config = ServerConfig::default(); 
        let store = Store::new();
        let (shutdown_tx, _) = broadcast::channel(1);

        // Initialize persistence manager with default config
        let persistence_manager = Some(PersistenceManager::new(PersistenceConfig::default(), store.clone()).await?);

        Ok(Self {
            config,
            store,
            listener: Some(listener),
            shutdown_tx,
            persistence_manager,
        })
    }

    pub async fn bind(&mut self) -> Result<()> {
        let addr = self.config.socket_addr()?;
        let listener = TcpListener::bind(addr).await?;
        info!("Server bound to {}", addr);
        self.listener = Some(listener);
        Ok(())
    }
    

    /// Load data from persistence files on startup
    pub async fn load_data(&self) -> Result<()> {
        if let Some(ref pm) = self.persistence_manager {
            info!("Loading data from persistence files...");
            pm.load_on_startup().await?;
            info!("Data load complete");
        }
        Ok(())
    }

    pub async fn run(&mut self) -> Result<()> {
        let listener = self.listener.take()
            .ok_or_else(|| RedisError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Server not bound"
            )))?;

        info!(
            "Fire-Redis v{} (Protocol: {}) starting...",
            crate::VERSION,
            crate::PROTOCOL_VERSION
        );
        info!("Listening on {}", self.config.socket_addr()?);
        info!("Max connections: {}", self.config.max_connections);
        if self.config.eviction_interval_ms == 0 {
            info!("Background expiration eviction disabled");
        } else {
            info!("Background expiration eviction every {} ms", self.config.eviction_interval_ms);
        }

        // Log persistence status
        if self.config.persistence.rdb_enabled {
            info!("RDB persistence enabled: {:?}", self.config.persistence.rdb_file);
        }
        if self.config.persistence.aof_enabled {
            info!("AOF persistence enabled: {:?}", self.config.persistence.aof_file);
        }

        // Load existing data
        self.load_data().await?;

        // Start background persistence tasks
        if let Some(ref pm) = self.persistence_manager {
            pm.start_background_tasks().await;
        }

        if self.config.eviction_interval_ms > 0 {
            Self::start_expiration_evictor(
                self.store.clone(),
                self.shutdown_tx.subscribe(),
                Duration::from_millis(self.config.eviction_interval_ms),
            );
        }

        let (conn_tx, mut conn_rx) = mpsc::channel::<()>(self.config.max_connections);

        let _shutdown_rx = self.shutdown_tx.subscribe();
        
        let persistence_manager = self.persistence_manager.clone();

        loop {
            tokio::select! {
                result = listener.accept() => {
                    match result {
                        Ok((socket, addr)) => {
                            let store = self.store.clone();
                            let shutdown = self.shutdown_tx.subscribe();
                            let conn_tx = conn_tx.clone();
                            let pm = persistence_manager.clone();

                            info!("New connection from {}", addr);

                            tokio::spawn(async move {
                                if let Err(e) = handle_connection(socket, addr, store, shutdown, pm).await {
                                    error!("Connection error from {}: {}", addr, e);
                                }
                                drop(conn_tx); 
                            });
                        }
                        Err(e) => {
                            error!("Accept error: {}", e);
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                    }
                }

                _ = signal::ctrl_c() => {
                    info!("Shutdown signal received, stopping server...");
                    let _ = self.shutdown_tx.send(());
                    break;
                }

                _ = conn_rx.recv() => {
                    if conn_rx.len() == 0 {
                        info!("All connections closed");
                    }
                }
            }
        }

        // Save data before shutdown
        if let Some(ref pm) = self.persistence_manager {
            info!("Saving data before shutdown...");
            if let Err(e) = pm.shutdown().await {
                error!("Persistence shutdown error: {}", e);
            }
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
        info!("Server stopped");
        Ok(())
    }

    pub fn shutdown(&self) {
        let _ = self.shutdown_tx.send(());
    }

    pub fn store(&self) -> &Store {
        &self.store
    }
    
    pub fn persistence_manager(&self) -> Option<&PersistenceManager> {
        self.persistence_manager.as_ref()
    }

    fn start_expiration_evictor(
        store: Store,
        mut shutdown: broadcast::Receiver<()>,
        interval_duration: Duration,
    ) {
        tokio::spawn(async move {
            let mut evict_interval = tokio::time::interval(interval_duration);

            loop {
                tokio::select! {
                    _ = evict_interval.tick() => {
                        let evicted = store.evict_expired();
                        if evicted > 0 {
                            debug!("Evicted {} expired keys", evicted);
                        }
                    }
                    _ = shutdown.recv() => {
                        break;
                    }
                }
            }
        });
    }
}

async fn handle_connection(
    socket: TcpStream,
    addr: SocketAddr,
    store: Store,
    mut shutdown: broadcast::Receiver<()>,
    persistence_manager: Option<PersistenceManager>,
) -> Result<()> {
    let mut framed = Framed::new(socket, RespCodec);

    loop {
        tokio::select! {
            result = framed.next() => {
                match result {
                    Some(Ok(value)) => {
                        if let Value::Array(Some(args)) = value {
                            if args.is_empty() {
                                framed.send(Value::Error("ERR empty command".to_string())).await?;
                                continue;
                            }

                            // Check if this is a write command and log to AOF
                            if let Some(ref pm) = persistence_manager {
                                if let Value::BulkString(Some(ref cmd_bytes)) = args[0] {
                                    let cmd_name = String::from_utf8_lossy(cmd_bytes);
                                    if crate::persistence::aof::should_log_to_aof(&cmd_name) {
                                        if let Some(channel) = pm.aof_channel() {
                                            channel.log(Value::Array(Some(args.clone())));
                                        }
                                        pm.record_write();
                                    }
                                }
                            }

                            let response = Command::try_from(args)
                                .map_or_else(|e| e, |cmd| cmd.execute(&store));

                            if let Value::SimpleString(ref s) = response {
                                if s == "OK" && is_quit_command(&response) {
                                    framed.send(response).await?;
                                    info!("Client {} disconnected (QUIT)", addr);
                                    break;
                                }
                            }

                            framed.send(response).await?;
                        } else {
                            framed.send(Value::Error("ERR invalid request format".to_string())).await?;
                        }
                    }
                    Some(Err(e)) => {
                        error!("Protocol error from {}: {}", addr, e);
                        framed.send(Value::Error(format!("ERR protocol error: {}", e))).await?;
                    }
                    None => {
                        info!("Client {} disconnected", addr);
                        break;
                    }
                }
            }

            _ = shutdown.recv() => {
                info!("Closing connection {} due to server shutdown", addr);
                let _ = framed.send(Value::Error("ERR Server shutting down".to_string())).await;
                break;
            }
        }
    }

    Ok(())
}

fn is_quit_command(_response: &Value) -> bool {
    false
}
