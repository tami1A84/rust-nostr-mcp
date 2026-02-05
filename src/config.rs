//! Configuration Module
//!
//! Handles loading and saving configuration from ~/.config/rust-nostr-mcp/config.json
//! Following the algia convention for configuration file structure.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use tracing::{info, warn};

/// Relay configuration following algia convention.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayConfig {
    /// Whether to read from this relay
    pub read: bool,
    /// Whether to write to this relay
    pub write: bool,
    /// Whether this relay supports NIP-50 search
    pub search: bool,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            read: true,
            write: true,
            search: false,
        }
    }
}

/// Main configuration structure following algia convention.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Relay configurations keyed by URL
    pub relays: HashMap<String, RelayConfig>,
    /// Private key in nsec format (stored locally, never passed to AI agents)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub privatekey: Option<String>,
    /// Nostr Wallet Connect URI (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "nwc-uri")]
    pub nwc_uri: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        let mut relays = HashMap::new();

        // Default relays
        relays.insert(
            "wss://relay.damus.io".to_string(),
            RelayConfig { read: true, write: true, search: false },
        );
        relays.insert(
            "wss://nos.lol".to_string(),
            RelayConfig { read: true, write: true, search: false },
        );
        relays.insert(
            "wss://relay.nostr.band".to_string(),
            RelayConfig { read: true, write: true, search: true },
        );
        relays.insert(
            "wss://nostr.wine".to_string(),
            RelayConfig { read: true, write: false, search: true },
        );
        relays.insert(
            "wss://relay.snort.social".to_string(),
            RelayConfig { read: true, write: true, search: false },
        );

        Self {
            relays,
            privatekey: None,
            nwc_uri: None,
        }
    }
}

impl Config {
    /// Get the configuration file path.
    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Could not determine config directory")?
            .join("rust-nostr-mcp");

        Ok(config_dir.join("config.json"))
    }

    /// Load configuration from the config file.
    /// Falls back to environment variables for backward compatibility.
    pub fn load() -> Result<Self> {
        let config_path = Self::config_path()?;

        if config_path.exists() {
            info!("Loading configuration from {:?}", config_path);
            let content = fs::read_to_string(&config_path)
                .context("Failed to read config file")?;
            let config: Config = serde_json::from_str(&content)
                .context("Failed to parse config file")?;
            return Ok(config);
        }

        // Fall back to environment variables for backward compatibility
        warn!("Config file not found at {:?}, checking environment variables", config_path);
        Self::load_from_env()
    }

    /// Load configuration from environment variables (backward compatibility).
    fn load_from_env() -> Result<Self> {
        // Load .env file if present
        let _ = dotenvy::dotenv();

        let mut config = Self::default();

        // Try to load secret key from environment
        if let Ok(nsec) = std::env::var("NSEC") {
            config.privatekey = Some(nsec);
        } else if let Ok(hex_key) = std::env::var("NOSTR_SECRET_KEY") {
            config.privatekey = Some(hex_key);
        }

        // Load custom relays if specified
        if let Ok(relay_list) = std::env::var("NOSTR_RELAYS") {
            config.relays.clear();
            for relay in relay_list.split(',').map(|s| s.trim()) {
                config.relays.insert(
                    relay.to_string(),
                    RelayConfig { read: true, write: true, search: false },
                );
            }
        }

        // Load search relays if specified
        if let Ok(search_list) = std::env::var("NOSTR_SEARCH_RELAYS") {
            for relay in search_list.split(',').map(|s| s.trim()) {
                config.relays
                    .entry(relay.to_string())
                    .and_modify(|r| r.search = true)
                    .or_insert(RelayConfig { read: true, write: false, search: true });
            }
        }

        Ok(config)
    }

    /// Save configuration to the config file.
    pub fn save(&self) -> Result<()> {
        let config_path = Self::config_path()?;

        // Create parent directories if they don't exist
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .context("Failed to create config directory")?;
        }

        let content = serde_json::to_string_pretty(self)
            .context("Failed to serialize config")?;

        fs::write(&config_path, content)
            .context("Failed to write config file")?;

        info!("Configuration saved to {:?}", config_path);
        Ok(())
    }

    /// Create a default config file if it doesn't exist.
    pub fn create_default_if_missing() -> Result<bool> {
        let config_path = Self::config_path()?;

        if !config_path.exists() {
            let default_config = Self::default();
            default_config.save()?;
            info!("Created default configuration at {:?}", config_path);
            return Ok(true);
        }

        Ok(false)
    }

    /// Get read-enabled relay URLs.
    pub fn read_relays(&self) -> Vec<String> {
        self.relays
            .iter()
            .filter(|(_, c)| c.read)
            .map(|(url, _)| url.clone())
            .collect()
    }

    /// Get write-enabled relay URLs.
    pub fn write_relays(&self) -> Vec<String> {
        self.relays
            .iter()
            .filter(|(_, c)| c.write)
            .map(|(url, _)| url.clone())
            .collect()
    }

    /// Get search-enabled relay URLs.
    pub fn search_relays(&self) -> Vec<String> {
        self.relays
            .iter()
            .filter(|(_, c)| c.search)
            .map(|(url, _)| url.clone())
            .collect()
    }

    /// Check if the config has a private key set.
    pub fn has_private_key(&self) -> bool {
        self.privatekey.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(!config.relays.is_empty());
        assert!(config.privatekey.is_none());
    }

    #[test]
    fn test_relay_filtering() {
        let config = Config::default();
        let read_relays = config.read_relays();
        let search_relays = config.search_relays();

        assert!(!read_relays.is_empty());
        assert!(!search_relays.is_empty());
    }
}
