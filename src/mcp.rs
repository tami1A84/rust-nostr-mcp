//! MCP Server Module
//!
//! Implements the Model Context Protocol (MCP) server using JSON-RPC over stdio.
//! This allows AI agents like Claude to communicate with the Nostr network.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::sync::Arc;
use tracing::{debug, error, info};

use crate::nostr_client::{NostrClient, NostrClientConfig};
use crate::tools::{get_tool_definitions, ToolExecutor};

/// MCP Protocol version.
const MCP_VERSION: &str = "2024-11-05";

/// Server information.
const SERVER_NAME: &str = "nostr-mcp-server";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// JSON-RPC request structure.
#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

/// JSON-RPC response structure.
#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

/// JSON-RPC error structure.
#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

impl JsonRpcResponse {
    fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    fn error(id: Value, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: None,
            }),
        }
    }
}

/// MCP Server implementation.
pub struct McpServer {
    /// The Nostr client
    client: Arc<NostrClient>,
    /// Tool executor
    tool_executor: ToolExecutor,
    /// Whether the server has been initialized
    initialized: bool,
}

impl McpServer {
    /// Creates a new MCP server with the given configuration.
    ///
    /// # Arguments
    /// * `config` - The Nostr client configuration
    ///
    /// # Returns
    /// A new `McpServer` instance.
    pub async fn new(config: NostrClientConfig) -> Result<Self> {
        let client = Arc::new(NostrClient::new(config).await?);
        let tool_executor = ToolExecutor::new(Arc::clone(&client));

        Ok(Self {
            client,
            tool_executor,
            initialized: false,
        })
    }

    /// Runs the MCP server, processing requests from stdin and writing responses to stdout.
    pub async fn run(mut self) -> Result<()> {
        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout();

        info!("MCP server ready, waiting for requests...");

        for line in stdin.lock().lines() {
            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    error!("Error reading from stdin: {}", e);
                    break;
                }
            };

            if line.is_empty() {
                continue;
            }

            debug!("Received request: {}", line);

            let response = self.handle_request(&line).await;

            if let Some(response) = response {
                let response_str = serde_json::to_string(&response)
                    .context("Failed to serialize response")?;

                debug!("Sending response: {}", response_str);

                writeln!(stdout, "{}", response_str)?;
                stdout.flush()?;
            }
        }

        // Cleanup
        self.client.disconnect().await;
        info!("MCP server shutting down");

        Ok(())
    }

    /// Handles a single JSON-RPC request.
    async fn handle_request(&mut self, request_str: &str) -> Option<JsonRpcResponse> {
        let request: JsonRpcRequest = match serde_json::from_str(request_str) {
            Ok(r) => r,
            Err(e) => {
                error!("Failed to parse request: {}", e);
                return Some(JsonRpcResponse::error(
                    Value::Null,
                    -32700,
                    format!("Parse error: {}", e),
                ));
            }
        };

        let id = request.id.clone().unwrap_or(Value::Null);

        // Check JSON-RPC version
        if request.jsonrpc != "2.0" {
            return Some(JsonRpcResponse::error(
                id,
                -32600,
                "Invalid JSON-RPC version".to_string(),
            ));
        }

        // Handle the request based on method
        let result = self.dispatch_method(&request.method, request.params).await;

        match result {
            Ok(value) => {
                // Notifications (no id) don't get responses
                if request.id.is_none() {
                    None
                } else {
                    Some(JsonRpcResponse::success(id, value))
                }
            }
            Err(e) => Some(JsonRpcResponse::error(id, -32603, e.to_string())),
        }
    }

    /// Dispatches a method call to the appropriate handler.
    async fn dispatch_method(&mut self, method: &str, params: Value) -> Result<Value> {
        match method {
            // Core MCP methods
            "initialize" => self.handle_initialize(params),
            "initialized" | "notifications/initialized" => self.handle_initialized(),

            // Tools
            "tools/list" => self.handle_tools_list(),
            "tools/call" => self.handle_tools_call(params).await,

            // Resources (empty but required for some clients)
            "resources/list" => self.handle_resources_list(),
            "resources/templates/list" => self.handle_resources_templates_list(),

            // Prompts (empty but required for some clients)
            "prompts/list" => self.handle_prompts_list(),

            // Utility
            "ping" => Ok(json!({})),

            _ => {
                info!("Unknown method requested: {}", method);
                Err(anyhow::anyhow!("Method not found: {}", method))
            }
        }
    }

    /// Handles the initialize request.
    fn handle_initialize(&mut self, _params: Value) -> Result<Value> {
        info!("Handling initialize request");

        self.initialized = true;

        Ok(json!({
            "protocolVersion": MCP_VERSION,
            "capabilities": {
                "tools": {},
                "resources": {},
                "prompts": {}
            },
            "serverInfo": {
                "name": SERVER_NAME,
                "version": SERVER_VERSION
            }
        }))
    }

    /// Handles the initialized notification.
    fn handle_initialized(&self) -> Result<Value> {
        info!("Client initialized");
        Ok(json!({}))
    }

    /// Handles the tools/list request.
    fn handle_tools_list(&self) -> Result<Value> {
        info!("Handling tools/list request");

        let tools = get_tool_definitions();

        Ok(json!({
            "tools": tools
        }))
    }

    /// Handles the resources/list request.
    /// Returns an empty list as this server doesn't provide resources.
    fn handle_resources_list(&self) -> Result<Value> {
        debug!("Handling resources/list request");
        Ok(json!({
            "resources": []
        }))
    }

    /// Handles the resources/templates/list request.
    /// Returns an empty list as this server doesn't provide resource templates.
    fn handle_resources_templates_list(&self) -> Result<Value> {
        debug!("Handling resources/templates/list request");
        Ok(json!({
            "resourceTemplates": []
        }))
    }

    /// Handles the prompts/list request.
    /// Returns an empty list as this server doesn't provide prompts.
    fn handle_prompts_list(&self) -> Result<Value> {
        debug!("Handling prompts/list request");
        Ok(json!({
            "prompts": []
        }))
    }

    /// Handles the tools/call request.
    async fn handle_tools_call(&self, params: Value) -> Result<Value> {
        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing tool name"))?;

        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or(json!({}));

        info!("Handling tools/call request for tool: {}", name);

        match self.tool_executor.execute(name, arguments).await {
            Ok(result) => {
                // Format the result as MCP tool response
                Ok(json!({
                    "content": [
                        {
                            "type": "text",
                            "text": serde_json::to_string_pretty(&result)?
                        }
                    ]
                }))
            }
            Err(e) => {
                error!("Tool execution error: {}", e);
                Ok(json!({
                    "content": [
                        {
                            "type": "text",
                            "text": format!("Error: {}", e)
                        }
                    ],
                    "isError": true
                }))
            }
        }
    }
}
