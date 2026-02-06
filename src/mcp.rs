//! MCP サーバーモジュール
//!
//! JSON-RPC over stdio を使用した Model Context Protocol (MCP) サーバーの実装です。
//! Claude などの AI エージェントが Nostr ネットワークと通信できるようにします。
//!
//! MCP Apps (SEP-1865) 拡張をサポートし、ツール実行結果を
//! リッチ UI で表示するための `ui://` リソースを提供します。

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::config::AuthMode;
use crate::mcp_apps;
use crate::nip46::{Nip46Config, Nip46Session};
use crate::nostr_client::{NostrClient, NostrClientConfig};
use crate::tools::{get_tool_definitions, ToolExecutor};

/// MCP プロトコルバージョン
const MCP_VERSION: &str = "2024-11-05";

/// サーバー情報
const SERVER_NAME: &str = "nostr-mcp-server";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// JSON-RPC リクエスト構造体
#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Value,
}

/// JSON-RPC レスポンス構造体
#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

/// JSON-RPC エラー構造体
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

/// MCP サーバーの実装
pub struct McpServer {
    /// Nostr クライアント
    client: Arc<NostrClient>,
    /// ツールエグゼキュータ
    tool_executor: ToolExecutor,
    /// サーバーが初期化済みかどうか
    initialized: bool,
    /// クライアントが MCP Apps UI 拡張をサポートしているか
    ui_enabled: bool,
    /// NIP-46 セッション（Phase 6）
    /// McpServer が nip46_session の所有権を保持（ToolExecutor と共有）
    #[allow(dead_code)]
    nip46_session: Arc<Nip46Session>,
}

impl McpServer {
    /// 指定された設定で新しい MCP サーバーを作成します。
    pub async fn new(config: NostrClientConfig) -> Result<Self> {
        // NIP-46 セッションを構築
        let nip46_config = config.nip46_config.clone().unwrap_or(Nip46Config {
            relays: vec![],
            perms: None,
            bunker_uri: None,
        });
        let nip46_session = Arc::new(Nip46Session::new(nip46_config));

        // バンカー方式の場合は起動時に自動接続
        if config.auth_mode == AuthMode::Bunker {
            if let Some(ref nip46_cfg) = config.nip46_config {
                if let Some(ref bunker_uri) = nip46_cfg.bunker_uri {
                    info!("NIP-46 バンカー方式で自動接続を開始...");
                    if let Err(e) = nip46_session.start_bunker_connect(bunker_uri).await {
                        warn!("NIP-46 バンカー接続に失敗: {}。ローカルモードにフォールバックします。", e);
                    }
                }
            }
        }

        let client = Arc::new(NostrClient::new(config).await?);
        let tool_executor = ToolExecutor::new(Arc::clone(&client), Arc::clone(&nip46_session));

        Ok(Self {
            client,
            tool_executor,
            initialized: false,
            ui_enabled: false,
            nip46_session,
        })
    }

    /// MCP サーバーを実行し、stdin からリクエストを処理して stdout にレスポンスを書き込みます。
    pub async fn run(mut self) -> Result<()> {
        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout();

        info!("MCP サーバー準備完了。リクエストを待機中...");

        for line in stdin.lock().lines() {
            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    error!("stdin からの読み取りエラー: {}", e);
                    break;
                }
            };

            if line.is_empty() {
                continue;
            }

            debug!("リクエスト受信: {}", line);

            let response = self.handle_request(&line).await;

            if let Some(response) = response {
                let response_str = serde_json::to_string(&response)
                    .context("レスポンスのシリアライズに失敗しました")?;

                debug!("レスポンス送信: {}", response_str);

                writeln!(stdout, "{}", response_str)?;
                stdout.flush()?;
            }
        }

        // クリーンアップ
        self.client.disconnect().await;
        info!("MCP サーバーをシャットダウンします");

        Ok(())
    }

    /// 単一の JSON-RPC リクエストを処理します。
    async fn handle_request(&mut self, request_str: &str) -> Option<JsonRpcResponse> {
        let request: JsonRpcRequest = match serde_json::from_str(request_str) {
            Ok(r) => r,
            Err(e) => {
                error!("リクエストのパースに失敗: {}", e);
                return Some(JsonRpcResponse::error(
                    Value::Null,
                    -32700,
                    format!("パースエラー: {}", e),
                ));
            }
        };

        let id = request.id.clone().unwrap_or(Value::Null);

        if request.jsonrpc != "2.0" {
            return Some(JsonRpcResponse::error(
                id,
                -32600,
                "無効な JSON-RPC バージョンです".to_string(),
            ));
        }

        let result = self.dispatch_method(&request.method, request.params).await;

        match result {
            Ok(value) => {
                // 通知（id なし）にはレスポンスを返さない
                if request.id.is_none() {
                    None
                } else {
                    Some(JsonRpcResponse::success(id, value))
                }
            }
            Err(e) => Some(JsonRpcResponse::error(id, -32603, e.to_string())),
        }
    }

    /// メソッド呼び出しを適切なハンドラにディスパッチします。
    async fn dispatch_method(&mut self, method: &str, params: Value) -> Result<Value> {
        match method {
            // コア MCP メソッド
            "initialize" => self.handle_initialize(params),
            "initialized" | "notifications/initialized" => self.handle_initialized(),

            // ツール
            "tools/list" => self.handle_tools_list(),
            "tools/call" => self.handle_tools_call(params).await,

            // リソース
            "resources/list" => self.handle_resources_list(),
            "resources/read" => self.handle_resources_read(params),
            "resources/templates/list" => self.handle_resources_templates_list(),

            // プロンプト（一部クライアントで必要）
            "prompts/list" => self.handle_prompts_list(),

            // ユーティリティ
            "ping" => Ok(json!({})),

            _ => {
                info!("不明なメソッドが要求されました: {}", method);
                Err(anyhow::anyhow!("メソッドが見つかりません: {}", method))
            }
        }
    }

    /// initialize リクエストを処理。
    /// クライアントが MCP Apps 拡張（`io.modelcontextprotocol/ui`）を
    /// サポートしている場合、UI リソースを有効化します。
    fn handle_initialize(&mut self, params: Value) -> Result<Value> {
        info!("initialize リクエストを処理中");

        self.initialized = true;

        // クライアントの MCP Apps サポートを検出
        self.ui_enabled = mcp_apps::client_supports_ui(&params);
        if self.ui_enabled {
            info!("クライアントは MCP Apps UI 拡張をサポートしています");
        } else {
            debug!("クライアントは MCP Apps UI 拡張をサポートしていません");
        }

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

    /// initialized 通知を処理
    fn handle_initialized(&self) -> Result<Value> {
        info!("クライアントが初期化されました");
        Ok(json!({}))
    }

    /// tools/list リクエストを処理。
    /// MCP Apps 対応クライアントには `_meta.ui` 付きのツール定義を返します。
    fn handle_tools_list(&self) -> Result<Value> {
        info!("tools/list リクエストを処理中 (ui_enabled={})", self.ui_enabled);

        let tools = get_tool_definitions(self.ui_enabled);

        Ok(json!({
            "tools": tools
        }))
    }

    /// resources/list リクエストを処理。
    /// MCP Apps 対応クライアントには UI リソースを返します。
    fn handle_resources_list(&self) -> Result<Value> {
        debug!("resources/list リクエストを処理中");

        if self.ui_enabled {
            let resources = mcp_apps::get_ui_resources();
            info!("UI リソース {} 件を返却", resources.len());
            Ok(json!({
                "resources": resources
            }))
        } else {
            Ok(json!({
                "resources": []
            }))
        }
    }

    /// resources/read リクエストを処理。
    /// `ui://` スキームの URI に対してテンプレート HTML を返します。
    fn handle_resources_read(&self, params: Value) -> Result<Value> {
        let uri = params
            .get("uri")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("URI が指定されていません"))?;

        debug!("resources/read リクエストを処理中: {}", uri);

        // ui:// スキームの場合は MCP Apps リソースとして処理
        if uri.starts_with("ui://") {
            match mcp_apps::read_ui_resource(uri) {
                Some(result) => {
                    info!("UI リソースを返却: {}", uri);
                    Ok(result)
                }
                None => {
                    Err(anyhow::anyhow!("UI リソースが見つかりません: {}", uri))
                }
            }
        } else {
            Err(anyhow::anyhow!("リソースが見つかりません: {}", uri))
        }
    }

    /// resources/templates/list リクエストを処理（空のリストを返す）
    fn handle_resources_templates_list(&self) -> Result<Value> {
        debug!("resources/templates/list リクエストを処理中");
        Ok(json!({
            "resourceTemplates": []
        }))
    }

    /// prompts/list リクエストを処理（空のリストを返す）
    fn handle_prompts_list(&self) -> Result<Value> {
        debug!("prompts/list リクエストを処理中");
        Ok(json!({
            "prompts": []
        }))
    }

    /// tools/call リクエストを処理
    async fn handle_tools_call(&self, params: Value) -> Result<Value> {
        let name = params
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("ツール名が指定されていません"))?;

        let arguments = params
            .get("arguments")
            .cloned()
            .unwrap_or(json!({}));

        info!("tools/call リクエストを処理中。ツール: {}", name);

        match self.tool_executor.execute(name, arguments).await {
            Ok(result) => {
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
                error!("ツール実行エラー: {}", e);
                Ok(json!({
                    "content": [
                        {
                            "type": "text",
                            "text": format!("エラー: {}", e)
                        }
                    ],
                    "isError": true
                }))
            }
        }
    }
}
