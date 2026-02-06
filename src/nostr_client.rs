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
    /// Nostr Wallet Connect URI（NIP-47、Zap 送信用）
    pub nwc_uri: Option<String>,
    /// 認証モード（Phase 6: NIP-46 対応）
    pub auth_mode: crate::config::AuthMode,
    /// NIP-46 セッション設定
    pub nip46_config: Option<crate::nip46::Nip46Config>,
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
    /// 書き込みアクセスの有無（秘密鍵が設定されているか、または NIP-46 接続済み）
    has_write_access: bool,
    /// 認証済みユーザーの公開鍵
    public_key: Option<PublicKey>,
    /// NIP-50 検索対応リレー
    search_relays: Vec<String>,
    /// 接続状態
    connected: Arc<RwLock<bool>>,
    /// プロフィールキャッシュ（繰り返しのルックアップを回避）
    profile_cache: Arc<RwLock<HashMap<PublicKey, AuthorInfo>>>,
    /// NWC URI（Zap 送信用、Phase 4）
    #[allow(dead_code)]
    nwc_uri: Option<String>,
    /// NIP-46 サイナーが有効か（Phase 6: 認証モード切り替え）
    nip46_active: Arc<RwLock<bool>>,
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

        // Phase 4: NWC Zapper の設定
        if let Some(ref nwc_uri_str) = config.nwc_uri {
            match NostrWalletConnectURI::parse(nwc_uri_str) {
                Ok(uri) => {
                    let nwc_zapper = nwc::NWC::new(uri);
                    client.set_zapper(nwc_zapper).await;
                    info!("NWC Zapper を設定しました");
                }
                Err(e) => {
                    warn!("NWC URI のパースに失敗: {}。Zap 送信は利用できません。", e);
                }
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
            nwc_uri: config.nwc_uri,
            nip46_active: Arc::new(RwLock::new(false)),
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
                "読み取り専用モードではこの操作はできません。設定ファイルに nsec を設定するか、NIP-46 で接続してください。"
            ));
        }
        Ok(())
    }

    /// NIP-46 リモートサイナーを有効化し、書き込みアクセスを切り替える（Phase 6 Step 6-3）
    pub async fn enable_nip46_signer(
        &mut self,
        signer: nostr_connect::prelude::NostrConnect,
        user_pubkey: PublicKey,
    ) -> Result<()> {
        info!(
            "NIP-46 サイナーに切り替え: {}",
            user_pubkey.to_bech32().unwrap_or_default()
        );

        self.client.set_signer(signer).await;
        self.has_write_access = true;
        self.public_key = Some(user_pubkey);
        *self.nip46_active.write().await = true;

        info!("NIP-46 リモートサイナーが有効化されました");
        Ok(())
    }

    /// NIP-46 リモートサイナーを無効化し、元の状態に戻す
    pub async fn disable_nip46_signer(&mut self) {
        info!("NIP-46 サイナーを無効化");
        // ローカル秘密鍵がない場合は書き込みアクセスも無効に
        let nip46_was_active = *self.nip46_active.read().await;
        if nip46_was_active {
            *self.nip46_active.write().await = false;
            // ローカル鍵がなければ書き込みを無効化
            // (client の signer はそのまま残るが、has_write_access で制御)
            self.has_write_access = false;
            self.public_key = None;
        }
    }

    /// NIP-46 サイナーが有効かどうか
    pub async fn is_nip46_active(&self) -> bool {
        *self.nip46_active.read().await
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

        // リアクション数とリプライ数を取得
        self.enrich_notes_with_counts(&mut notes).await;

        Ok(notes)
    }

    /// ノートにリアクション数とリプライ数を付与するヘルパー
    async fn enrich_notes_with_counts(&self, notes: &mut [NoteInfo]) {
        if notes.is_empty() {
            return;
        }

        let event_ids: Vec<EventId> = notes.iter()
            .filter_map(|n| EventId::from_hex(&n.id).ok())
            .collect();

        if event_ids.is_empty() {
            return;
        }

        // リアクション (Kind 7) を一括取得
        let reaction_filter = Filter::new()
            .kind(Kind::Reaction)
            .events(event_ids.clone())
            .limit(1000);

        // リプライ (Kind 1 で e タグ参照) を一括取得
        let reply_filter = Filter::new()
            .kind(Kind::TextNote)
            .events(event_ids.clone())
            .limit(1000);

        let (reactions_result, replies_result) = tokio::join!(
            self.client.fetch_events(vec![reaction_filter], Duration::from_secs(5)),
            self.client.fetch_events(vec![reply_filter], Duration::from_secs(5))
        );

        // リアクション数をカウント
        let mut reaction_counts: HashMap<String, u64> = HashMap::new();
        if let Ok(events) = reactions_result {
            for event in events {
                for tag in event.tags.iter() {
                    let values = tag.as_slice();
                    if values.len() >= 2 && values[0] == "e" {
                        *reaction_counts.entry(values[1].to_string()).or_insert(0) += 1;
                    }
                }
            }
        }

        // リプライ数をカウント
        let mut reply_counts: HashMap<String, u64> = HashMap::new();
        if let Ok(events) = replies_result {
            for event in events {
                for tag in event.tags.iter() {
                    let values = tag.as_slice();
                    if values.len() >= 2 && values[0] == "e" {
                        *reply_counts.entry(values[1].to_string()).or_insert(0) += 1;
                    }
                }
            }
        }

        // ノートに付与
        for note in notes.iter_mut() {
            note.reactions = Some(*reaction_counts.get(&note.id).unwrap_or(&0));
            note.replies = Some(*reply_counts.get(&note.id).unwrap_or(&0));
        }
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
    // Phase 3: プロフィール統計情報
    // ========================================

    /// ユーザーのプロフィール統計情報（フォロー数・フォロワー数・ノート数）を取得します。
    pub async fn get_profile_stats(&self, pubkey_str: &str) -> Result<ProfileStats> {
        let public_key = Self::parse_public_key(pubkey_str)?;

        // フォロー数: Kind 3 (ContactList) の p タグ数
        let contact_filter = Filter::new()
            .author(public_key)
            .kind(Kind::ContactList)
            .limit(1);

        // ノート数: Kind 1 の件数（上限付き）
        let notes_filter = Filter::new()
            .author(public_key)
            .kind(Kind::TextNote)
            .limit(5000);

        // フォロワー数: Kind 3 で対象ユーザーを p タグで参照しているイベント
        let followers_filter = Filter::new()
            .kind(Kind::ContactList)
            .pubkey(public_key)
            .limit(5000);

        let (contacts_result, notes_result, followers_result) = tokio::join!(
            self.client.fetch_events(vec![contact_filter], Duration::from_secs(10)),
            self.client.fetch_events(vec![notes_filter], Duration::from_secs(10)),
            self.client.fetch_events(vec![followers_filter], Duration::from_secs(10))
        );

        // フォロー数
        let following = contacts_result
            .ok()
            .and_then(|events| events.into_iter().next())
            .map(|event| {
                event.tags.iter()
                    .filter(|tag| {
                        let values = tag.as_slice();
                        values.len() >= 2 && values[0] == "p"
                    })
                    .count() as u64
            })
            .unwrap_or(0);

        // ノート数
        let notes = notes_result
            .map(|events| events.into_iter().count() as u64)
            .unwrap_or(0);

        // フォロワー数（ユニークな著者のみカウント）
        let followers = followers_result
            .map(|events| {
                events.into_iter()
                    .map(|e| e.pubkey)
                    .collect::<std::collections::HashSet<_>>()
                    .len() as u64
            })
            .unwrap_or(0);

        Ok(ProfileStats {
            following,
            followers,
            notes,
        })
    }

    // ========================================
    // Phase 1: NIP-23 長文コンテンツサポート
    // ========================================

    /// 長文記事 (Kind 30023) を投稿します。
    pub async fn post_article(&self, params: ArticleParams) -> Result<ArticleInfo> {
        self.publish_article_event(params, Kind::LongFormTextNote, false).await
    }

    /// 長文記事 (Kind 30023) を取得します。
    pub async fn get_articles(&self, author: Option<&str>, tags: Option<&[String]>, limit: u64) -> Result<Vec<ArticleInfo>> {
        self.fetch_articles_by_kind(Kind::LongFormTextNote, author, tags, limit).await
    }

    /// 記事を下書き (Kind 30024) として保存します。
    pub async fn save_draft(&self, params: ArticleParams) -> Result<ArticleInfo> {
        self.publish_article_event(params, Kind::from(30024), true).await
    }

    /// ユーザーの下書き記事 (Kind 30024) を取得します。
    pub async fn get_drafts(&self, limit: u64) -> Result<Vec<ArticleInfo>> {
        self.fetch_articles_by_kind(Kind::from(30024), None, None, limit).await
    }

    /// 記事/下書きを公開する共通ヘルパー
    async fn publish_article_event(&self, params: ArticleParams, kind: Kind, is_draft: bool) -> Result<ArticleInfo> {
        self.require_write_access()?;

        let d_tag = params.identifier.unwrap_or_else(|| {
            slug_from_title(&params.title)
        });

        let mut tags = build_article_tags(&params.title, &params.summary, &params.image, &params.tags, &d_tag);

        // 公開記事の場合のみ published_at を追加
        if !is_draft {
            let ts = params.published_at.unwrap_or_else(current_unix_timestamp);
            tags.push(Tag::custom(
                TagKind::custom("published_at".to_string()),
                vec![ts.to_string()],
            ));
        }

        let builder = EventBuilder::new(kind, &params.content).tags(tags);

        let label = if is_draft { "下書き" } else { "記事" };
        let output = self.client.send_event_builder(builder).await
            .context(format!("{}の公開に失敗しました", label))?;

        let event_id = *output.id();
        info!("{}を公開しました。イベント ID: {}", label, event_id);

        let naddr = self.public_key.and_then(|pk| {
            Coordinate::new(kind, pk).identifier(&d_tag).to_bech32().ok()
        });

        Ok(ArticleInfo {
            id: event_id.to_hex(),
            nevent: event_id.to_bech32().unwrap_or_default(),
            naddr,
            identifier: d_tag,
            title: params.title,
            summary: params.summary,
            image: params.image,
            content: params.content,
            author: self.public_key.map(|pk| AuthorInfo::from_public_key(&pk)),
            published_at: params.published_at,
            created_at: current_unix_timestamp(),
            tags: params.tags,
            is_draft,
        })
    }

    /// 記事/下書きを取得する共通ヘルパー
    async fn fetch_articles_by_kind(
        &self,
        kind: Kind,
        author: Option<&str>,
        tags: Option<&[String]>,
        limit: u64,
    ) -> Result<Vec<ArticleInfo>> {
        let is_draft = kind == Kind::from(30024);

        // 下書き取得は認証必須
        let mut filter = if is_draft {
            let pk = self.public_key
                .ok_or_else(|| anyhow!("下書きの取得には認証が必要です。設定ファイルに nsec を設定してください。"))?;
            Filter::new().author(pk).kind(kind).limit(limit as usize)
        } else {
            let mut f = Filter::new().kind(kind).limit(limit as usize);
            if let Some(author_str) = author {
                let pk = Self::parse_public_key(author_str)?;
                f = f.author(pk);
            }
            f
        };

        if let Some(hashtags) = tags {
            filter = filter.hashtags(hashtags.to_vec());
        }

        let timeout = if is_draft { 10 } else { 15 };
        let events = self.client
            .fetch_events(vec![filter], Duration::from_secs(timeout))
            .await
            .context(format!("{}の取得に失敗しました", if is_draft { "下書き" } else { "記事" }))?;

        let events_vec: Vec<Event> = events.into_iter().collect();
        let pubkeys = Self::collect_pubkeys(&events_vec);
        let profiles = self.fetch_profiles(&pubkeys).await;

        let mut articles: Vec<ArticleInfo> = events_vec.iter().map(|event| {
            let mut article = Self::event_to_article(event, &profiles);
            if is_draft {
                article.is_draft = true;
            }
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

    // ========================================
    // Phase 2: タイムライン拡張機能
    // ========================================

    /// スレッド形式でノートとリプライを取得します（NIP-10 対応）。
    pub async fn get_thread(&self, note_id: &str, depth: u64) -> Result<ThreadInfo> {
        let event_id = Self::parse_event_id(note_id)?;

        // ルートノートを取得
        let root_filter = Filter::new()
            .id(event_id)
            .limit(1);

        let root_events = self.client
            .fetch_events(vec![root_filter], Duration::from_secs(10))
            .await
            .context("ルートノートの取得に失敗しました")?;

        let root_event = root_events
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("ノートが見つかりません: {}", note_id))?;

        // リプライを取得（e タグでルートノートを参照しているイベント）
        let reply_filter = Filter::new()
            .kind(Kind::TextNote)
            .event(event_id)
            .limit(200);

        let reply_events = self.client
            .fetch_events(vec![reply_filter], Duration::from_secs(10))
            .await
            .context("リプライの取得に失敗しました")?;

        let reply_events_vec: Vec<Event> = reply_events.into_iter().collect();

        // リアクション数を取得
        let reaction_filter = Filter::new()
            .kind(Kind::Reaction)
            .event(event_id)
            .limit(500);

        let reaction_count = match self.client
            .fetch_events(vec![reaction_filter], Duration::from_secs(5))
            .await {
            Ok(events) => events.into_iter().count() as u64,
            Err(_) => 0,
        };

        // プロフィールを取得
        let mut all_events = vec![root_event.clone()];
        all_events.extend(reply_events_vec.iter().cloned());
        let pubkeys = Self::collect_pubkeys(&all_events);
        let profiles = self.fetch_profiles(&pubkeys).await;

        // ルートノート情報を作成
        let root_author = profiles
            .get(&root_event.pubkey)
            .cloned()
            .unwrap_or_else(|| AuthorInfo::from_public_key(&root_event.pubkey));

        let root_note = NoteInfo {
            id: root_event.id.to_hex(),
            nevent: root_event.id.to_bech32().unwrap_or_default(),
            author: root_author,
            content: root_event.content.clone(),
            created_at: root_event.created_at.as_u64(),
            reactions: Some(reaction_count),
            replies: Some(reply_events_vec.len() as u64),
        };

        // リプライをスレッド構造に変換
        let replies = self.build_thread_replies(&reply_events_vec, &profiles, &event_id, depth);

        Ok(ThreadInfo {
            root: root_note,
            replies,
            total_replies: reply_events_vec.len() as u64,
            depth,
        })
    }

    /// リプライイベントからスレッド構造を構築するヘルパー
    fn build_thread_replies(
        &self,
        events: &[Event],
        profiles: &HashMap<PublicKey, AuthorInfo>,
        parent_id: &EventId,
        max_depth: u64,
    ) -> Vec<ThreadReply> {
        if max_depth == 0 {
            return vec![];
        }

        let mut replies: Vec<ThreadReply> = events
            .iter()
            .filter(|event| {
                // NIP-10: 最後の e タグが reply マーカー（親への参照）
                event.tags.iter().any(|tag| {
                    let values = tag.as_slice();
                    values.len() >= 2
                        && values[0] == "e"
                        && values[1] == parent_id.to_hex()
                })
            })
            .map(|event| {
                let author = profiles
                    .get(&event.pubkey)
                    .cloned()
                    .unwrap_or_else(|| AuthorInfo::from_public_key(&event.pubkey));

                let child_replies = self.build_thread_replies(
                    events,
                    profiles,
                    &event.id,
                    max_depth - 1,
                );

                ThreadReply {
                    note: NoteInfo {
                        id: event.id.to_hex(),
                        nevent: event.id.to_bech32().unwrap_or_default(),
                        author,
                        content: event.content.clone(),
                        created_at: event.created_at.as_u64(),
                        reactions: None,
                        replies: Some(child_replies.len() as u64),
                    },
                    replies: child_replies,
                }
            })
            .collect();

        replies.sort_by(|a, b| a.note.created_at.cmp(&b.note.created_at));
        replies
    }

    /// イベント ID で単一のイベントを取得するヘルパー
    async fn fetch_event_by_id(&self, event_id: EventId, context: &str) -> Result<Event> {
        let filter = Filter::new().id(event_id).limit(1);
        let events = self.client
            .fetch_events(vec![filter], Duration::from_secs(5))
            .await
            .context(format!("{}の取得に失敗しました", context))?;
        events
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("{}が見つかりません", context))
    }

    /// ノートにリアクション (Kind 7, NIP-25) を送信します。
    pub async fn react_to_note(&self, note_id: &str, reaction: &str) -> Result<EventId> {
        self.require_write_access()?;

        let event_id = Self::parse_event_id(note_id)?;
        let target_event = self.fetch_event_by_id(event_id, "リアクション対象のノート").await?;

        // NIP-25: リアクションイベントを作成
        let builder = EventBuilder::new(Kind::Reaction, reaction)
            .tags(vec![
                Tag::event(event_id),
                Tag::public_key(target_event.pubkey),
            ]);

        let output = self.client.send_event_builder(builder).await
            .context("リアクションの送信に失敗しました")?;

        let reaction_id = *output.id();
        info!("リアクションを送信しました。イベント ID: {}", reaction_id);
        Ok(reaction_id)
    }

    /// 既存のノートに返信を投稿します（NIP-10 対応）。
    pub async fn reply_to_note(&self, note_id: &str, content: &str) -> Result<EventId> {
        self.require_write_access()?;

        let event_id = Self::parse_event_id(note_id)?;
        let target_event = self.fetch_event_by_id(event_id, "返信対象のノート").await?;

        // NIP-10: root と reply のマーカーを設定
        // 対象ノート自体にルートがある場合はそれを引き継ぐ
        let mut tags = Vec::new();

        // ルートイベントの検出
        let root_id = target_event.tags.iter().find_map(|tag| {
            let values = tag.as_slice();
            if values.len() >= 4 && values[0] == "e" && values[3] == "root" {
                EventId::from_hex(&values[1]).ok()
            } else {
                None
            }
        });

        if let Some(root) = root_id {
            // 既存スレッドへの返信: root を引き継ぎ、対象ノートを reply とする
            tags.push(Tag::parse(vec!["e".to_string(), root.to_hex(), String::new(), "root".to_string()]).unwrap());
            tags.push(Tag::parse(vec!["e".to_string(), event_id.to_hex(), String::new(), "reply".to_string()]).unwrap());
        } else {
            // 新規スレッド開始: 対象ノートが root かつ reply
            tags.push(Tag::parse(vec!["e".to_string(), event_id.to_hex(), String::new(), "root".to_string()]).unwrap());
            tags.push(Tag::parse(vec!["e".to_string(), event_id.to_hex(), String::new(), "reply".to_string()]).unwrap());
        }

        // 対象ノートの著者を p タグで追加
        tags.push(Tag::public_key(target_event.pubkey));

        let builder = EventBuilder::text_note(content)
            .tags(tags);

        let output = self.client.send_event_builder(builder).await
            .context("返信の投稿に失敗しました")?;

        let reply_id = *output.id();
        info!("返信を投稿しました。イベント ID: {}", reply_id);
        Ok(reply_id)
    }

    /// ユーザーへのメンションとリアクションの通知を取得します。
    pub async fn get_notifications(&self, since: Option<u64>, limit: u64) -> Result<Vec<NotificationInfo>> {
        let pk = self.public_key
            .ok_or_else(|| anyhow!("通知の取得には認証が必要です。設定ファイルに nsec を設定してください。"))?;

        // メンション（p タグで自分を参照しているテキストノート）
        let mut mention_filter = Filter::new()
            .kind(Kind::TextNote)
            .pubkey(pk)
            .limit(limit as usize);

        if let Some(since_ts) = since {
            mention_filter = mention_filter.since(Timestamp::from(since_ts));
        }

        // リアクション（p タグで自分を参照しているリアクション）
        let mut reaction_filter = Filter::new()
            .kind(Kind::Reaction)
            .pubkey(pk)
            .limit(limit as usize);

        if let Some(since_ts) = since {
            reaction_filter = reaction_filter.since(Timestamp::from(since_ts));
        }

        let events = self.client
            .fetch_events(vec![mention_filter, reaction_filter], Duration::from_secs(15))
            .await
            .context("通知の取得に失敗しました")?;

        let events_vec: Vec<Event> = events.into_iter()
            .filter(|e| e.pubkey != pk) // 自分自身の投稿を除外
            .collect();

        let pubkeys = Self::collect_pubkeys(&events_vec);
        let profiles = self.fetch_profiles(&pubkeys).await;

        let mut notifications: Vec<NotificationInfo> = events_vec.iter().map(|event| {
            let author = profiles
                .get(&event.pubkey)
                .cloned()
                .unwrap_or_else(|| AuthorInfo::from_public_key(&event.pubkey));

            let notification_type = match event.kind {
                Kind::Reaction => "reaction".to_string(),
                Kind::TextNote => "mention".to_string(),
                _ => "other".to_string(),
            };

            // リアクションの場合、対象ノートの ID を取得
            let target_note_id = event.tags.iter().find_map(|tag| {
                let values = tag.as_slice();
                if values.len() >= 2 && values[0] == "e" {
                    Some(values[1].to_string())
                } else {
                    None
                }
            });

            NotificationInfo {
                id: event.id.to_hex(),
                nevent: event.id.to_bech32().unwrap_or_default(),
                notification_type,
                author,
                content: event.content.clone(),
                target_note_id,
                created_at: event.created_at.as_u64(),
            }
        }).collect();

        notifications.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        notifications.truncate(limit as usize);

        Ok(notifications)
    }

    // ========================================
    // Phase 4: Zap サポート (NIP-57)
    // ========================================

    /// ノートの Zap レシート (Kind 9735) を取得します。
    pub async fn get_zap_receipts(&self, note_id: &str, limit: u64) -> Result<Vec<ZapReceiptInfo>> {
        let event_id = Self::parse_event_id(note_id)?;

        // Kind 9735 (Zap Receipt) を取得
        let filter = Filter::new()
            .kind(Kind::ZapReceipt)
            .event(event_id)
            .limit(limit as usize);

        let events = self.client
            .fetch_events(vec![filter], Duration::from_secs(10))
            .await
            .context("Zap レシートの取得に失敗しました")?;

        let events_vec: Vec<Event> = events.into_iter().collect();
        let mut receipts = Vec::new();

        for event in &events_vec {
            let receipt = self.parse_zap_receipt(event).await;
            receipts.push(receipt);
        }

        receipts.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        receipts.truncate(limit as usize);

        Ok(receipts)
    }

    /// Zap レシートイベントをパースするヘルパー
    async fn parse_zap_receipt(&self, event: &Event) -> ZapReceiptInfo {
        // bolt11 タグから金額を抽出
        let bolt11 = extract_tag_value(event, "bolt11").unwrap_or_default();
        let amount_sats = Self::extract_bolt11_amount(&bolt11);

        // description タグから Zap リクエストを取得（送信者・コメント情報）
        let description = extract_tag_value(event, "description");
        let (sender_pubkey, comment) = if let Some(ref desc) = description {
            Self::parse_zap_request_description(desc)
        } else {
            (None, None)
        };

        // 送信者のプロフィールを取得
        let sender = if let Some(pk_hex) = &sender_pubkey {
            if let Ok(pk) = PublicKey::from_hex(pk_hex) {
                let profiles = self.fetch_profiles(&[pk]).await;
                profiles.get(&pk).cloned()
            } else {
                None
            }
        } else {
            None
        };

        // 対象ノート ID とpubkey を取得
        let target_note_id = event.tags.iter().find_map(|tag| {
            let values = tag.as_slice();
            if values.len() >= 2 && values[0] == "e" {
                Some(values[1].to_string())
            } else {
                None
            }
        });

        let target_pubkey = event.tags.iter().find_map(|tag| {
            let values = tag.as_slice();
            if values.len() >= 2 && values[0] == "p" {
                Some(values[1].to_string())
            } else {
                None
            }
        });

        ZapReceiptInfo {
            id: event.id.to_hex(),
            nevent: event.id.to_bech32().unwrap_or_default(),
            sender,
            amount_sats,
            comment,
            target_note_id,
            target_pubkey,
            created_at: event.created_at.as_u64(),
        }
    }

    /// bolt11 インボイスから金額（sats）を抽出
    fn extract_bolt11_amount(bolt11: &str) -> u64 {
        // bolt11 形式: lnbc{amount}{multiplier}...
        // multiplier: m = milli (0.001), u = micro (0.000001), n = nano, p = pico
        let bolt11_lower = bolt11.to_lowercase();
        if let Some(start) = bolt11_lower.strip_prefix("lnbc") {
            // 数字部分を取得
            let num_str: String = start.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(num) = num_str.parse::<u64>() {
                let after_num = &start[num_str.len()..];
                if after_num.starts_with('m') {
                    return num * 100_000; // milli-BTC → sats
                } else if after_num.starts_with('u') {
                    return num * 100; // micro-BTC → sats
                } else if after_num.starts_with('n') {
                    return num / 10; // nano-BTC → sats
                } else if after_num.starts_with('p') {
                    return num / 10_000; // pico-BTC → sats
                } else {
                    return num * 100_000_000; // BTC → sats
                }
            }
        }
        0
    }

    /// Zap リクエストの description JSON から送信者 pubkey とコメントを抽出
    fn parse_zap_request_description(description: &str) -> (Option<String>, Option<String>) {
        if let Ok(event) = serde_json::from_str::<serde_json::Value>(description) {
            let pubkey = event.get("pubkey")
                .and_then(|v| v.as_str())
                .map(String::from);
            let comment = event.get("content")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from);
            (pubkey, comment)
        } else {
            (None, None)
        }
    }

    /// ノートまたはプロフィールに Zap を送信します（NWC 設定が必要）。
    pub async fn send_zap(&self, target: &str, amount_sats: u64, comment: Option<&str>) -> Result<serde_json::Value> {
        self.require_write_access()?;

        if !self.client.has_zapper().await {
            return Err(anyhow!(
                "Zap 送信には NWC (Nostr Wallet Connect) の設定が必要です。\
                設定ファイルに \"nwc-uri\" を追加してください。"
            ));
        }

        // target がイベント ID かpubkey かを判定
        let zap_entity: ZapEntity = if target.starts_with("npub") || (!target.starts_with("note") && !target.starts_with("nevent") && target.len() == 64 && target.chars().all(|c| c.is_ascii_hexdigit())) {
            // pubkey として解釈を試みる（ただし64文字hex以外も考慮）
            if let Ok(pk) = Self::parse_public_key(target) {
                ZapEntity::from(pk)
            } else if let Ok(eid) = Self::parse_event_id(target) {
                ZapEntity::from(eid)
            } else {
                return Err(anyhow!("無効な target です。イベント ID または公開鍵を指定してください。"));
            }
        } else {
            // イベント ID として解釈
            if let Ok(eid) = Self::parse_event_id(target) {
                ZapEntity::from(eid)
            } else if let Ok(pk) = Self::parse_public_key(target) {
                ZapEntity::from(pk)
            } else {
                return Err(anyhow!("無効な target です。イベント ID または公開鍵を指定してください。"));
            }
        };

        let details = if let Some(msg) = comment {
            Some(ZapDetails::new(ZapType::Public).message(msg))
        } else {
            Some(ZapDetails::new(ZapType::Public))
        };

        self.client.zap(zap_entity, amount_sats, details).await
            .context("Zap の送信に失敗しました")?;

        info!("Zap を送信しました: {} sats → {}", amount_sats, target);

        Ok(serde_json::json!({
            "success": true,
            "amount_sats": amount_sats,
            "target": target,
            "message": format!("{} sats の Zap を送信しました。", amount_sats)
        }))
    }

    // ========================================
    // Phase 4: ダイレクトメッセージ (NIP-04)
    // ========================================

    /// 暗号化されたダイレクトメッセージを送信します（NIP-04）。
    pub async fn send_dm(&self, recipient: &str, content: &str) -> Result<EventId> {
        self.require_write_access()?;

        let recipient_pk = Self::parse_public_key(recipient)?;

        // NIP-04: signer を使って暗号化
        let signer = self.client.signer().await
            .map_err(|e| anyhow!("署名者の取得に失敗: {}", e))?;
        let encrypted = signer.nip04_encrypt(&recipient_pk, content).await
            .map_err(|e| anyhow!("メッセージの暗号化に失敗: {}", e))?;

        // Kind 4 (Encrypted Direct Message) イベントを作成
        let builder = EventBuilder::new(Kind::EncryptedDirectMessage, encrypted)
            .tags(vec![Tag::public_key(recipient_pk)]);

        let output = self.client.send_event_builder(builder).await
            .context("ダイレクトメッセージの送信に失敗しました")?;

        let event_id = *output.id();
        info!("DM を送信しました。イベント ID: {}", event_id);
        Ok(event_id)
    }

    /// ダイレクトメッセージの会話を取得します（NIP-04）。
    pub async fn get_dms(&self, with: Option<&str>, limit: u64) -> Result<Vec<DirectMessageInfo>> {
        let pk = self.public_key
            .ok_or_else(|| anyhow!("DM の取得には認証が必要です。設定ファイルに nsec を設定してください。"))?;

        let signer = self.client.signer().await
            .map_err(|e| anyhow!("署名者の取得に失敗: {}", e))?;

        // 相手の公開鍵（指定されている場合）
        let peer_pk = if let Some(with_str) = with {
            Some(Self::parse_public_key(with_str)?)
        } else {
            None
        };

        // 受信 DM: 自分宛の Kind 4 イベント
        let mut received_filter = Filter::new()
            .kind(Kind::EncryptedDirectMessage)
            .pubkey(pk)
            .limit(limit as usize);

        if let Some(ref peer) = peer_pk {
            received_filter = received_filter.author(*peer);
        }

        // 送信 DM: 自分が送った Kind 4 イベント
        let mut sent_filter = Filter::new()
            .kind(Kind::EncryptedDirectMessage)
            .author(pk)
            .limit(limit as usize);

        if let Some(ref peer) = peer_pk {
            sent_filter = sent_filter.pubkey(*peer);
        }

        let events = self.client
            .fetch_events(vec![received_filter, sent_filter], Duration::from_secs(15))
            .await
            .context("DM の取得に失敗しました")?;

        let events_vec: Vec<Event> = events.into_iter()
            .collect();

        let pubkeys = Self::collect_pubkeys(&events_vec);
        let profiles = self.fetch_profiles(&pubkeys).await;

        let mut messages = Vec::new();

        for event in &events_vec {
            let is_sent = event.pubkey == pk;
            let peer_pubkey = if is_sent {
                // 送信メッセージ: p タグから相手の pubkey を取得
                event.tags.iter().find_map(|tag| {
                    let values = tag.as_slice();
                    if values.len() >= 2 && values[0] == "p" {
                        PublicKey::from_hex(&values[1]).ok()
                    } else {
                        None
                    }
                })
            } else {
                Some(event.pubkey)
            };

            let Some(peer) = peer_pubkey else { continue };

            // NIP-04 復号
            let decrypted = if is_sent {
                signer.nip04_decrypt(&peer, &event.content).await
            } else {
                signer.nip04_decrypt(&event.pubkey, &event.content).await
            };

            let content = match decrypted {
                Ok(text) => text,
                Err(e) => {
                    debug!("DM 復号に失敗（スキップ）: {}", e);
                    continue;
                }
            };

            let author = profiles
                .get(&event.pubkey)
                .cloned()
                .unwrap_or_else(|| AuthorInfo::from_public_key(&event.pubkey));

            messages.push(DirectMessageInfo {
                id: event.id.to_hex(),
                nevent: event.id.to_bech32().unwrap_or_default(),
                author,
                content,
                direction: if is_sent { "sent".to_string() } else { "received".to_string() },
                peer_pubkey: peer.to_hex(),
                created_at: event.created_at.as_u64(),
            });
        }

        messages.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        messages.truncate(limit as usize);

        Ok(messages)
    }

    // ========================================
    // Phase 4: リレーリスト (NIP-65)
    // ========================================

    /// ユーザーのリレーリスト (Kind 10002, NIP-65) を取得します。
    pub async fn get_relay_list(&self, pubkey_str: &str) -> Result<RelayListInfo> {
        let public_key = Self::parse_public_key(pubkey_str)?;

        let filter = Filter::new()
            .author(public_key)
            .kind(Kind::RelayList)
            .limit(1);

        let events = self.client
            .fetch_events(vec![filter], Duration::from_secs(10))
            .await
            .context("リレーリストの取得に失敗しました")?;

        let event = events
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("{} のリレーリストが見つかりません", pubkey_str))?;

        let relays: Vec<RelayListEntry> = nip65::extract_relay_list(&event)
            .map(|(url, metadata)| {
                let (read, write) = match metadata {
                    Some(RelayMetadata::Read) => (true, false),
                    Some(RelayMetadata::Write) => (false, true),
                    None => (true, true), // メタデータなし = 両方
                };
                RelayListEntry {
                    url: url.to_string(),
                    read,
                    write,
                }
            })
            .collect();

        Ok(RelayListInfo {
            pubkey: public_key.to_hex(),
            npub: public_key.to_bech32().unwrap_or_default(),
            relays,
        })
    }

    /// イベント ID 文字列をパース（nevent、note、hex 対応）
    fn parse_event_id(id_str: &str) -> Result<EventId> {
        let id_str = id_str.trim();
        if id_str.starts_with("nevent") {
            let nip19 = Nip19Event::from_bech32(id_str)
                .context("無効な nevent 形式です")?;
            Ok(nip19.event_id)
        } else if id_str.starts_with("note") {
            EventId::from_bech32(id_str).context("無効な note 形式です")
        } else {
            EventId::from_hex(id_str).context("無効な hex イベント ID です")
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

/// プロフィール統計情報（Phase 3: プロフィールカード用）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProfileStats {
    /// フォロー中の数
    pub following: u64,
    /// フォロワー数（推定値、リレーの対応状況に依存）
    pub followers: u64,
    /// ノート投稿数（推定値）
    pub notes: u64,
}

/// スレッド情報（Phase 2）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ThreadInfo {
    /// ルートノート
    pub root: NoteInfo,
    /// リプライ一覧（ネスト構造）
    pub replies: Vec<ThreadReply>,
    /// リプライの総数
    pub total_replies: u64,
    /// 取得したリプライの深さ
    pub depth: u64,
}

/// スレッドのリプライ（ネスト可能）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ThreadReply {
    /// リプライノート
    pub note: NoteInfo,
    /// さらにネストされたリプライ
    pub replies: Vec<ThreadReply>,
}

/// 通知情報（Phase 2）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NotificationInfo {
    /// hex 形式のイベント ID
    pub id: String,
    /// nevent 形式のイベント ID
    pub nevent: String,
    /// 通知の種類（"mention" または "reaction"）
    pub notification_type: String,
    /// 通知元の著者情報
    pub author: AuthorInfo,
    /// コンテンツ（リアクションの場合は絵文字、メンションの場合はノート内容）
    pub content: String,
    /// リアクション対象のノート ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_note_id: Option<String>,
    /// 作成日時の Unix タイムスタンプ
    pub created_at: u64,
}

// ========================================
// Phase 4: データ構造体
// ========================================

/// Zap レシート情報（NIP-57）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ZapReceiptInfo {
    /// hex 形式のイベント ID
    pub id: String,
    /// nevent 形式のイベント ID
    pub nevent: String,
    /// Zap 送信者の情報（Zap リクエストから取得）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sender: Option<AuthorInfo>,
    /// Zap 金額（sats）
    pub amount_sats: u64,
    /// Zap コメント
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Zap 対象のノート ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_note_id: Option<String>,
    /// Zap 対象の pubkey
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_pubkey: Option<String>,
    /// 作成日時の Unix タイムスタンプ
    pub created_at: u64,
}

/// ダイレクトメッセージ情報（NIP-04）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DirectMessageInfo {
    /// hex 形式のイベント ID
    pub id: String,
    /// nevent 形式のイベント ID
    pub nevent: String,
    /// 送信者の情報
    pub author: AuthorInfo,
    /// 復号済みメッセージ内容
    pub content: String,
    /// メッセージの方向（"sent" または "received"）
    pub direction: String,
    /// 会話相手の pubkey (hex)
    pub peer_pubkey: String,
    /// 作成日時の Unix タイムスタンプ
    pub created_at: u64,
}

/// リレーリスト情報（NIP-65）
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RelayListInfo {
    /// hex 形式の公開鍵
    pub pubkey: String,
    /// npub 形式の公開鍵
    pub npub: String,
    /// リレー一覧
    pub relays: Vec<RelayListEntry>,
}

/// リレーリストのエントリ
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RelayListEntry {
    /// リレー URL
    pub url: String,
    /// 読み取り可能
    pub read: bool,
    /// 書き込み可能
    pub write: bool,
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

/// 現在の Unix タイムスタンプ（秒）を取得
fn current_unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 記事/下書きの共通タグを構築するヘルパー
fn build_article_tags(
    title: &str,
    summary: &Option<String>,
    image: &Option<String>,
    hashtags: &Option<Vec<String>>,
    d_tag: &str,
) -> Vec<Tag> {
    let mut tags = vec![
        Tag::identifier(d_tag.to_string()),
        Tag::custom(TagKind::Title, vec![title.to_string()]),
    ];

    if let Some(ref s) = summary {
        tags.push(Tag::custom(TagKind::custom("summary".to_string()), vec![s.clone()]));
    }

    if let Some(ref img) = image {
        tags.push(Tag::custom(TagKind::custom("image".to_string()), vec![img.clone()]));
    }

    if let Some(ref ht) = hashtags {
        for t in ht {
            tags.push(Tag::hashtag(t.clone()));
        }
    }

    tags
}
