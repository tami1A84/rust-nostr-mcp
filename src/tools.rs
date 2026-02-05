//! MCP Tools Module
//!
//! Defines the available tools that AI agents can use to interact
//! with the Nostr network. Tool names follow the algia convention
//! with `nostr_` prefix for clarity.

use anyhow::{anyhow, Result};
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
            description: "Post a new short text note (Kind 1) to the Nostr network. Requires write access (NSEC environment variable must be set).".to_string(),
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
            description: "Get the latest notes from the Nostr timeline. If authenticated, returns notes from followed users; otherwise, returns the global timeline.".to_string(),
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
            description: "Search for notes on the Nostr network containing the specified keywords using NIP-50 search-capable relays.".to_string(),
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
            description: "Get the profile information for a Nostr user by their public key (npub or hex format).".to_string(),
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
            "message": format!("Note posted successfully with event ID: {}", event_id.to_hex())
        }))
    }

    /// Gets the timeline.
    async fn get_timeline(&self, arguments: Value) -> Result<Value> {
        let limit = arguments
            .get("limit")
            .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
            .unwrap_or(20)
            .min(100);

        debug!("Fetching timeline with limit: {}", limit);

        let notes = self.client.get_timeline(limit).await?;

        Ok(json!({
            "success": true,
            "count": notes.len(),
            "notes": notes
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

        Ok(json!({
            "success": true,
            "query": query,
            "count": notes.len(),
            "notes": notes
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
