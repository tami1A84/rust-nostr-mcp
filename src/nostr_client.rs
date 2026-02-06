//! Nostr クライアントモジュール
//!
//! nostr-sdk のラッパーとして、MCP ツールで利用しやすい
//! 高レベルメソッドを提供します。

use anyhow::{anyhow, Context, Result};
use nostr_sdk::prelude::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Nostr クライアントの設定
#[derive(Debug, Clone)]
pub struct NostrClientConfig {
    /// nsec または hex 形式の秘密鍵（読み取り専用モードでは不要）
    pub secret_key: Option<String>,
    /// 一般操作用リレー URL のリスト
    pub relays: Vec<String>,
    /// NIP-50 検索対応リレー URL のリスト
    pub search_relays: Vec<String>,
}

/// 著者情報（表示用）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AuthorInfo {
    /// hex 形式の公開鍵
    pub pubkey: String,
    /// npub 形式の公開鍵
    pub npub: String,
    /// ユーザー名（プロフィールの name フィールド）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// 表示名
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// プロフィール画像 URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub picture: Option<String>,
    /// NIP-05 識別子
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nip05: Option<String>,
}

impl AuthorInfo {
    /// 最適な表示名を取得
    pub fn display(&self) -> String {
        self.display_name
            .as_ref()
            .or(self.name.as_ref())
            .cloned()
            .unwrap_or_else(|| self.short_npub())
    }

    /// 短縮 npub を取得（先頭12文字 + ... + 末尾4文字）
    pub fn short_npub(&self) -> String {
        if self.npub.len() > 16 {
            format!("{}...{}", &self.npub[..12], &self.npub[self.npub.len()-4..])
        } else {
            self.npub.clone()
        }
    }

    /// 公開鍵からデフォルトの著者情報を作成
    fn from_public_key(pk: &PublicKey) -> Self {
        Self {
            pubkey: pk.to_hex(),
            npub: pk.to_bech32().unwrap_or_default(),
            name: None,
            display_name: None,
            picture: None,
            nip05: None,
        }
    }
}

/// nostr-sdk クライアントのラッパー
pub struct NostrClient {
    /// nostr-sdk クライアント
    client: Client,
    /// 書き込みアクセスの有無（秘密鍵が設定されているか）
    has_write_access: bool,
    /// 認証済みユーザーの公開鍵
    public_key: Option<PublicKey>,
    /// NIP-50 検索対応リレー
    search_relays: Vec<String>,
    /// 接続状態
    connected: Arc<RwLock<bool>>,
    /// プロフィールキャッシュ（繰り返しのルックアップを回避）
    profile_cache: Arc<RwLock<HashMap<PublicKey, AuthorInfo>>>,
}

impl NostrClient {
    /// 指定された設定で新しい Nostr クライアントを作成します。
    pub async fn new(config: NostrClientConfig) -> Result<Self> {
        let (client, has_write_access, public_key) = if let Some(ref secret_key_str) = config.secret_key {
            let keys = Self::parse_secret_key(secret_key_str)?;
            let public_key = keys.public_key();

            info!("公開鍵で初期化: {}", public_key.to_bech32()?);

            let client = Client::new(keys);
            (client, true, Some(public_key))
        } else {
            let client = Client::default();
            (client, false, None)
        };

        for relay_url in &config.relays {
            if let Err(e) = client.add_relay(relay_url).await {
                warn!("リレー {} の追加に失敗: {}", relay_url, e);
            }
        }

        client.connect().await;
        tokio::time::sleep(Duration::from_millis(500)).await;

        Ok(Self {
            client,
            has_write_access,
            public_key,
            search_relays: config.search_relays,
            connected: Arc::new(RwLock::new(true)),
            profile_cache: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// nsec または hex 形式の秘密鍵をパース
    fn parse_secret_key(secret_key_str: &str) -> Result<Keys> {
        let secret_key_str = secret_key_str.trim();

        let secret_key = if secret_key_str.starts_with("nsec") {
            SecretKey::from_bech32(secret_key_str)
                .context("無効な nsec 形式です")?
        } else {
            SecretKey::from_hex(secret_key_str)
                .context("無効な hex 秘密鍵です")?
        };

        Ok(Keys::new(secret_key))
    }

    /// 書き込みアクセスの有無を確認
    #[allow(dead_code)]
    pub fn has_write_access(&self) -> bool {
        self.has_write_access
    }

    /// 認証済みの場合、公開鍵を取得
    #[allow(dead_code)]
    pub fn public_key(&self) -> Option<PublicKey> {
        self.public_key
    }

    /// 書き込みアクセスを要求し、ない場合はエラーを返す
    fn require_write_access(&self) -> Result<()> {
        if !self.has_write_access {
            return Err(anyhow!(
                "読み取り専用モードではこの操作はできません。設定ファイルに nsec を設定してください。"
            ));
        }
        Ok(())
    }

    /// 公開鍵のリストに対してプロフィールを取得（キャッシュ付き）
    async fn fetch_profiles(&self, pubkeys: &[PublicKey]) -> HashMap<PublicKey, AuthorInfo> {
        let mut results = HashMap::new();
        let mut to_fetch = Vec::new();

        // キャッシュから確認
        {
            let cache = self.profile_cache.read().await;
            for pk in pubkeys {
                if let Some(info) = cache.get(pk) {
                    results.insert(*pk, info.clone());
                } else {
                    to_fetch.push(*pk);
                }
            }
        }

        if to_fetch.is_empty() {
            return results;
        }

        // 未取得のプロフィールを取得
        let filter = Filter::new()
            .authors(to_fetch.clone())
            .kind(Kind::Metadata)
            .limit(to_fetch.len());

        match self.client.fetch_events(vec![filter], Duration::from_secs(5)).await {
            Ok(events) => {
                let mut cache = self.profile_cache.write().await;

                for event in events {
                    if let Ok(metadata) = serde_json::from_str::<Metadata>(&event.content) {
                        let author_info = AuthorInfo {
                            pubkey: event.pubkey.to_hex(),
                            npub: event.pubkey.to_bech32().unwrap_or_default(),
                            name: metadata.name,
                            display_name: metadata.display_name,
                            picture: metadata.picture,
                            nip05: metadata.nip05,
                        };
                        cache.insert(event.pubkey, author_info.clone());
                        results.insert(event.pubkey, author_info);
                    }
                }

                // 見つからなかったプロフィールにはデフォルト値を設定
                for pk in &to_fetch {
                    results.entry(*pk).or_insert_with(|| AuthorInfo::from_public_key(pk));
                }
            }
            Err(e) => {
                warn!("プロフィールの取得に失敗: {}", e);
                for pk in &to_fetch {
                    results.entry(*pk).or_insert_with(|| AuthorInfo::from_public_key(pk));
                }
            }
        }

        results
    }

    /// イベントリストからノート情報のリストに変換するヘルパー
    fn events_to_notes(&self, events: &[Event], profiles: &HashMap<PublicKey, AuthorInfo>) -> Vec<NoteInfo> {
        events.iter().map(|event| {
            let author = profiles
                .get(&event.pubkey)
                .cloned()
                .unwrap_or_else(|| AuthorInfo::from_public_key(&event.pubkey));

            NoteInfo {
                id: event.id.to_hex(),
                nevent: event.id.to_bech32().unwrap_or_default(),
                author,
                content: event.content.clone(),
                created_at: event.created_at.as_u64(),
                reactions: None,
                replies: None,
            }
        }).collect()
    }

    /// イベントリストからユニークな公開鍵を収集
    fn collect_pubkeys(events: &[Event]) -> Vec<PublicKey> {
        events.iter()
            .map(|e| e.pubkey)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect()
    }

    /// ノートをタイムスタンプ降順でソートし、指定数に切り詰める
    fn sort_and_truncate(notes: &mut Vec<NoteInfo>, limit: usize) {
        notes.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        notes.truncate(limit);
    }

    /// 新しいノート (Kind 1) を投稿します。
    pub async fn post_note(&self, content: &str) -> Result<EventId> {
        self.require_write_access()?;

        let builder = EventBuilder::text_note(content);
        let output = self.client.send_event_builder(builder).await
            .context("ノートの公開に失敗しました")?;

        let event_id = *output.id();
        info!("ノートを公開しました。イベント ID: {}", event_id);
        Ok(event_id)
    }

    /// タイムラインを取得します（認証済みの場合はフォロー中のユーザー、それ以外はグローバル）。
    pub async fn get_timeline(&self, limit: u64) -> Result<Vec<NoteInfo>> {
        let filter = if let Some(pk) = self.public_key {
            let contact_filter = Filter::new()
                .author(pk)
                .kind(Kind::ContactList)
                .limit(1);

            let contacts: Vec<Event> = self.client
                .fetch_events(vec![contact_filter], Duration::from_secs(5))
                .await
                .ok()
                .into_iter()
                .flatten()
                .collect();

            if let Some(contact_event) = contacts.into_iter().next() {
                let followed: Vec<PublicKey> = contact_event.tags.iter()
                    .filter_map(|tag| {
                        if let Some(TagStandard::PublicKey { public_key, .. }) = tag.as_standardized() {
                            Some(*public_key)
                        } else {
                            None
                        }
                    })
                    .collect();

                if !followed.is_empty() {
                    debug!("フォロー中アカウント: {} 件", followed.len());
                    Filter::new()
                        .authors(followed)
                        .kind(Kind::TextNote)
                        .limit(limit as usize)
                } else {
                    Filter::new()
                        .kind(Kind::TextNote)
                        .limit(limit as usize)
                }
            } else {
                Filter::new()
                    .kind(Kind::TextNote)
                    .limit(limit as usize)
            }
        } else {
            Filter::new()
                .kind(Kind::TextNote)
                .limit(limit as usize)
        };

        let events = self.client
            .fetch_events(vec![filter], Duration::from_secs(10))
            .await
            .context("タイムラインの取得に失敗しました")?;

        let events_vec: Vec<Event> = events.into_iter().collect();
        let pubkeys = Self::collect_pubkeys(&events_vec);
        let profiles = self.fetch_profiles(&pubkeys).await;
        let mut notes = self.events_to_notes(&events_vec, &profiles);
        Self::sort_and_truncate(&mut notes, limit as usize);

        Ok(notes)
    }

    /// NIP-50 対応リレーでノートを検索します。
    pub async fn search_notes(&self, query: &str, limit: u64) -> Result<Vec<NoteInfo>> {
        let search_client = Client::default();

        for relay_url in &self.search_relays {
            if let Err(e) = search_client.add_relay(relay_url).await {
                warn!("検索リレー {} の追加に失敗: {}", relay_url, e);
            }
        }

        search_client.connect().await;
        tokio::time::sleep(Duration::from_millis(300)).await;

        let filter = Filter::new()
            .kind(Kind::TextNote)
            .search(query)
            .limit(limit as usize);

        let events = search_client
            .fetch_events(vec![filter], Duration::from_secs(15))
            .await
            .context("ノートの検索に失敗しました")?;

        let events_vec: Vec<Event> = events.into_iter().collect();
        let pubkeys = Self::collect_pubkeys(&events_vec);
        let profiles = self.fetch_profiles(&pubkeys).await;
        let mut notes = self.events_to_notes(&events_vec, &profiles);
        Self::sort_and_truncate(&mut notes, limit as usize);

        let _ = search_client.disconnect().await;

        Ok(notes)
    }

    /// 指定されたユーザーのプロフィール情報を取得します。
    pub async fn get_profile(&self, npub: &str) -> Result<ProfileInfo> {
        let npub = npub.trim();

        let public_key = if npub.starts_with("npub") {
            PublicKey::from_bech32(npub)
                .context("無効な npub 形式です")?
        } else {
            PublicKey::from_hex(npub)
                .context("無効な hex 公開鍵です")?
        };

        let filter = Filter::new()
            .author(public_key)
            .kind(Kind::Metadata)
            .limit(1);

        let events = self.client
            .fetch_events(vec![filter], Duration::from_secs(10))
            .await
            .context("プロフィールの取得に失敗しました")?;

        let profile_event = events
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("{} のプロフィールが見つかりません", npub))?;

        let metadata: Metadata = serde_json::from_str(&profile_event.content)
            .context("プロフィールメタデータのパースに失敗しました")?;

        Ok(ProfileInfo {
            pubkey: public_key.to_hex(),
            npub: public_key.to_bech32()?,
            name: metadata.name,
            display_name: metadata.display_name,
            about: metadata.about,
            picture: metadata.picture,
            banner: metadata.banner,
            nip05: metadata.nip05,
            lud16: metadata.lud16,
            website: metadata.website,
        })
    }

    // ========================================
    // Phase 1: NIP-23 長文コンテンツサポート
    // ========================================

    /// 長文記事 (Kind 30023) を投稿します。
    pub async fn post_article(&self, params: ArticleParams) -> Result<ArticleInfo> {
        self.require_write_access()?;

        let d_tag = params.identifier.unwrap_or_else(|| {
            // d タグが未指定の場合、タイトルからスラッグを生成
            slug_from_title(&params.title)
        });

        let mut tags = vec![
            Tag::identifier(d_tag.clone()),
            Tag::custom(TagKind::Title, vec![params.title.clone()]),
        ];

        if let Some(ref summary) = params.summary {
            tags.push(Tag::custom(TagKind::custom("summary".to_string()), vec![summary.clone()]));
        }

        if let Some(ref image) = params.image {
            tags.push(Tag::custom(TagKind::custom("image".to_string()), vec![image.clone()]));
        }

        if let Some(ref hashtags) = params.tags {
            for t in hashtags {
                tags.push(Tag::hashtag(t.clone()));
            }
        }

        if let Some(published_at) = params.published_at {
            tags.push(Tag::custom(
                TagKind::custom("published_at".to_string()),
                vec![published_at.to_string()],
            ));
        } else {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            tags.push(Tag::custom(
                TagKind::custom("published_at".to_string()),
                vec![now.to_string()],
            ));
        }

        let builder = EventBuilder::new(Kind::LongFormTextNote, &params.content)
            .tags(tags);

        let output = self.client.send_event_builder(builder).await
            .context("記事の公開に失敗しました")?;

        let event_id = *output.id();
        info!("記事を公開しました。イベント ID: {}", event_id);

        // naddr の生成
        let naddr = if let Some(pk) = self.public_key {
            let coordinate = Coordinate::new(Kind::LongFormTextNote, pk).identifier(&d_tag);
            coordinate.to_bech32().ok()
        } else {
            None
        };

        Ok(ArticleInfo {
            id: event_id.to_hex(),
            nevent: event_id.to_bech32().unwrap_or_default(),
            naddr,
            identifier: d_tag,
            title: params.title,
            summary: params.summary,
            image: params.image,
            content: params.content,
            author: self.public_key
                .map(|pk| AuthorInfo::from_public_key(&pk)),
            published_at: params.published_at,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            tags: params.tags,
            is_draft: false,
        })
    }

    /// 長文記事 (Kind 30023) を取得します。
    pub async fn get_articles(&self, author: Option<&str>, tags: Option<&[String]>, limit: u64) -> Result<Vec<ArticleInfo>> {
        let mut filter = Filter::new()
            .kind(Kind::LongFormTextNote)
            .limit(limit as usize);

        if let Some(author_str) = author {
            let pk = Self::parse_public_key(author_str)?;
            filter = filter.author(pk);
        }

        if let Some(hashtags) = tags {
            filter = filter.hashtags(hashtags.to_vec());
        }

        let events = self.client
            .fetch_events(vec![filter], Duration::from_secs(15))
            .await
            .context("記事の取得に失敗しました")?;

        let events_vec: Vec<Event> = events.into_iter().collect();
        let pubkeys = Self::collect_pubkeys(&events_vec);
        let profiles = self.fetch_profiles(&pubkeys).await;

        let mut articles: Vec<ArticleInfo> = events_vec.iter().map(|event| {
            Self::event_to_article(event, &profiles)
        }).collect();

        articles.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        articles.truncate(limit as usize);

        Ok(articles)
    }

    /// 記事を下書き (Kind 30024) として保存します。
    pub async fn save_draft(&self, params: ArticleParams) -> Result<ArticleInfo> {
        self.require_write_access()?;

        let d_tag = params.identifier.unwrap_or_else(|| {
            slug_from_title(&params.title)
        });

        let mut tags = vec![
            Tag::identifier(d_tag.clone()),
            Tag::custom(TagKind::Title, vec![params.title.clone()]),
        ];

        if let Some(ref summary) = params.summary {
            tags.push(Tag::custom(TagKind::custom("summary".to_string()), vec![summary.clone()]));
        }

        if let Some(ref image) = params.image {
            tags.push(Tag::custom(TagKind::custom("image".to_string()), vec![image.clone()]));
        }

        if let Some(ref hashtags) = params.tags {
            for t in hashtags {
                tags.push(Tag::hashtag(t.clone()));
            }
        }

        // Kind 30024 = Draft Long-form Content
        let draft_kind = Kind::from(30024);

        let builder = EventBuilder::new(draft_kind, &params.content)
            .tags(tags);

        let output = self.client.send_event_builder(builder).await
            .context("下書きの保存に失敗しました")?;

        let event_id = *output.id();
        info!("下書きを保存しました。イベント ID: {}", event_id);

        let naddr = if let Some(pk) = self.public_key {
            let coordinate = Coordinate::new(draft_kind, pk).identifier(&d_tag);
            coordinate.to_bech32().ok()
        } else {
            None
        };

        Ok(ArticleInfo {
            id: event_id.to_hex(),
            nevent: event_id.to_bech32().unwrap_or_default(),
            naddr,
            identifier: d_tag,
            title: params.title,
            summary: params.summary,
            image: params.image,
            content: params.content,
            author: self.public_key
                .map(|pk| AuthorInfo::from_public_key(&pk)),
            published_at: params.published_at,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            tags: params.tags,
            is_draft: true,
        })
    }

    /// ユーザーの下書き記事 (Kind 30024) を取得します。
    pub async fn get_drafts(&self, limit: u64) -> Result<Vec<ArticleInfo>> {
        let pk = self.public_key
            .ok_or_else(|| anyhow!("下書きの取得には認証が必要です。設定ファイルに nsec を設定してください。"))?;

        let draft_kind = Kind::from(30024);

        let filter = Filter::new()
            .author(pk)
            .kind(draft_kind)
            .limit(limit as usize);

        let events = self.client
            .fetch_events(vec![filter], Duration::from_secs(10))
            .await
            .context("下書きの取得に失敗しました")?;

        let events_vec: Vec<Event> = events.into_iter().collect();
        let pubkeys = Self::collect_pubkeys(&events_vec);
        let profiles = self.fetch_profiles(&pubkeys).await;

        let mut articles: Vec<ArticleInfo> = events_vec.iter().map(|event| {
            let mut article = Self::event_to_article(event, &profiles);
            article.is_draft = true;
            article
        }).collect();

        articles.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        articles.truncate(limit as usize);

        Ok(articles)
    }

    /// 公開鍵文字列をパース（npub または hex 対応）
    fn parse_public_key(key_str: &str) -> Result<PublicKey> {
        let key_str = key_str.trim();
        if key_str.starts_with("npub") {
            PublicKey::from_bech32(key_str).context("無効な npub 形式です")
        } else {
            PublicKey::from_hex(key_str).context("無効な hex 公開鍵です")
        }
    }

    /// イベントから記事情報に変換するヘルパー
    fn event_to_article(event: &Event, profiles: &HashMap<PublicKey, AuthorInfo>) -> ArticleInfo {
        let author = profiles
            .get(&event.pubkey)
            .cloned()
            .unwrap_or_else(|| AuthorInfo::from_public_key(&event.pubkey));

        let identifier = event.tags.iter()
            .find_map(|tag| {
                if let Some(TagStandard::Identifier(id)) = tag.as_standardized() {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default();

        let title = extract_tag_value(event, "title")
            .unwrap_or_else(|| "無題".to_string());

        let summary = extract_tag_value(event, "summary");
        let image = extract_tag_value(event, "image");

        let published_at = extract_tag_value(event, "published_at")
            .and_then(|s| s.parse::<u64>().ok());

        let tags: Vec<String> = event.tags.iter()
            .filter_map(|tag| {
                if let Some(TagStandard::Hashtag(h)) = tag.as_standardized() {
                    Some(h.clone())
                } else {
                    None
                }
            })
            .collect();

        let naddr = Coordinate::new(event.kind, event.pubkey)
            .identifier(&identifier)
            .to_bech32()
            .ok();

        ArticleInfo {
            id: event.id.to_hex(),
            nevent: event.id.to_bech32().unwrap_or_default(),
            naddr,
            identifier,
            title,
            summary,
            image,
            content: event.content.clone(),
            author: Some(author),
            published_at,
            created_at: event.created_at.as_u64(),
            tags: if tags.is_empty() { None } else { Some(tags) },
            is_draft: event.kind == Kind::from(30024),
        }
    }

    /// すべてのリレーから切断します。
    pub async fn disconnect(&self) {
        let _ = self.client.disconnect().await;
        let mut connected = self.connected.write().await;
        *connected = false;
    }
}

// ========================================
// データ構造体
// ========================================

/// ノートの情報（表示用）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NoteInfo {
    /// hex 形式のイベント ID
    pub id: String,
    /// リンク用の nevent 形式のイベント ID
    pub nevent: String,
    /// 著者情報
    pub author: AuthorInfo,
    /// ノートの内容
    pub content: String,
    /// 作成日時の Unix タイムスタンプ
    pub created_at: u64,
    /// リアクション数（将来の拡張用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reactions: Option<u64>,
    /// リプライ数（将来の拡張用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replies: Option<u64>,
}

/// プロフィール情報
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProfileInfo {
    /// hex 形式の公開鍵
    pub pubkey: String,
    /// npub 形式の公開鍵
    pub npub: String,
    /// ユーザー名
    pub name: Option<String>,
    /// 表示名
    pub display_name: Option<String>,
    /// 自己紹介文
    pub about: Option<String>,
    /// プロフィール画像 URL
    pub picture: Option<String>,
    /// バナー画像 URL
    pub banner: Option<String>,
    /// NIP-05 識別子
    pub nip05: Option<String>,
    /// Lightning アドレス (LUD-16)
    pub lud16: Option<String>,
    /// ウェブサイト URL
    pub website: Option<String>,
}

/// 記事投稿のパラメータ
#[derive(Debug, Clone)]
pub struct ArticleParams {
    /// 記事タイトル
    pub title: String,
    /// Markdown コンテンツ
    pub content: String,
    /// 識別子（d タグ、未指定時はタイトルから自動生成）
    pub identifier: Option<String>,
    /// 要約
    pub summary: Option<String>,
    /// ヘッダー画像 URL
    pub image: Option<String>,
    /// トピックハッシュタグ
    pub tags: Option<Vec<String>>,
    /// 公開日時の Unix タイムスタンプ
    pub published_at: Option<u64>,
}

/// 記事情報（NIP-23 長文コンテンツ）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ArticleInfo {
    /// hex 形式のイベント ID
    pub id: String,
    /// nevent 形式のイベント ID
    pub nevent: String,
    /// naddr 形式のアドレス（アドレス可能なイベント用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub naddr: Option<String>,
    /// 識別子（d タグ）
    pub identifier: String,
    /// 記事タイトル
    pub title: String,
    /// 要約
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// ヘッダー画像 URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// Markdown コンテンツ
    pub content: String,
    /// 著者情報
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<AuthorInfo>,
    /// 公開日時の Unix タイムスタンプ
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_at: Option<u64>,
    /// 作成日時の Unix タイムスタンプ
    pub created_at: u64,
    /// ハッシュタグ
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    /// 下書きかどうか
    pub is_draft: bool,
}

// ========================================
// ユーティリティ関数
// ========================================

/// タイトルから URL 用スラッグを生成
fn slug_from_title(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else if c == ' ' {
                '-'
            } else {
                // 日本語等のマルチバイト文字はそのまま保持
                c
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string()
}

/// イベントのタグから指定されたキーの値を抽出
fn extract_tag_value(event: &Event, key: &str) -> Option<String> {
    event.tags.iter().find_map(|tag| {
        let values = tag.as_slice();
        if values.len() >= 2 && values[0] == key {
            Some(values[1].to_string())
        } else {
            None
        }
    })
}
