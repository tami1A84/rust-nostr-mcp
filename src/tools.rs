//! MCP Tools Module
//!
//! Defines the available tools that AI agents can use to interact
//! with the Nostr network. Tool names follow the algia convention
//! with `nostr_` prefix for clarity.
//!
//! Security: Private keys are stored in the local config file
//! (~/.config/rust-nostr-mcp/config.json) and never passed to AI agents.

use anyhow::{anyhow, Result};
use nostr_sdk::ToBech32;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, info};

use crate::nostr_client::NostrClient;

/// Tool definitions for MCP.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// The name of the tool
    pub name: String,
    /// A description of what the tool does
    pub description: String,
    /// JSON Schema for the tool's input parameters
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

/// Returns the list of available tools.
/// Tool names follow the algia convention with `nostr_` prefix.
pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "post_nostr_note".to_string(),
            description: "Post a new short text note (Kind 1) to the Nostr network. Requires write access (private key must be configured in ~/.config/rust-nostr-mcp/config.json).".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "The text content of the note to post"
                    }
                },
                "required": ["content"]
            }),
        },
        ToolDefinition {
            name: "get_nostr_timeline".to_string(),
            description: "Get the latest notes from the Nostr timeline with author information (name, display_name, picture, nip05). If authenticated, returns notes from followed users; otherwise, returns the global timeline.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "number",
                        "description": "Maximum number of notes to retrieve (default: 20, max: 100)"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "search_nostr_notes".to_string(),
            description: "Search for notes on the Nostr network containing the specified keywords using NIP-50 search-capable relays. Returns notes with author information.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query string"
                    },
                    "limit": {
                        "type": "number",
                        "description": "Maximum number of results to return (default: 20, max: 100)"
                    }
                },
                "required": ["query"]
            }),
        },
        ToolDefinition {
            name: "get_nostr_profile".to_string(),
            description: "Get the profile information for a Nostr user by their public key (npub or hex format). Returns name, display_name, about, picture, banner, nip05, lud16, and website.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pubkey": {
                        "type": "string",
                        "description": "The user's public key in npub (bech32) or hex format"
                    }
                },
                "required": ["pubkey"]
            }),
        },
    ]
}

/// Tool executor that handles tool invocations.
pub struct ToolExecutor {
    /// The Nostr client instance
    client: Arc<NostrClient>,
}

impl ToolExecutor {
    /// Creates a new tool executor.
    pub fn new(client: Arc<NostrClient>) -> Self {
        Self { client }
    }

    /// Executes a tool with the given arguments.
    ///
    /// # Arguments
    /// * `name` - The name of the tool to execute
    /// * `arguments` - The JSON arguments for the tool
    ///
    /// # Returns
    /// The result of the tool execution as a JSON value.
    pub async fn execute(&self, name: &str, arguments: Value) -> Result<Value> {
        info!("Executing tool: {} with arguments: {}", name, arguments);

        match name {
            // New names with nostr_ prefix (algia convention)
            "post_nostr_note" => self.post_note(arguments).await,
            "get_nostr_timeline" => self.get_timeline(arguments).await,
            "search_nostr_notes" => self.search_notes(arguments).await,
            "get_nostr_profile" => self.get_profile(arguments).await,
            // Legacy names for backward compatibility
            "post_note" => self.post_note(arguments).await,
            "get_timeline" => self.get_timeline(arguments).await,
            "search_notes" => self.search_notes(arguments).await,
            "get_profile" => self.get_profile(arguments).await,
            _ => Err(anyhow!("Unknown tool: {}", name)),
        }
    }

    /// Posts a new note to Nostr.
    async fn post_note(&self, arguments: Value) -> Result<Value> {
        let content = arguments
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing required argument: content"))?;

        if content.is_empty() {
            return Err(anyhow!("Content cannot be empty"));
        }

        let event_id = self.client.post_note(content).await?;

        Ok(json!({
            "success": true,
            "event_id": event_id.to_hex(),
            "nevent": event_id.to_bech32().unwrap_or_default(),
            "message": format!("Note posted successfully with event ID: {}", event_id.to_hex())
        }))
    }

    /// Gets the timeline with modern display format.
    async fn get_timeline(&self, arguments: Value) -> Result<Value> {
        let limit = arguments
            .get("limit")
            .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
            .unwrap_or(20)
            .min(100);

        debug!("Fetching timeline with limit: {}", limit);

        let notes = self.client.get_timeline(limit).await?;

        // Format notes for modern display
        let formatted_notes: Vec<Value> = notes.iter().map(|note| {
            json!({
                "id": note.id,
                "nevent": note.nevent,
                "author": {
                    "pubkey": note.author.pubkey,
                    "npub": note.author.npub,
                    "name": note.author.name,
                    "display_name": note.author.display_name,
                    "display": note.author.display(),
                    "picture": note.author.picture,
                    "nip05": note.author.nip05
                },
                "content": note.content,
                "created_at": note.created_at,
                "formatted_time": format_timestamp(note.created_at)
            })
        }).collect();

        Ok(json!({
            "success": true,
            "count": notes.len(),
            "notes": formatted_notes
        }))
    }

    /// Searches for notes.
    async fn search_notes(&self, arguments: Value) -> Result<Value> {
        let query = arguments
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing required argument: query"))?;

        if query.is_empty() {
            return Err(anyhow!("Query cannot be empty"));
        }

        let limit = arguments
            .get("limit")
            .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
            .unwrap_or(20)
            .min(100);

        debug!("Searching notes with query: '{}', limit: {}", query, limit);

        let notes = self.client.search_notes(query, limit).await?;

        // Format notes for modern display
        let formatted_notes: Vec<Value> = notes.iter().map(|note| {
            json!({
                "id": note.id,
                "nevent": note.nevent,
                "author": {
                    "pubkey": note.author.pubkey,
                    "npub": note.author.npub,
                    "name": note.author.name,
                    "display_name": note.author.display_name,
                    "display": note.author.display(),
                    "picture": note.author.picture,
                    "nip05": note.author.nip05
                },
                "content": note.content,
                "created_at": note.created_at,
                "formatted_time": format_timestamp(note.created_at)
            })
        }).collect();

        Ok(json!({
            "success": true,
            "query": query,
            "count": notes.len(),
            "notes": formatted_notes
        }))
    }

    /// Gets a user's profile.
    async fn get_profile(&self, arguments: Value) -> Result<Value> {
        // Support both "pubkey" (new) and "npub" (legacy) parameter names
        let pubkey = arguments
            .get("pubkey")
            .or_else(|| arguments.get("npub"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Missing required argument: pubkey"))?;

        if pubkey.is_empty() {
            return Err(anyhow!("pubkey cannot be empty"));
        }

        debug!("Fetching profile for: {}", pubkey);

        let profile = self.client.get_profile(pubkey).await?;

        Ok(json!({
            "success": true,
            "profile": profile
        }))
    }
}

/// Format a Unix timestamp to a human-readable relative time.
fn format_timestamp(timestamp: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let diff = now.saturating_sub(timestamp);

    if diff < 60 {
        "just now".to_string()
    } else if diff < 3600 {
        let mins = diff / 60;
        format!("{}m ago", mins)
    } else if diff < 86400 {
        let hours = diff / 3600;
        format!("{}h ago", hours)
    } else if diff < 604800 {
        let days = diff / 86400;
        format!("{}d ago", days)
    } else {
        // Format as date for older posts
        let datetime = chrono::DateTime::from_timestamp(timestamp as i64, 0)
            .map(|dt| dt.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| timestamp.to_string());
        datetime
    }
}
