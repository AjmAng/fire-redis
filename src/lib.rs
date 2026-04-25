//! Redis-rs: A high-performance Redis-compatible server implementation in Rust
//!
//! This library provides the core components for building a Redis-compatible
//! key-value store with async I/O and memory-safe operations.

pub mod commands;
pub mod persistence;
pub mod resp;
pub mod server;
pub mod store;

pub use resp::{RespCodec, Value};
pub use server::{Server, ServerConfig};
pub use store::{Store, StoredValue};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum RedisError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("RESP protocol error: {0}")]
    Resp(#[from] resp::RespError),  

    #[error("Command error: {0}")]
    Command(String),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Shutdown signal received")]
    Shutdown,
}

pub type Result<T> = std::result::Result<T, RedisError>;

pub const VERSION: &str = include_str!("../version");
pub const PROTOCOL_VERSION: &str = "RESP2";