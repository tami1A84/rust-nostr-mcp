//! MCP ツールモジュール
//!
//! AI エージェントが Nostr ネットワークとやり取りするためのツールを定義します。
//! ツール名は algia の規則に従い `nostr_` プレフィックスを使用します。
//!
//! セキュリティ: 秘密鍵はローカル設定ファイル
//! (~/.config/rust-nostr-mcp/config.json) に保存され、AI エージェントには渡されません。

use anyhow::{anyhow, Result};
use nostr_sdk::ToBech32;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, info};

use crate::nostr_client::{ArticleParams, NostrClient, NoteInfo, ThreadReply};

/// 取得件数の上限
const MAX_LIMIT: u64 = 100;
/// 取得件数のデフォルト値
const DEFAULT_LIMIT: u64 = 20;

/// MCP ツール定義
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// ツール名
    pub name: String,
    /// ツールの説明
    pub description: String,
    /// 入力パラメータの JSON Schema
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

/// limit パラメータを抽出するヘルパー
fn extract_limit(arguments: &Value) -> u64 {
    arguments
        .get("limit")
        .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
        .unwrap_or(DEFAULT_LIMIT)
        .min(MAX_LIMIT)
}

/// ノートを JSON 表示形式にフォーマットするヘルパー
fn format_note_json(note: &NoteInfo) -> Value {
    let mut result = json!({
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
    });

    if let Some(reactions) = note.reactions {
        result["reactions"] = json!(reactions);
    }
    if let Some(replies) = note.replies {
        result["replies"] = json!(replies);
    }

    result
}

/// 利用可能なツールのリストを返します。
pub fn get_tool_definitions() -> Vec<ToolDefinition> {
    vec![
        // 既存ツール
        ToolDefinition {
            name: "post_nostr_note".to_string(),
            description: "Nostr ネットワークにショートテキストノート (Kind 1) を投稿します。書き込みアクセスが必要です（~/.config/rust-nostr-mcp/config.json に秘密鍵を設定）。".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "投稿するノートのテキスト内容"
                    }
                },
                "required": ["content"]
            }),
        },
        ToolDefinition {
            name: "get_nostr_timeline".to_string(),
            description: "Nostr タイムラインから最新のノートを著者情報付きで取得します。認証済みの場合はフォロー中のユーザーのノート、それ以外はグローバルタイムラインを返します。".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "number",
                        "description": "取得するノートの最大数（デフォルト: 20、最大: 100）"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "search_nostr_notes".to_string(),
            description: "NIP-50 検索対応リレーを使用して、指定キーワードを含むノートを検索します。著者情報付きで結果を返します。".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "検索クエリ文字列"
                    },
                    "limit": {
                        "type": "number",
                        "description": "結果の最大数（デフォルト: 20、最大: 100）"
                    }
                },
                "required": ["query"]
            }),
        },
        ToolDefinition {
            name: "get_nostr_profile".to_string(),
            description: "公開鍵（npub または hex 形式）で Nostr ユーザーのプロフィール情報を取得します。name、display_name、about、picture、banner、nip05、lud16、website を返します。".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pubkey": {
                        "type": "string",
                        "description": "npub (bech32) または hex 形式の公開鍵"
                    }
                },
                "required": ["pubkey"]
            }),
        },
        // Phase 1: NIP-23 長文コンテンツツール
        ToolDefinition {
            name: "post_nostr_article".to_string(),
            description: "Nostr ネットワークに長文記事 (Kind 30023, NIP-23) を投稿します。Markdown コンテンツをサポートします。書き込みアクセスが必要です。".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "記事のタイトル"
                    },
                    "content": {
                        "type": "string",
                        "description": "Markdown 形式の記事本文"
                    },
                    "summary": {
                        "type": "string",
                        "description": "記事の要約（任意）"
                    },
                    "image": {
                        "type": "string",
                        "description": "ヘッダー画像の URL（任意）"
                    },
                    "tags": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "トピックハッシュタグ（任意）"
                    },
                    "published_at": {
                        "type": "number",
                        "description": "公開日時の Unix タイムスタンプ（任意、未指定時は現在時刻）"
                    },
                    "identifier": {
                        "type": "string",
                        "description": "記事の識別子（d タグ、任意。未指定時はタイトルから自動生成）"
                    }
                },
                "required": ["title", "content"]
            }),
        },
        ToolDefinition {
            name: "get_nostr_articles".to_string(),
            description: "Nostr ネットワークから長文記事 (Kind 30023, NIP-23) を取得します。著者やハッシュタグでフィルタリングできます。".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "author": {
                        "type": "string",
                        "description": "著者の公開鍵でフィルタ（npub または hex 形式、任意）"
                    },
                    "tags": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "ハッシュタグでフィルタ（任意）"
                    },
                    "limit": {
                        "type": "number",
                        "description": "取得する記事の最大数（デフォルト: 20、最大: 100）"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "save_nostr_draft".to_string(),
            description: "記事を下書き (Kind 30024) として Nostr に保存します。後で編集・公開できます。書き込みアクセスが必要です。".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "title": {
                        "type": "string",
                        "description": "記事のタイトル"
                    },
                    "content": {
                        "type": "string",
                        "description": "Markdown 形式の記事本文"
                    },
                    "summary": {
                        "type": "string",
                        "description": "記事の要約（任意）"
                    },
                    "image": {
                        "type": "string",
                        "description": "ヘッダー画像の URL（任意）"
                    },
                    "tags": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "トピックハッシュタグ（任意）"
                    },
                    "identifier": {
                        "type": "string",
                        "description": "記事の識別子（d タグ、任意。未指定時はタイトルから自動生成）"
                    }
                },
                "required": ["title", "content"]
            }),
        },
        ToolDefinition {
            name: "get_nostr_drafts".to_string(),
            description: "自分の下書き記事 (Kind 30024) を取得します。認証が必要です（秘密鍵が設定されている必要があります）。".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "limit": {
                        "type": "number",
                        "description": "取得する下書きの最大数（デフォルト: 20、最大: 100）"
                    }
                }
            }),
        },
        // Phase 2: タイムライン拡張機能
        ToolDefinition {
            name: "get_nostr_thread".to_string(),
            description: "ノートのスレッド（リプライツリー）を取得します。指定したノートとそのリプライを階層構造で返します。".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "note_id": {
                        "type": "string",
                        "description": "対象ノートのイベント ID（hex、nevent、note 形式対応）"
                    },
                    "depth": {
                        "type": "number",
                        "description": "取得するリプライの深さ（デフォルト: 3、最大: 10）"
                    }
                },
                "required": ["note_id"]
            }),
        },
        ToolDefinition {
            name: "react_to_note".to_string(),
            description: "ノートにリアクション (Kind 7, NIP-25) を送信します。デフォルトは「+」（いいね）です。書き込みアクセスが必要です。".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "note_id": {
                        "type": "string",
                        "description": "リアクション対象のイベント ID（hex、nevent、note 形式対応）"
                    },
                    "reaction": {
                        "type": "string",
                        "description": "リアクション文字（デフォルト: \"+\"、絵文字も可）"
                    }
                },
                "required": ["note_id"]
            }),
        },
        ToolDefinition {
            name: "reply_to_note".to_string(),
            description: "既存のノートに返信を投稿します（NIP-10 スレッディング対応）。書き込みアクセスが必要です。".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "note_id": {
                        "type": "string",
                        "description": "返信先のイベント ID（hex、nevent、note 形式対応）"
                    },
                    "content": {
                        "type": "string",
                        "description": "返信のテキスト内容"
                    }
                },
                "required": ["note_id", "content"]
            }),
        },
        ToolDefinition {
            name: "get_nostr_notifications".to_string(),
            description: "自分のノートへのメンションやリアクションを取得します。認証が必要です。".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "since": {
                        "type": "number",
                        "description": "この Unix タイムスタンプ以降の通知のみ取得（任意）"
                    },
                    "limit": {
                        "type": "number",
                        "description": "取得する通知の最大数（デフォルト: 20、最大: 100）"
                    }
                }
            }),
        },
    ]
}

/// ツール呼び出しを処理するエグゼキュータ
pub struct ToolExecutor {
    /// Nostr クライアントインスタンス
    client: Arc<NostrClient>,
}

impl ToolExecutor {
    /// 新しいツールエグゼキュータを作成
    pub fn new(client: Arc<NostrClient>) -> Self {
        Self { client }
    }

    /// 指定されたツールを引数付きで実行します。
    pub async fn execute(&self, name: &str, arguments: Value) -> Result<Value> {
        info!("ツール実行: {} 引数: {}", name, arguments);

        match name {
            "post_nostr_note" => self.post_note(arguments).await,
            "get_nostr_timeline" => self.get_timeline(arguments).await,
            "search_nostr_notes" => self.search_notes(arguments).await,
            "get_nostr_profile" => self.get_profile(arguments).await,
            // Phase 1: NIP-23 長文コンテンツ
            "post_nostr_article" => self.post_article(arguments).await,
            "get_nostr_articles" => self.get_articles(arguments).await,
            "save_nostr_draft" => self.save_draft(arguments).await,
            "get_nostr_drafts" => self.get_drafts(arguments).await,
            // Phase 2: タイムライン拡張機能
            "get_nostr_thread" => self.get_thread(arguments).await,
            "react_to_note" => self.react_to_note(arguments).await,
            "reply_to_note" => self.reply_to_note(arguments).await,
            "get_nostr_notifications" => self.get_notifications(arguments).await,
            _ => Err(anyhow!("不明なツール: {}", name)),
        }
    }

    /// 新しいノートを投稿
    async fn post_note(&self, arguments: Value) -> Result<Value> {
        let content = arguments
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("必須パラメータが不足: content"))?;

        if content.is_empty() {
            return Err(anyhow!("content は空にできません"));
        }

        let event_id = self.client.post_note(content).await?;

        Ok(json!({
            "success": true,
            "event_id": event_id.to_hex(),
            "nevent": event_id.to_bech32().unwrap_or_default(),
            "message": format!("ノートを投稿しました。イベント ID: {}", event_id.to_hex())
        }))
    }

    /// タイムラインを取得
    async fn get_timeline(&self, arguments: Value) -> Result<Value> {
        let limit = extract_limit(&arguments);
        debug!("タイムライン取得: limit={}", limit);

        let notes = self.client.get_timeline(limit).await?;
        let formatted_notes: Vec<Value> = notes.iter().map(format_note_json).collect();

        Ok(json!({
            "success": true,
            "count": notes.len(),
            "notes": formatted_notes
        }))
    }

    /// ノートを検索
    async fn search_notes(&self, arguments: Value) -> Result<Value> {
        let query = arguments
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("必須パラメータが不足: query"))?;

        if query.is_empty() {
            return Err(anyhow!("query は空にできません"));
        }

        let limit = extract_limit(&arguments);
        debug!("ノート検索: query='{}', limit={}", query, limit);

        let notes = self.client.search_notes(query, limit).await?;
        let formatted_notes: Vec<Value> = notes.iter().map(format_note_json).collect();

        Ok(json!({
            "success": true,
            "query": query,
            "count": notes.len(),
            "notes": formatted_notes
        }))
    }

    /// プロフィールを取得
    async fn get_profile(&self, arguments: Value) -> Result<Value> {
        let pubkey = arguments
            .get("pubkey")
            .or_else(|| arguments.get("npub"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("必須パラメータが不足: pubkey"))?;

        if pubkey.is_empty() {
            return Err(anyhow!("pubkey は空にできません"));
        }

        debug!("プロフィール取得: {}", pubkey);

        let profile = self.client.get_profile(pubkey).await?;

        Ok(json!({
            "success": true,
            "profile": profile
        }))
    }

    // ========================================
    // Phase 1: NIP-23 長文コンテンツツール
    // ========================================

    /// 長文記事を投稿
    async fn post_article(&self, arguments: Value) -> Result<Value> {
        let title = arguments
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("必須パラメータが不足: title"))?
            .to_string();

        let content = arguments
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("必須パラメータが不足: content"))?
            .to_string();

        if title.is_empty() {
            return Err(anyhow!("title は空にできません"));
        }
        if content.is_empty() {
            return Err(anyhow!("content は空にできません"));
        }

        let params = ArticleParams {
            title,
            content,
            identifier: arguments.get("identifier").and_then(|v| v.as_str()).map(String::from),
            summary: arguments.get("summary").and_then(|v| v.as_str()).map(String::from),
            image: arguments.get("image").and_then(|v| v.as_str()).map(String::from),
            tags: arguments.get("tags").and_then(|v| {
                v.as_array().map(|arr| {
                    arr.iter().filter_map(|item| item.as_str().map(String::from)).collect()
                })
            }),
            published_at: arguments.get("published_at").and_then(|v| v.as_u64()),
        };

        let article = self.client.post_article(params).await?;

        Ok(json!({
            "success": true,
            "event_id": article.id,
            "nevent": article.nevent,
            "naddr": article.naddr,
            "identifier": article.identifier,
            "title": article.title,
            "message": format!("記事「{}」を投稿しました。", article.title)
        }))
    }

    /// 長文記事を取得
    async fn get_articles(&self, arguments: Value) -> Result<Value> {
        let author = arguments.get("author").and_then(|v| v.as_str());
        let tags: Option<Vec<String>> = arguments.get("tags").and_then(|v| {
            v.as_array().map(|arr| {
                arr.iter().filter_map(|item| item.as_str().map(String::from)).collect()
            })
        });
        let limit = extract_limit(&arguments);

        debug!("記事取得: author={:?}, tags={:?}, limit={}", author, tags, limit);

        let articles = self.client.get_articles(
            author,
            tags.as_deref(),
            limit,
        ).await?;

        let formatted: Vec<Value> = articles.iter().map(|article| {
            json!({
                "id": article.id,
                "nevent": article.nevent,
                "naddr": article.naddr,
                "identifier": article.identifier,
                "title": article.title,
                "summary": article.summary,
                "image": article.image,
                "content": article.content,
                "author": article.author,
                "published_at": article.published_at,
                "created_at": article.created_at,
                "formatted_time": format_timestamp(article.created_at),
                "tags": article.tags,
                "is_draft": article.is_draft
            })
        }).collect();

        Ok(json!({
            "success": true,
            "count": articles.len(),
            "articles": formatted
        }))
    }

    /// 下書きを保存
    async fn save_draft(&self, arguments: Value) -> Result<Value> {
        let title = arguments
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("必須パラメータが不足: title"))?
            .to_string();

        let content = arguments
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("必須パラメータが不足: content"))?
            .to_string();

        if title.is_empty() {
            return Err(anyhow!("title は空にできません"));
        }
        if content.is_empty() {
            return Err(anyhow!("content は空にできません"));
        }

        let params = ArticleParams {
            title,
            content,
            identifier: arguments.get("identifier").and_then(|v| v.as_str()).map(String::from),
            summary: arguments.get("summary").and_then(|v| v.as_str()).map(String::from),
            image: arguments.get("image").and_then(|v| v.as_str()).map(String::from),
            tags: arguments.get("tags").and_then(|v| {
                v.as_array().map(|arr| {
                    arr.iter().filter_map(|item| item.as_str().map(String::from)).collect()
                })
            }),
            published_at: None,
        };

        let article = self.client.save_draft(params).await?;

        Ok(json!({
            "success": true,
            "event_id": article.id,
            "nevent": article.nevent,
            "naddr": article.naddr,
            "identifier": article.identifier,
            "title": article.title,
            "is_draft": true,
            "message": format!("下書き「{}」を保存しました。", article.title)
        }))
    }

    // ========================================
    // Phase 2: タイムライン拡張機能ツール
    // ========================================

    /// スレッドを取得
    async fn get_thread(&self, arguments: Value) -> Result<Value> {
        let note_id = arguments
            .get("note_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("必須パラメータが不足: note_id"))?;

        if note_id.is_empty() {
            return Err(anyhow!("note_id は空にできません"));
        }

        let depth = arguments
            .get("depth")
            .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
            .unwrap_or(3)
            .min(10);

        debug!("スレッド取得: note_id='{}', depth={}", note_id, depth);

        let thread = self.client.get_thread(note_id, depth).await?;

        let formatted_replies: Vec<Value> = thread.replies.iter()
            .map(|reply| format_thread_reply(reply))
            .collect();

        Ok(json!({
            "success": true,
            "root": format_note_json(&thread.root),
            "replies": formatted_replies,
            "total_replies": thread.total_replies,
            "depth": thread.depth
        }))
    }

    /// リアクションを送信
    async fn react_to_note(&self, arguments: Value) -> Result<Value> {
        let note_id = arguments
            .get("note_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("必須パラメータが不足: note_id"))?;

        if note_id.is_empty() {
            return Err(anyhow!("note_id は空にできません"));
        }

        let reaction = arguments
            .get("reaction")
            .and_then(|v| v.as_str())
            .unwrap_or("+");

        debug!("リアクション送信: note_id='{}', reaction='{}'", note_id, reaction);

        let event_id = self.client.react_to_note(note_id, reaction).await?;

        Ok(json!({
            "success": true,
            "event_id": event_id.to_hex(),
            "nevent": event_id.to_bech32().unwrap_or_default(),
            "reaction": reaction,
            "message": format!("リアクション「{}」を送信しました。", reaction)
        }))
    }

    /// ノートに返信
    async fn reply_to_note(&self, arguments: Value) -> Result<Value> {
        let note_id = arguments
            .get("note_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("必須パラメータが不足: note_id"))?;

        let content = arguments
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("必須パラメータが不足: content"))?;

        if note_id.is_empty() {
            return Err(anyhow!("note_id は空にできません"));
        }
        if content.is_empty() {
            return Err(anyhow!("content は空にできません"));
        }

        debug!("返信投稿: note_id='{}'", note_id);

        let event_id = self.client.reply_to_note(note_id, content).await?;

        Ok(json!({
            "success": true,
            "event_id": event_id.to_hex(),
            "nevent": event_id.to_bech32().unwrap_or_default(),
            "message": "返信を投稿しました。"
        }))
    }

    /// 通知を取得
    async fn get_notifications(&self, arguments: Value) -> Result<Value> {
        let since = arguments
            .get("since")
            .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)));

        let limit = extract_limit(&arguments);
        debug!("通知取得: since={:?}, limit={}", since, limit);

        let notifications = self.client.get_notifications(since, limit).await?;

        let formatted: Vec<Value> = notifications.iter().map(|n| {
            json!({
                "id": n.id,
                "nevent": n.nevent,
                "type": n.notification_type,
                "author": {
                    "pubkey": n.author.pubkey,
                    "npub": n.author.npub,
                    "name": n.author.name,
                    "display_name": n.author.display_name,
                    "display": n.author.display(),
                    "picture": n.author.picture,
                    "nip05": n.author.nip05
                },
                "content": n.content,
                "target_note_id": n.target_note_id,
                "created_at": n.created_at,
                "formatted_time": format_timestamp(n.created_at)
            })
        }).collect();

        Ok(json!({
            "success": true,
            "count": notifications.len(),
            "notifications": formatted
        }))
    }

    /// 下書き一覧を取得
    async fn get_drafts(&self, arguments: Value) -> Result<Value> {
        let limit = extract_limit(&arguments);
        debug!("下書き取得: limit={}", limit);

        let drafts = self.client.get_drafts(limit).await?;

        let formatted: Vec<Value> = drafts.iter().map(|article| {
            json!({
                "id": article.id,
                "nevent": article.nevent,
                "naddr": article.naddr,
                "identifier": article.identifier,
                "title": article.title,
                "summary": article.summary,
                "content": article.content,
                "created_at": article.created_at,
                "formatted_time": format_timestamp(article.created_at),
                "tags": article.tags,
                "is_draft": true
            })
        }).collect();

        Ok(json!({
            "success": true,
            "count": drafts.len(),
            "drafts": formatted
        }))
    }
}

/// スレッドリプライを再帰的に JSON にフォーマット
fn format_thread_reply(reply: &ThreadReply) -> Value {
    let children: Vec<Value> = reply.replies.iter()
        .map(|r| format_thread_reply(r))
        .collect();

    json!({
        "note": format_note_json(&reply.note),
        "replies": children
    })
}

/// Unix タイムスタンプを人間が読める相対時間にフォーマット
fn format_timestamp(timestamp: u64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let diff = now.saturating_sub(timestamp);

    if diff < 60 {
        "たった今".to_string()
    } else if diff < 3600 {
        let mins = diff / 60;
        format!("{}分前", mins)
    } else if diff < 86400 {
        let hours = diff / 3600;
        format!("{}時間前", hours)
    } else if diff < 604800 {
        let days = diff / 86400;
        format!("{}日前", days)
    } else {
        chrono::DateTime::from_timestamp(timestamp as i64, 0)
            .map(|dt| dt.format("%Y-%m-%d").to_string())
            .unwrap_or_else(|| timestamp.to_string())
    }
}
