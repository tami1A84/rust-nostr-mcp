//! MCP Apps UI リソース管理モジュール
//!
//! MCP Apps (SEP-1865) 仕様に基づき、`ui://` スキームの UI リソースを管理します。
//! ホスト（Goose、Claude Desktop 等）が MCP Apps 拡張をサポートしている場合、
//! ツール実行結果をサンドボックス化された iframe 内のリッチ UI で表示します。

use serde_json::{json, Value};

use crate::ui_templates;

/// MCP Apps の URI プレフィックス
const UI_URI_PREFIX: &str = "ui://nostr-mcp/";

/// MCP Apps MIME タイプ
const MCP_APP_MIME_TYPE: &str = "text/html;profile=mcp-app";

/// UI リソース定義
struct UiResourceDef {
    /// リソース名（URI のパス部分）
    name: &'static str,
    /// CSP: 外部接続を許可するドメイン
    connect_domains: &'static [&'static str],
    /// CSP: 外部リソース読み込みを許可するドメイン
    resource_domains: &'static [&'static str],
}

/// 全 UI リソースの定義
const UI_RESOURCES: &[UiResourceDef] = &[
    UiResourceDef {
        name: "note-card",
        connect_domains: &[],
        resource_domains: &["*"], // プロフィール画像・メディア読み込み
    },
    UiResourceDef {
        name: "article-card",
        connect_domains: &[],
        resource_domains: &["*"], // 記事内の画像読み込み
    },
    UiResourceDef {
        name: "profile-card",
        connect_domains: &[],
        resource_domains: &["*"], // アバター・バナー画像
    },
    UiResourceDef {
        name: "zap-button",
        connect_domains: &[],
        resource_domains: &[],
    },
    UiResourceDef {
        name: "connect-qr",
        connect_domains: &[],
        resource_domains: &[], // QR は Base64 データ URI で埋め込み
    },
];

/// ツール名から対応する UI リソース URI へのマッピング
struct ToolUiMapping {
    tool_name: &'static str,
    resource_name: &'static str,
    visibility: &'static [&'static str],
}

/// ツールと UI リソースのマッピング定義
const TOOL_UI_MAPPINGS: &[ToolUiMapping] = &[
    ToolUiMapping {
        tool_name: "get_nostr_timeline",
        resource_name: "note-card",
        visibility: &["model", "app"],
    },
    ToolUiMapping {
        tool_name: "search_nostr_notes",
        resource_name: "note-card",
        visibility: &["model", "app"],
    },
    ToolUiMapping {
        tool_name: "get_nostr_thread",
        resource_name: "note-card",
        visibility: &["model", "app"],
    },
    ToolUiMapping {
        tool_name: "get_nostr_articles",
        resource_name: "article-card",
        visibility: &["model", "app"],
    },
    ToolUiMapping {
        tool_name: "get_nostr_drafts",
        resource_name: "article-card",
        visibility: &["model", "app"],
    },
    ToolUiMapping {
        tool_name: "get_nostr_profile",
        resource_name: "profile-card",
        visibility: &["model", "app"],
    },
    ToolUiMapping {
        tool_name: "send_zap",
        resource_name: "zap-button",
        visibility: &["model", "app"],
    },
    ToolUiMapping {
        tool_name: "get_zap_receipts",
        resource_name: "zap-button",
        visibility: &["model", "app"],
    },
    // Phase 6: NIP-46 Nostr Connect
    ToolUiMapping {
        tool_name: "nostr_connect",
        resource_name: "connect-qr",
        visibility: &["model", "app"],
    },
    ToolUiMapping {
        tool_name: "nostr_connect_status",
        resource_name: "connect-qr",
        visibility: &["model", "app"],
    },
];

/// クライアントが MCP Apps 拡張をサポートしているかチェックする。
/// `initialize` リクエストの `params.capabilities.extensions` に
/// `io.modelcontextprotocol/ui` が含まれているか確認する。
///
/// 注: 現在は `handle_initialize` で `ui_enabled` を強制的に `true` にしているため
/// この関数は直接使用されていませんが、テストおよび将来のクライアント判定用に保持しています。
#[allow(dead_code)]
pub fn client_supports_ui(init_params: &Value) -> bool {
    init_params
        .get("capabilities")
        .and_then(|c| c.get("extensions"))
        .and_then(|e| e.get("io.modelcontextprotocol/ui"))
        .is_some()
}

/// `resources/list` レスポンスに含める UI リソースの一覧を返す。
pub fn get_ui_resources() -> Vec<Value> {
    UI_RESOURCES
        .iter()
        .map(|res| {
            json!({
                "uri": format!("{}{}", UI_URI_PREFIX, res.name),
                "name": ui_templates::get_template_display_name(res.name),
                "description": ui_templates::get_template_description(res.name),
                "mimeType": MCP_APP_MIME_TYPE
            })
        })
        .collect()
}

/// `resources/read` で `ui://` リソースの HTML コンテンツを返す。
/// URI が不明な場合は `None` を返す。
pub fn read_ui_resource(uri: &str) -> Option<Value> {
    let name = uri.strip_prefix(UI_URI_PREFIX)?;

    // リソース定義を検索
    let res_def = UI_RESOURCES.iter().find(|r| r.name == name)?;

    // テンプレート HTML を取得（CSS 注入済み）
    let html = ui_templates::get_template(name)?;

    // CSP メタデータを構築
    let mut csp = json!({});
    if !res_def.connect_domains.is_empty() {
        csp["connectDomains"] = json!(res_def.connect_domains);
    }
    if !res_def.resource_domains.is_empty() {
        csp["resourceDomains"] = json!(res_def.resource_domains);
    }

    Some(json!({
        "contents": [{
            "uri": uri,
            "mimeType": MCP_APP_MIME_TYPE,
            "text": html,
            "_meta": {
                "ui": {
                    "csp": csp,
                    "prefersBorder": true
                }
            }
        }]
    }))
}

/// ツール定義に追加する `_meta` フィールドを生成する。
/// MCP Apps 非対応のクライアント向けには `None` を返す。
pub fn get_tool_ui_meta(tool_name: &str) -> Option<Value> {
    TOOL_UI_MAPPINGS
        .iter()
        .find(|m| m.tool_name == tool_name)
        .map(|m| {
            json!({
                "ui": {
                    "resourceUri": format!("{}{}", UI_URI_PREFIX, m.resource_name),
                    "visibility": m.visibility
                }
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_ui_resources() {
        let resources = get_ui_resources();
        assert_eq!(resources.len(), UI_RESOURCES.len());
        for res in &resources {
            assert!(res["uri"].as_str().unwrap().starts_with("ui://nostr-mcp/"));
            assert_eq!(res["mimeType"], MCP_APP_MIME_TYPE);
        }
    }

    #[test]
    fn test_read_ui_resource_known() {
        let result = read_ui_resource("ui://nostr-mcp/note-card");
        assert!(result.is_some());
        let val = result.unwrap();
        let contents = val["contents"].as_array().unwrap();
        assert_eq!(contents.len(), 1);
        assert_eq!(contents[0]["mimeType"], MCP_APP_MIME_TYPE);
        assert!(contents[0]["text"].as_str().unwrap().contains("<!DOCTYPE html>"));
    }

    #[test]
    fn test_read_ui_resource_unknown() {
        assert!(read_ui_resource("ui://nostr-mcp/nonexistent").is_none());
        assert!(read_ui_resource("https://example.com").is_none());
        assert!(read_ui_resource("").is_none());
    }

    #[test]
    fn test_get_tool_ui_meta() {
        let meta = get_tool_ui_meta("get_nostr_timeline");
        assert!(meta.is_some());
        let meta = meta.unwrap();
        assert_eq!(
            meta["ui"]["resourceUri"],
            "ui://nostr-mcp/note-card"
        );

        // Unknown tool returns None
        assert!(get_tool_ui_meta("unknown_tool").is_none());
    }

    #[test]
    fn test_client_supports_ui() {
        let with_ui = json!({
            "capabilities": {
                "extensions": {
                    "io.modelcontextprotocol/ui": {
                        "mimeTypes": ["text/html;profile=mcp-app"]
                    }
                }
            }
        });
        assert!(client_supports_ui(&with_ui));

        let without_ui = json!({
            "capabilities": {
                "tools": {}
            }
        });
        assert!(!client_supports_ui(&without_ui));

        let empty = json!({});
        assert!(!client_supports_ui(&empty));
    }
}
