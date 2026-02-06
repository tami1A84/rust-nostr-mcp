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

use crate::content;
use crate::mcp_apps;
use crate::nostr_client::{ArticleParams, DirectMessageInfo, NostrClient, NoteInfo, ThreadReply};

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
    /// MCP Apps UI メタデータ（オプション）
    #[serde(rename = "_meta", skip_serializing_if = "Option::is_none")]
    pub meta: Option<Value>,
}

/// limit パラメータを抽出するヘルパー
fn extract_limit(arguments: &Value) -> u64 {
    arguments
        .get("limit")
        .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
        .unwrap_or(DEFAULT_LIMIT)
        .min(MAX_LIMIT)
}

/// ノートを JSON 表示形式にフォーマットするヘルパー（Phase 3: 構造化表示対応）
fn format_note_json(note: &NoteInfo) -> Value {
    let formatted_time = format_timestamp(note.created_at);

    // Phase 3: display_card の構築
    let header = format_display_card_header(&note.author);
    let footer = format_display_card_footer(note.reactions, note.replies, &formatted_time);

    // Phase 3: コンテンツ解析（メディア・ハッシュタグ・Nostr 参照）
    let parsed = content::parse_content(&note.content);

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
        "formatted_time": formatted_time,
        "display_card": {
            "header": header,
            "content": note.content,
            "footer": footer
        }
    });

    if let Some(reactions) = note.reactions {
        result["reactions"] = json!(reactions);
    }
    if let Some(replies) = note.replies {
        result["replies"] = json!(replies);
    }

    // Phase 3: メディア・解析済みコンテンツを追加（空でない場合のみ）
    if !parsed.media.is_empty() {
        result["media"] = json!(parsed.media);
    }
    if !parsed.is_empty() {
        result["parsed_content"] = json!({});
        if !parsed.hashtags.is_empty() {
            result["parsed_content"]["hashtags"] = json!(parsed.hashtags);
        }
        if !parsed.references.is_empty() {
            result["parsed_content"]["references"] = json!(parsed.references);
        }
    }

    result
}

/// display_card のヘッダーを生成（"表示名 (@nip05)" 形式）
fn format_display_card_header(author: &crate::nostr_client::AuthorInfo) -> String {
    let display = author.display();
    if let Some(ref nip05) = author.nip05 {
        format!("{} (@{})", display, nip05)
    } else {
        format!("{} ({})", display, author.short_npub())
    }
}

/// display_card のフッターを生成（"N リアクション · N リプライ · 時間" 形式）
fn format_display_card_footer(reactions: Option<u64>, replies: Option<u64>, formatted_time: &str) -> String {
    let mut parts = Vec::new();

    if let Some(r) = reactions {
        if r > 0 {
            parts.push(format!("{} リアクション", r));
        }
    }
    if let Some(r) = replies {
        if r > 0 {
            parts.push(format!("{} リプライ", r));
        }
    }
    parts.push(formatted_time.to_string());

    parts.join(" · ")
}

/// 利用可能なツールのリストを返します。
/// `ui_enabled` が `true` の場合、MCP Apps UI メタデータを含めます。
pub fn get_tool_definitions(ui_enabled: bool) -> Vec<ToolDefinition> {
    let meta = |name: &str| -> Option<Value> {
        if ui_enabled {
            mcp_apps::get_tool_ui_meta(name)
        } else {
            None
        }
    };

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
            meta: meta("post_nostr_note"),
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
            meta: meta("get_nostr_timeline"),
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
            meta: meta("search_nostr_notes"),
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
            meta: meta("get_nostr_profile"),
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
            meta: meta("post_nostr_article"),
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
            meta: meta("get_nostr_articles"),
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
            meta: meta("save_nostr_draft"),
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
            meta: meta("get_nostr_drafts"),
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
            meta: meta("get_nostr_thread"),
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
            meta: meta("react_to_note"),
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
            meta: meta("reply_to_note"),
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
            meta: meta("get_nostr_notifications"),
        },
        // Phase 4: 高度な機能
        ToolDefinition {
            name: "send_zap".to_string(),
            description: "ノートまたはプロフィールに Lightning Zap (NIP-57) を送信します。NWC (Nostr Wallet Connect) の設定が必要です。書き込みアクセスが必要です。".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "target": {
                        "type": "string",
                        "description": "Zap 対象のイベント ID（hex、nevent、note 形式）または公開鍵（npub または hex 形式）"
                    },
                    "amount": {
                        "type": "number",
                        "description": "sats 単位の金額"
                    },
                    "comment": {
                        "type": "string",
                        "description": "Zap コメント（任意）"
                    }
                },
                "required": ["target", "amount"]
            }),
            meta: meta("send_zap"),
        },
        ToolDefinition {
            name: "get_zap_receipts".to_string(),
            description: "ノートの Zap レシート (Kind 9735, NIP-57) を取得します。送信者・金額・コメント情報付きで返します。".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "note_id": {
                        "type": "string",
                        "description": "対象ノートのイベント ID（hex、nevent、note 形式対応）"
                    },
                    "limit": {
                        "type": "number",
                        "description": "取得するレシートの最大数（デフォルト: 20、最大: 100）"
                    }
                },
                "required": ["note_id"]
            }),
            meta: meta("get_zap_receipts"),
        },
        ToolDefinition {
            name: "send_dm".to_string(),
            description: "暗号化されたダイレクトメッセージ (NIP-04) を送信します。書き込みアクセスが必要です。".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "recipient": {
                        "type": "string",
                        "description": "受信者の公開鍵（npub または hex 形式）"
                    },
                    "content": {
                        "type": "string",
                        "description": "メッセージ内容"
                    }
                },
                "required": ["recipient", "content"]
            }),
            meta: meta("send_dm"),
        },
        ToolDefinition {
            name: "get_dms".to_string(),
            description: "暗号化されたダイレクトメッセージ (NIP-04) の会話を取得・復号します。認証が必要です。".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "with": {
                        "type": "string",
                        "description": "会話相手の公開鍵（npub または hex 形式）でフィルタ（任意）"
                    },
                    "limit": {
                        "type": "number",
                        "description": "取得する最大メッセージ数（デフォルト: 20、最大: 100）"
                    }
                }
            }),
            meta: meta("get_dms"),
        },
        ToolDefinition {
            name: "get_relay_list".to_string(),
            description: "ユーザーのリレーリスト (Kind 10002, NIP-65) を取得します。各リレーの読み書き設定を返します。".to_string(),
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
            meta: meta("get_relay_list"),
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
            // Phase 4: 高度な機能
            "send_zap" => self.send_zap(arguments).await,
            "get_zap_receipts" => self.get_zap_receipts(arguments).await,
            "send_dm" => self.send_dm(arguments).await,
            "get_dms" => self.get_dms(arguments).await,
            "get_relay_list" => self.get_relay_list(arguments).await,
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

    /// プロフィールを取得（Phase 3: プロフィールカード・統計情報付き）
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

        // プロフィールと統計情報を並行取得
        let (profile_result, stats_result) = tokio::join!(
            self.client.get_profile(pubkey),
            self.client.get_profile_stats(pubkey)
        );

        let profile = profile_result?;

        // Phase 3: プロフィールカードの構築
        let display_name = profile.display_name.as_ref()
            .or(profile.name.as_ref())
            .cloned()
            .unwrap_or_else(|| {
                if profile.npub.len() > 16 {
                    format!("{}...{}", &profile.npub[..12], &profile.npub[profile.npub.len()-4..])
                } else {
                    profile.npub.clone()
                }
            });

        let mut profile_card = json!({
            "avatar": profile.picture,
            "name": display_name,
            "nip05": profile.nip05,
            "bio": profile.about
        });

        // 統計情報を追加（取得に成功した場合のみ）
        if let Ok(stats) = stats_result {
            profile_card["stats"] = json!({
                "following": stats.following,
                "followers": stats.followers,
                "notes": stats.notes
            });
        }

        Ok(json!({
            "success": true,
            "profile": profile,
            "profile_card": profile_card
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

    /// 長文記事を取得（Phase 3: コンテンツ解析付き）
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
            format_article_json(article)
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

    /// 下書き一覧を取得（Phase 3: コンテンツ解析付き）
    async fn get_drafts(&self, arguments: Value) -> Result<Value> {
        let limit = extract_limit(&arguments);
        debug!("下書き取得: limit={}", limit);

        let drafts = self.client.get_drafts(limit).await?;

        let formatted: Vec<Value> = drafts.iter().map(|article| {
            format_article_json(article)
        }).collect();

        Ok(json!({
            "success": true,
            "count": drafts.len(),
            "drafts": formatted
        }))
    }

    // ========================================
    // Phase 4: 高度な機能ツール
    // ========================================

    /// Zap を送信
    async fn send_zap(&self, arguments: Value) -> Result<Value> {
        let target = arguments
            .get("target")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("必須パラメータが不足: target"))?;

        let amount = arguments
            .get("amount")
            .and_then(|v| v.as_u64().or_else(|| v.as_f64().map(|f| f as u64)))
            .ok_or_else(|| anyhow!("必須パラメータが不足: amount"))?;

        if target.is_empty() {
            return Err(anyhow!("target は空にできません"));
        }

        if amount == 0 {
            return Err(anyhow!("amount は 0 より大きくなければなりません"));
        }

        let comment = arguments
            .get("comment")
            .and_then(|v| v.as_str());

        debug!("Zap 送信: target='{}', amount={}, comment={:?}", target, amount, comment);

        self.client.send_zap(target, amount, comment).await
    }

    /// Zap レシートを取得
    async fn get_zap_receipts(&self, arguments: Value) -> Result<Value> {
        let note_id = arguments
            .get("note_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("必須パラメータが不足: note_id"))?;

        if note_id.is_empty() {
            return Err(anyhow!("note_id は空にできません"));
        }

        let limit = extract_limit(&arguments);
        debug!("Zap レシート取得: note_id='{}', limit={}", note_id, limit);

        let receipts = self.client.get_zap_receipts(note_id, limit).await?;

        let total_sats: u64 = receipts.iter().map(|r| r.amount_sats).sum();

        let formatted: Vec<Value> = receipts.iter().map(|receipt| {
            let mut result = json!({
                "id": receipt.id,
                "nevent": receipt.nevent,
                "amount_sats": receipt.amount_sats,
                "created_at": receipt.created_at,
                "formatted_time": format_timestamp(receipt.created_at)
            });

            if let Some(ref sender) = receipt.sender {
                result["sender"] = json!({
                    "pubkey": sender.pubkey,
                    "npub": sender.npub,
                    "name": sender.name,
                    "display_name": sender.display_name,
                    "display": sender.display(),
                    "picture": sender.picture,
                    "nip05": sender.nip05
                });
            }

            if let Some(ref comment) = receipt.comment {
                result["comment"] = json!(comment);
            }

            if let Some(ref target_note_id) = receipt.target_note_id {
                result["target_note_id"] = json!(target_note_id);
            }

            if let Some(ref target_pubkey) = receipt.target_pubkey {
                result["target_pubkey"] = json!(target_pubkey);
            }

            result
        }).collect();

        Ok(json!({
            "success": true,
            "count": receipts.len(),
            "total_sats": total_sats,
            "zap_receipts": formatted
        }))
    }

    /// ダイレクトメッセージを送信
    async fn send_dm(&self, arguments: Value) -> Result<Value> {
        let recipient = arguments
            .get("recipient")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("必須パラメータが不足: recipient"))?;

        let content = arguments
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("必須パラメータが不足: content"))?;

        if recipient.is_empty() {
            return Err(anyhow!("recipient は空にできません"));
        }
        if content.is_empty() {
            return Err(anyhow!("content は空にできません"));
        }

        debug!("DM 送信: recipient='{}'", recipient);

        let event_id = self.client.send_dm(recipient, content).await?;

        Ok(json!({
            "success": true,
            "event_id": event_id.to_hex(),
            "nevent": event_id.to_bech32().unwrap_or_default(),
            "message": "ダイレクトメッセージを送信しました。"
        }))
    }

    /// ダイレクトメッセージを取得
    async fn get_dms(&self, arguments: Value) -> Result<Value> {
        let with = arguments
            .get("with")
            .and_then(|v| v.as_str());

        let limit = extract_limit(&arguments);
        debug!("DM 取得: with={:?}, limit={}", with, limit);

        let messages = self.client.get_dms(with, limit).await?;

        let formatted: Vec<Value> = messages.iter().map(|dm| {
            format_dm_json(dm)
        }).collect();

        Ok(json!({
            "success": true,
            "count": messages.len(),
            "messages": formatted
        }))
    }

    /// リレーリストを取得
    async fn get_relay_list(&self, arguments: Value) -> Result<Value> {
        let pubkey = arguments
            .get("pubkey")
            .or_else(|| arguments.get("npub"))
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("必須パラメータが不足: pubkey"))?;

        if pubkey.is_empty() {
            return Err(anyhow!("pubkey は空にできません"));
        }

        debug!("リレーリスト取得: {}", pubkey);

        let relay_list = self.client.get_relay_list(pubkey).await?;

        let formatted_relays: Vec<Value> = relay_list.relays.iter().map(|entry| {
            json!({
                "url": entry.url,
                "read": entry.read,
                "write": entry.write
            })
        }).collect();

        Ok(json!({
            "success": true,
            "pubkey": relay_list.pubkey,
            "npub": relay_list.npub,
            "count": relay_list.relays.len(),
            "relays": formatted_relays
        }))
    }
}

/// 記事を JSON 表示形式にフォーマットするヘルパー（Phase 3: コンテンツ解析対応）
fn format_article_json(article: &crate::nostr_client::ArticleInfo) -> Value {
    let formatted_time = format_timestamp(article.created_at);
    let parsed = content::parse_content(&article.content);

    let mut result = json!({
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
        "formatted_time": formatted_time,
        "tags": article.tags,
        "is_draft": article.is_draft
    });

    // Phase 3: メディア検出
    if !parsed.media.is_empty() {
        result["media"] = json!(parsed.media);
    }

    // Phase 3: コンテンツ解析結果
    if !parsed.is_empty() {
        let mut parsed_content = json!({});
        if !parsed.hashtags.is_empty() {
            parsed_content["hashtags"] = json!(parsed.hashtags);
        }
        if !parsed.references.is_empty() {
            parsed_content["references"] = json!(parsed.references);
        }
        result["parsed_content"] = parsed_content;
    }

    result
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

/// DM を JSON 表示形式にフォーマットするヘルパー
fn format_dm_json(dm: &DirectMessageInfo) -> Value {
    let formatted_time = format_timestamp(dm.created_at);

    json!({
        "id": dm.id,
        "nevent": dm.nevent,
        "direction": dm.direction,
        "author": {
            "pubkey": dm.author.pubkey,
            "npub": dm.author.npub,
            "name": dm.author.name,
            "display_name": dm.author.display_name,
            "display": dm.author.display(),
            "picture": dm.author.picture,
            "nip05": dm.author.nip05
        },
        "peer_pubkey": dm.peer_pubkey,
        "content": dm.content,
        "created_at": dm.created_at,
        "formatted_time": formatted_time
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
