//! Nostr MCP Server
//!
//! A Model Context Protocol (MCP) server that enables AI agents to interact
//! with the Nostr network for reading and writing notes.

mod mcp;
mod nostr_client;
mod tools;

use anyhow::Result;
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::mcp::McpServer;
use crate::nostr_client::NostrClientConfig;

/// Initialize logging with tracing subscriber.
/// Logs are written to stderr to avoid interfering with MCP communication on stdout.
fn init_logging() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_target(false)
                .compact(),
        )
        .init();
}

/// Load configuration from environment variables.
fn load_config() -> NostrClientConfig {
    // Load .env file if present
    if let Err(e) = dotenvy::dotenv() {
        info!("No .env file found or error loading it: {}", e);
    }

    // Try to load secret key from environment
    let secret_key = std::env::var("NSEC")
        .or_else(|_| std::env::var("NOSTR_SECRET_KEY"))
        .ok();

    if secret_key.is_none() {
        warn!("No secret key found (NSEC or NOSTR_SECRET_KEY). Running in read-only mode.");
    }

    // Load relay list from environment or use defaults
    let relays = std::env::var("NOSTR_RELAYS")
        .map(|s| s.split(',').map(|r| r.trim().to_string()).collect())
        .unwrap_or_else(|_| {
            vec![
                "wss://relay.damus.io".to_string(),
                "wss://nos.lol".to_string(),
                "wss://relay.nostr.band".to_string(),
                "wss://nostr.wine".to_string(),
                "wss://relay.snort.social".to_string(),
            ]
        });

    // Search-capable relays for search_notes
    let search_relays = std::env::var("NOSTR_SEARCH_RELAYS")
        .map(|s| s.split(',').map(|r| r.trim().to_string()).collect())
        .unwrap_or_else(|_| {
            vec![
                "wss://relay.nostr.band".to_string(),
                "wss://nostr.wine".to_string(),
            ]
        });

    NostrClientConfig {
        secret_key,
        relays,
        search_relays,
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    init_logging();

    info!("Starting Nostr MCP Server...");

    let config = load_config();

    info!("Loaded configuration:");
    info!("  - Relays: {:?}", config.relays);
    info!("  - Search relays: {:?}", config.search_relays);
    info!("  - Write access: {}", if config.secret_key.is_some() { "enabled" } else { "disabled (read-only)" });

    // Create and run MCP server
    let server = McpServer::new(config).await?;
    server.run().await?;

    Ok(())
}
