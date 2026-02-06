//! NIP-46 Nostr Connect セッション管理モジュール
//!
//! リモートサイナー（Primal、Amber 等のモバイルウォレット）との
//! NIP-46 (Nostr Connect) 接続を管理します。
//!
//! 2 つの接続方式をサポート:
//! - クライアント発行方式: `nostrconnect://` URI を QR コードとして表示
//! - バンカー方式: `bunker://` URI を config に設定
//!
//! Step 6-3: 認証モード切り替え
//! NostrClient と統合し、NIP-46 接続完了後に自動的にサイナーを切り替えます。

use anyhow::{anyhow, Context, Result};
use base64::Engine;
use nostr_connect::prelude::{
    NostrConnect, NostrConnectMetadata, NostrConnectURI, RelayUrl, Url,
};
use nostr_sdk::prelude::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// NIP-46 接続のデフォルトタイムアウト（秒）
const DEFAULT_NIP46_TIMEOUT_SECS: u64 = 120;

/// NIP-46 通信用のデフォルトリレー
const DEFAULT_NIP46_RELAYS: &[&str] = &[
    "wss://relay.nsec.app",
    "wss://relay.damus.io",
];

/// QR コードの画像サイズ（ピクセル）
const QR_IMAGE_SIZE: u32 = 256;

/// NIP-46 接続状態
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum Nip46State {
    /// 未接続
    Disconnected,
    /// 接続待ち（QR コードを表示中）
    WaitingForConnection {
        /// nostrconnect:// URI 文字列
        connect_uri: String,
        /// QR コードの Base64 PNG
        qr_base64: String,
    },
    /// 接続済み
    Connected {
        /// リモートサイナーのユーザー公開鍵
        user_pubkey: PublicKey,
    },
    /// エラー
    Error(String),
}

/// NIP-46 セッション設定
#[derive(Debug, Clone)]
pub struct Nip46Config {
    /// NIP-46 通信用リレー
    pub relays: Vec<String>,
    /// 要求する権限（カンマ区切り: "sign_event:1,sign_event:7,nip44_encrypt,nip44_decrypt"）
    /// Step 6-3 で実装する権限粒度制御で使用
    #[allow(dead_code)]
    pub perms: Option<String>,
    /// bunker:// URI（バンカー方式の場合）
    pub bunker_uri: Option<String>,
}

/// NIP-46 セッションマネージャー
pub struct Nip46Session {
    /// 接続状態
    state: Arc<RwLock<Nip46State>>,
    /// NostrConnect サイナー（接続確立後に保持）
    signer: Arc<RwLock<Option<NostrConnect>>>,
    /// アプリケーション鍵ペア（NIP-46 通信チャネル用）
    app_keys: Keys,
    /// セッション設定
    config: Nip46Config,
}

impl Nip46Session {
    /// 新しい NIP-46 セッションを作成
    pub fn new(config: Nip46Config) -> Self {
        let app_keys = Keys::generate();

        Self {
            state: Arc::new(RwLock::new(Nip46State::Disconnected)),
            signer: Arc::new(RwLock::new(None)),
            app_keys,
            config,
        }
    }

    /// 現在の接続状態を取得
    #[allow(dead_code)]
    pub async fn state(&self) -> Nip46State {
        self.state.read().await.clone()
    }

    /// NostrConnect サイナーを取得（接続済みの場合のみ）
    /// Step 6-3 の認証モード切り替えで使用
    #[allow(dead_code)]
    pub async fn get_signer(&self) -> Option<NostrConnect> {
        self.signer.read().await.clone()
    }

    /// クライアント発行方式で NIP-46 接続を開始。
    /// `nostrconnect://` URI を生成し、QR コードを返す。
    pub async fn start_client_connect(&self) -> Result<Nip46ConnectResult> {
        info!("NIP-46 クライアント接続を開始");

        // リレー URL をパース
        let relay_urls = self.parse_relay_urls()?;

        // nostrconnect:// URI を構築
        let metadata = NostrConnectMetadata::new("rust-nostr-mcp")
            .description("Nostr MCP Server for AI agents")
            .url(
                Url::parse("https://github.com/tami1A84/rust-nostr-mcp")
                    .context("URL パースに失敗")?,
            );

        let uri = NostrConnectURI::Client {
            public_key: self.app_keys.public_key(),
            relays: relay_urls.clone(),
            metadata,
        };

        let uri_string = uri.to_string();
        info!("nostrconnect:// URI を生成: {}...", &uri_string[..uri_string.len().min(60)]);

        // QR コードを生成
        let qr_base64 = generate_qr_base64(&uri_string)?;
        info!("QR コード生成完了（Base64 PNG）");

        // NostrConnect サイナーを作成
        let signer = NostrConnect::new(
            uri,
            self.app_keys.clone(),
            Duration::from_secs(DEFAULT_NIP46_TIMEOUT_SECS),
            None,
        )
        .map_err(|e| anyhow!("NostrConnect の作成に失敗: {}", e))?;

        // サイナーを保存
        {
            let mut signer_lock = self.signer.write().await;
            *signer_lock = Some(signer);
        }

        // 状態を更新
        {
            let mut state = self.state.write().await;
            *state = Nip46State::WaitingForConnection {
                connect_uri: uri_string.clone(),
                qr_base64: qr_base64.clone(),
            };
        }

        Ok(Nip46ConnectResult {
            connect_uri: uri_string,
            qr_base64,
        })
    }

    /// バンカー方式で NIP-46 接続を開始
    pub async fn start_bunker_connect(&self, bunker_uri_str: &str) -> Result<()> {
        info!("NIP-46 バンカー接続を開始");

        let uri = NostrConnectURI::parse(bunker_uri_str)
            .map_err(|e| anyhow!("bunker URI のパースに失敗: {}", e))?;

        let signer = NostrConnect::new(
            uri,
            self.app_keys.clone(),
            Duration::from_secs(DEFAULT_NIP46_TIMEOUT_SECS),
            None,
        )
        .map_err(|e| anyhow!("NostrConnect の作成に失敗: {}", e))?;

        // 接続テスト: get_public_key を呼んで bootstrap を発動
        let user_pubkey = signer
            .get_public_key()
            .await
            .map_err(|e| anyhow!("リモートサイナーへの接続に失敗: {}", e))?;

        info!(
            "NIP-46 バンカー接続成功: {}",
            user_pubkey.to_bech32().unwrap_or_default()
        );

        // サイナーを保存
        {
            let mut signer_lock = self.signer.write().await;
            *signer_lock = Some(signer);
        }

        // 状態を更新
        {
            let mut state = self.state.write().await;
            *state = Nip46State::Connected { user_pubkey };
        }

        Ok(())
    }

    /// リモートサイナーとの接続を待ち、接続完了を確認する。
    /// クライアント発行方式で QR スキャン後に呼び出す。
    /// Step 6-3/6-4 で接続完了後の認証フローで使用
    #[allow(dead_code)]
    pub async fn wait_for_connection(&self) -> Result<PublicKey> {
        let signer = {
            let signer_lock = self.signer.read().await;
            signer_lock
                .clone()
                .ok_or_else(|| anyhow!("NIP-46 セッションが開始されていません"))?
        };

        info!("リモートサイナーの接続を待機中...");

        // get_public_key を呼ぶことで bootstrap が発動し、
        // リモートサイナーからの接続を待つ
        let user_pubkey = signer
            .get_public_key()
            .await
            .map_err(|e| anyhow!("リモートサイナーの接続待機に失敗: {}", e))?;

        info!(
            "NIP-46 接続成功: {}",
            user_pubkey.to_bech32().unwrap_or_default()
        );

        // 状態を更新
        {
            let mut state = self.state.write().await;
            *state = Nip46State::Connected { user_pubkey };
        }

        Ok(user_pubkey)
    }

    /// リモートサイナーとの接続を切断
    pub async fn disconnect(&self) -> Result<()> {
        let signer = {
            let mut signer_lock = self.signer.write().await;
            signer_lock.take()
        };

        if let Some(signer) = signer {
            if let Err(e) = signer.shutdown().await {
                warn!("NIP-46 シャットダウン中にエラー: {}", e);
            }
            info!("NIP-46 接続を切断しました");
        }

        {
            let mut state = self.state.write().await;
            *state = Nip46State::Disconnected;
        }

        Ok(())
    }

    /// 接続済みかどうかを確認
    pub async fn is_connected(&self) -> bool {
        matches!(&*self.state.read().await, Nip46State::Connected { .. })
    }

    /// 接続済みユーザーの公開鍵を取得
    pub async fn connected_pubkey(&self) -> Option<PublicKey> {
        match &*self.state.read().await {
            Nip46State::Connected { user_pubkey } => Some(*user_pubkey),
            _ => None,
        }
    }

    /// NIP-46 サイナーを NostrClient に設定するための NostrConnect インスタンスを取得
    pub async fn get_nostr_connect(&self) -> Option<NostrConnect> {
        self.signer.read().await.clone()
    }

    /// 接続ステータスを JSON 値として取得
    pub async fn status_json(&self) -> serde_json::Value {
        let state = self.state.read().await;

        match &*state {
            Nip46State::Disconnected => serde_json::json!({
                "status": "disconnected",
                "message": "NIP-46 リモートサイナーに接続されていません。"
            }),
            Nip46State::WaitingForConnection {
                connect_uri,
                qr_base64,
            } => serde_json::json!({
                "status": "waiting",
                "message": "リモートサイナーの接続を待機中。QR コードをスキャンしてください。",
                "connect_uri": connect_uri,
                "qr_base64": qr_base64
            }),
            Nip46State::Connected { user_pubkey } => serde_json::json!({
                "status": "connected",
                "message": "NIP-46 リモートサイナーに接続済み。",
                "user_pubkey": user_pubkey.to_hex(),
                "user_npub": user_pubkey.to_bech32().unwrap_or_default()
            }),
            Nip46State::Error(msg) => serde_json::json!({
                "status": "error",
                "message": msg
            }),
        }
    }

    /// リレー URL リストをパース
    fn parse_relay_urls(&self) -> Result<Vec<RelayUrl>> {
        let relay_strs = if self.config.relays.is_empty() {
            DEFAULT_NIP46_RELAYS.iter().map(|s| s.to_string()).collect()
        } else {
            self.config.relays.clone()
        };

        relay_strs
            .iter()
            .map(|url| {
                RelayUrl::parse(url)
                    .map_err(|e| anyhow!("リレー URL のパースに失敗 '{}': {}", url, e))
            })
            .collect()
    }
}

/// NIP-46 接続開始の結果
#[derive(Debug, Clone)]
pub struct Nip46ConnectResult {
    /// nostrconnect:// URI 文字列
    pub connect_uri: String,
    /// QR コードの Base64 エンコード PNG 画像
    pub qr_base64: String,
}

/// 文字列から QR コードを PNG 画像として生成し、Base64 エンコードする
pub fn generate_qr_base64(data: &str) -> Result<String> {
    use ::image::codecs::png::PngEncoder;
    use ::image::{ExtendedColorType, ImageBuffer, ImageEncoder, Luma};
    use qrcode::QrCode;

    debug!("QR コード生成中: {}...", &data[..data.len().min(40)]);

    // QR コードを生成
    let code = QrCode::new(data.as_bytes()).context("QR コードの生成に失敗しました")?;

    // QR コードモジュール数に基づいてスケールを計算
    let module_count = code.width() as u32;
    let scale = (QR_IMAGE_SIZE / module_count).max(1);
    let quiet_zone = scale * 2; // 周囲の余白
    let img_size = module_count * scale + quiet_zone * 2;

    // PNG 画像を生成
    let mut img = ImageBuffer::<Luma<u8>, Vec<u8>>::new(img_size, img_size);

    // 背景を白に
    for pixel in img.pixels_mut() {
        *pixel = Luma([255u8]);
    }

    // QR コードモジュールを描画
    for (y, row) in code.to_colors().chunks(module_count as usize).enumerate() {
        for (x, &color) in row.iter().enumerate() {
            if color == qrcode::Color::Dark {
                let px = x as u32 * scale + quiet_zone;
                let py = y as u32 * scale + quiet_zone;
                for dy in 0..scale {
                    for dx in 0..scale {
                        if px + dx < img_size && py + dy < img_size {
                            img.put_pixel(px + dx, py + dy, Luma([0u8]));
                        }
                    }
                }
            }
        }
    }

    // PNG としてバイト列にエンコード
    let mut png_bytes: Vec<u8> = Vec::new();
    let encoder = PngEncoder::new(&mut png_bytes);
    encoder
        .write_image(&img, img_size, img_size, ExtendedColorType::L8)
        .context("PNG エンコードに失敗しました")?;

    // Base64 エンコード
    let base64_str = base64::engine::general_purpose::STANDARD.encode(&png_bytes);

    debug!(
        "QR コード生成完了: {}x{}px, Base64 {} bytes",
        img_size,
        img_size,
        base64_str.len()
    );

    Ok(base64_str)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_qr_base64() {
        let data = "nostrconnect://abc123?relay=wss://relay.damus.io";
        let result = generate_qr_base64(data);
        assert!(result.is_ok());

        let base64_str = result.unwrap();
        assert!(!base64_str.is_empty());

        // Base64 文字列が有効なことを確認
        let decoded = base64::engine::general_purpose::STANDARD.decode(&base64_str);
        assert!(decoded.is_ok());

        // PNG ヘッダーを確認
        let bytes = decoded.unwrap();
        assert!(bytes.len() > 8);
        assert_eq!(&bytes[0..4], &[0x89, 0x50, 0x4E, 0x47]); // PNG magic number
    }

    #[test]
    fn test_nip46_config_default_relays() {
        let config = Nip46Config {
            relays: vec![],
            perms: None,
            bunker_uri: None,
        };
        let session = Nip46Session::new(config);
        let relay_urls = session.parse_relay_urls();
        assert!(relay_urls.is_ok());
        assert_eq!(relay_urls.unwrap().len(), DEFAULT_NIP46_RELAYS.len());
    }

    #[test]
    fn test_nip46_config_custom_relays() {
        let config = Nip46Config {
            relays: vec!["wss://custom.relay.example".to_string()],
            perms: None,
            bunker_uri: None,
        };
        let session = Nip46Session::new(config);
        let relay_urls = session.parse_relay_urls();
        assert!(relay_urls.is_ok());
        assert_eq!(relay_urls.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_session_initial_state() {
        let config = Nip46Config {
            relays: vec![],
            perms: None,
            bunker_uri: None,
        };
        let session = Nip46Session::new(config);
        let state = session.state().await;
        assert_eq!(state, Nip46State::Disconnected);
    }

    #[tokio::test]
    async fn test_status_json_disconnected() {
        let config = Nip46Config {
            relays: vec![],
            perms: None,
            bunker_uri: None,
        };
        let session = Nip46Session::new(config);
        let json = session.status_json().await;
        assert_eq!(json["status"], "disconnected");
    }
}
