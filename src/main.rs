//! Nostr MCP Server
//!
//! A Model Context Protocol (MCP) server that enables AI agents to interact
//! with the Nostr network for reading and writing notes.
//!
//! Configuration is stored in ~/.config/rust-nostr-mcp/config.json
//! Private keys are stored locally and never passed to AI agents.

mod config;
mod mcp;
mod nostr_client;
mod tools;

use anyhow::Result;
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::config::Config;
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

/// Load configuration from config file (~/.config/rust-nostr-mcp/config.json).
/// Falls back to environment variables for backward compatibility.
fn load_config() -> NostrClientConfig {
    // Try to load from config file first
    let config = match Config::load() {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to load config file, using defaults: {}", e);
            Config::default()
        }
    };

    let secret_key = config.privatekey.clone();

    if secret_key.is_none() {
        warn!("No private key configured. Running in read-only mode.");
        warn!("To enable write access, add your nsec to: {:?}", Config::config_path().unwrap_or_default());
    }

    let relays = config.read_relays();
    let search_relays = config.search_relays();

    NostrClientConfig {
        secret_key,
        relays,
        search_relays,
    }
}

/// Print configuration instructions on first run.
fn print_setup_instructions() {
    let config_path = Config::config_path().unwrap_or_default();
    eprintln!();
    eprintln!("=== Nostr MCP Server Setup ===");
    eprintln!();
    eprintln!("Configuration file: {:?}", config_path);
    eprintln!();
    eprintln!("To enable posting, add your private key (nsec) to the config file:");
    eprintln!();
    eprintln!("  {{");
    eprintln!("    \"relays\": {{");
    eprintln!("      \"wss://relay.damus.io\": {{ \"read\": true, \"write\": true, \"search\": false }}");
    eprintln!("    }},");
    eprintln!("    \"privatekey\": \"nsec1...\"");
    eprintln!("  }}");
    eprintln!();
    eprintln!("IMPORTANT: Your private key is stored locally and never passed to AI agents.");
    eprintln!();
}

#[tokio::main]
async fn main() -> Result<()> {
    init_logging();

    info!("Starting Nostr MCP Server...");

    // Create default config file if it doesn't exist
    match Config::create_default_if_missing() {
        Ok(true) => print_setup_instructions(),
        Ok(false) => {}
        Err(e) => warn!("Could not create default config: {}", e),
    }

    let config = load_config();

    info!("Loaded configuration:");
    info!("  - Read relays: {:?}", config.relays);
    info!("  - Search relays: {:?}", config.search_relays);
    info!("  - Write access: {}", if config.secret_key.is_some() { "enabled" } else { "disabled (read-only)" });

    // Create and run MCP server
    let server = McpServer::new(config).await?;
    server.run().await?;

    Ok(())
}
